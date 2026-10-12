// MNA stamps, LU, DC and backward-Euler transient
use std::collections::HashSet;

use crate::{Ctrl, Kind::*, Netlist};

pub struct State {
    pub time: f64,
    pub volts: Vec<f64>, // per node, ground 0
    pub amps: Vec<f64>,  // per part, from→to
}

// unknowns: node volts, then branch currents
// branch rows: V/E/H always, plus any part an F/H senses
struct Mna<'a> {
    net: &'a Netlist,
    n: usize,
    size: usize,
    branch: Vec<Option<usize>>,
    owner: Vec<usize>,
}

fn node(i: usize) -> Option<usize> {
    i.checked_sub(1)
}

impl<'a> Mna<'a> {
    fn new(net: &'a Netlist) -> Self {
        let n = net.nodes.len() - 1;
        let sensed: HashSet<usize> = net
            .parts
            .iter()
            .filter_map(|p| if let Ctrl::Part(c) = p.ctrl { Some(c) } else { None })
            .collect();
        let mut owner = vec![];
        let branch = (0..net.parts.len())
            .map(|e| {
                (matches!(net.parts[e].kind, V | E | H) || sensed.contains(&e)).then(|| {
                    owner.push(e);
                    n + owner.len() - 1
                })
            })
            .collect();
        Mna { net, n, size: n + owner.len(), branch, owner }
    }

    fn ctrl(&self, e: usize) -> (Option<usize>, Option<usize>, Option<usize>) {
        match self.net.parts[e].ctrl {
            Ctrl::Nodes(p, m) => (node(p), node(m), None),
            Ctrl::Part(c) => (None, None, self.branch[c]),
            Ctrl::None => (None, None, None),
        }
    }

    // h: None = DC with caps open, Some = one BE step
    fn matrix(&self, h: Option<f64>) -> Vec<f64> {
        let s = self.size;
        let mut a = vec![0.0; s * s];
        let mut add = |r: Option<usize>, c: Option<usize>, v: f64| {
            if let (Some(r), Some(c)) = (r, c) {
                a[r * s + c] += v;
            }
        };
        for (e, p) in self.net.parts.iter().enumerate() {
            let (f, t, v) = (node(p.from), node(p.to), p.value);
            let (cp, cn, ic) = self.ctrl(e);
            let g = match p.kind {
                R => 1.0 / v,
                C => h.map_or(0.0, |h| v / h),
                _ => 0.0,
            };
            let Some(k) = self.branch[e].map(Some) else {
                match p.kind {
                    R | C => {
                        add(f, f, g);
                        add(t, t, g);
                        add(f, t, -g);
                        add(t, f, -g);
                    }
                    G => {
                        add(f, cp, v);
                        add(f, cn, -v);
                        add(t, cp, -v);
                        add(t, cn, v);
                    }
                    F => {
                        add(f, ic, v);
                        add(t, ic, -v);
                    }
                    _ => {}
                }
                continue;
            };
            // i_k leaves from, enters to
            add(f, k, 1.0);
            add(t, k, -1.0);
            match p.kind {
                V | E | H => {
                    add(k, t, 1.0);
                    add(k, f, -1.0);
                }
                _ => add(k, k, 1.0),
            }
            match p.kind {
                E | G => {
                    add(k, cp, -v);
                    add(k, cn, v);
                }
                H | F => add(k, ic, -v),
                R | C => {
                    add(k, f, -g);
                    add(k, t, g);
                }
                _ => {}
            }
        }
        a
    }

    // prev: per part cap voltage from the last step
    fn rhs(&self, h: Option<f64>, prev: &[f64]) -> Vec<f64> {
        let mut b = vec![0.0; self.size];
        for (e, p) in self.net.parts.iter().enumerate() {
            let (f, t) = (node(p.from), node(p.to));
            let ieq = if p.kind == C { h.map_or(0.0, |h| p.value / h * prev[e]) } else { 0.0 };
            match (p.kind, self.branch[e]) {
                (V | I, Some(k)) => b[k] = p.value,
                (C, Some(k)) => b[k] = -ieq,
                (I, None) | (C, None) => {
                    let j = if p.kind == I { -p.value } else { ieq };
                    if let Some(f) = f {
                        b[f] += j;
                    }
                    if let Some(t) = t {
                        b[t] -= j;
                    }
                }
                _ => {}
            }
        }
        b
    }

    fn read(&self, x: &[f64], h: Option<f64>, prev: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let volts: Vec<f64> = std::iter::once(0.0).chain(x[..self.n].iter().copied()).collect();
        let amps = self
            .net
            .parts
            .iter()
            .enumerate()
            .map(|(e, p)| {
                let drop = volts[p.from] - volts[p.to];
                match (self.branch[e], p.kind, p.ctrl) {
                    (Some(k), ..) => x[k],
                    (_, R, _) => drop / p.value,
                    (_, C, _) => h.map_or(0.0, |h| p.value / h * (drop - prev[e])),
                    (_, I, _) => p.value,
                    (_, G, Ctrl::Nodes(a, b)) => p.value * (volts[a] - volts[b]),
                    (_, F, Ctrl::Part(c)) => p.value * x[self.branch[c].unwrap()],
                    _ => 0.0,
                }
            })
            .collect();
        (volts, amps)
    }

    fn solve(&self, h: Option<f64>, prev: &[f64]) -> Result<(Vec<f64>, Vec<f64>), String> {
        let x = self.lu(h)?.solve(self.rhs(h, prev));
        Ok(self.read(&x, h, prev))
    }

    fn lu(&self, h: Option<f64>) -> Result<Lu, String> {
        Lu::new(self.matrix(h), self.size).map_err(|c| match c.checked_sub(self.n) {
            None => format!("net {} floating: nothing sets its voltage", self.net.nodes[c + 1]),
            Some(k) => format!("{} in a source loop or cut", self.net.parts[self.owner[k]].name),
        })
    }

    // per part voltage across caps, 0 elsewhere
    fn caps(&self, volts: &[f64]) -> Vec<f64> {
        self.net.parts.iter().map(|p| if p.kind == C { volts[p.from] - volts[p.to] } else { 0.0 }).collect()
    }
}

// row-pivoted LU, multipliers below the diagonal
struct Lu {
    a: Vec<f64>,
    piv: Vec<usize>,
    n: usize,
}

impl Lu {
    // Err = column without a pivot
    fn new(mut a: Vec<f64>, n: usize) -> Result<Lu, usize> {
        let tol = a.iter().fold(0.0f64, |m, v| m.max(v.abs())) * n as f64 * f64::EPSILON;
        let mut piv: Vec<usize> = (0..n).collect();
        for c in 0..n {
            let p = (c..n).max_by(|&i, &j| a[i * n + c].abs().total_cmp(&a[j * n + c].abs())).unwrap();
            if a[p * n + c].abs() <= tol {
                return Err(c);
            }
            if p != c {
                (0..n).for_each(|j| a.swap(p * n + j, c * n + j));
                piv.swap(p, c);
            }
            let (top, rest) = a.split_at_mut((c + 1) * n);
            let pivot = &top[c * n..];
            for row in rest.chunks_mut(n) {
                let f = row[c] / pivot[c];
                row[c] = f;
                row[c + 1..].iter_mut().zip(&pivot[c + 1..]).for_each(|(x, y)| *x -= f * y);
            }
        }
        Ok(Lu { a, piv, n })
    }

    fn solve(&self, b: Vec<f64>) -> Vec<f64> {
        let (a, n) = (&self.a, self.n);
        let mut x: Vec<f64> = self.piv.iter().map(|&i| b[i]).collect();
        for i in 0..n {
            x[i] -= (0..i).map(|j| a[i * n + j] * x[j]).sum::<f64>();
        }
        for i in (0..n).rev() {
            x[i] = (x[i] - (i + 1..n).map(|j| a[i * n + j] * x[j]).sum::<f64>()) / a[i * n + i];
        }
        x
    }
}

pub fn dc(net: &Netlist) -> Result<State, String> {
    let m = Mna::new(net);
    let (volts, amps) = m.solve(None, &vec![0.0; net.parts.len()])?;
    Ok(State { time: 0.0, volts, amps })
}

// round up to 1, 2 or 5 × 10^k
fn nice(x: f64) -> f64 {
    let p = 10f64.powf(x.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * p).find(|&v| v >= x * 0.999).unwrap_or(x)
}

// caps start discharged, sources switch on at t = 0
pub fn transient(net: &Netlist, frames: usize) -> Result<Vec<State>, String> {
    let m = Mna::new(net);
    let rest = vec![0.0; net.parts.len()];
    let span = |k| {
        let v = net.parts.iter().filter(|p| p.kind == k).map(|p| p.value.abs());
        (v.clone().fold(f64::INFINITY, f64::min), v.fold(0.0, f64::max))
    };
    let (c, r) = (span(C), span(R));
    let (rmin, rmax) = if r.1 > 0.0 { r } else { (1.0, 1.0) };

    // steady state: DC, else one huge step for cap-only nets
    let target = match m.solve(None, &rest) {
        Ok((v, _)) => m.caps(&v),
        Err(_) => {
            let v = m.caps(&m.solve(Some(1e6 * c.1 * rmax), &rest)?.0);
            let limit = 1e6 * net.parts.iter().map(|p| p.value.abs() * rmax).fold(1.0, f64::max);
            if let Some(e) = (0..v.len()).find(|&e| v[e].abs() > limit) {
                return Err(format!("{} charges without limit: no DC path", net.parts[e].name));
            }
            v
        }
    };
    let scale = target.iter().fold(1e-12f64, |a, v| a.max(v.abs()));

    // growing steps until within 1% of steady state
    let (mut t, mut h, mut prev) = (0.0, 1e-3 * c.0 * rmin, rest.clone());
    for _ in 0..400 {
        prev = m.caps(&m.solve(Some(h), &prev)?.0);
        t += h;
        if prev.iter().zip(&target).all(|(a, b)| (a - b).abs() <= 0.01 * scale) {
            break;
        }
        h *= 1.2;
    }

    // uniform run, one factorization
    let end = nice(t * 1.25);
    let (per, steps) = (8, frames * 8);
    let h = end / steps as f64;
    let (v0, a0) = m.solve(Some(end * 1e-6), &rest)?;
    let mut out = vec![State { time: 0.0, volts: v0, amps: a0 }];
    let lu = m.lu(Some(h))?;
    let mut prev = rest;
    for i in 1..=steps {
        let (volts, amps) = m.read(&lu.solve(m.rhs(Some(h), &prev)), Some(h), &prev);
        prev = m.caps(&volts);
        if i % per == 0 {
            out.push(State { time: i as f64 * h, volts, amps });
        }
    }
    Ok(out)
}
