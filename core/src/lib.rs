// nets in, MNA solve, scenes out
mod geom;
mod layout;
mod mna;
mod scene;

pub use scene::Sim;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub const VERSION: &str = env!("CIRCUIT_VERSION");
pub const GROUND: &str = "gnd";
const MAX_PARTS: usize = 500;
const CANVAS: (f32, f32) = (200.0, 8000.0);

// E vcvs, F cccs, G vccs, H ccvs
#[derive(Clone, Copy, PartialEq, Debug, Deserialize)]
pub enum Kind {
    R,
    C,
    V,
    I,
    E,
    F,
    G,
    H,
}

// from/to: net names, GROUND is ground
// ctrl: E/G [net+, net-], F/H [sensed part name]
#[derive(Deserialize)]
pub struct PartIn {
    pub name: String,
    pub kind: Kind,
    pub from: String,
    pub to: String,
    pub value: f64,
    #[serde(default)]
    pub ctrl: Vec<String>,
}

#[derive(Clone, Copy)]
pub enum Ctrl {
    None,
    Nodes(usize, usize),
    Part(usize),
}

pub struct Part {
    pub name: String,
    pub kind: Kind,
    pub from: usize,
    pub to: usize,
    pub value: f64,
    pub ctrl: Ctrl,
}

// node 0 = ground; skipped = parts not wired to ground
pub struct Netlist {
    pub nodes: Vec<String>,
    pub parts: Vec<Part>,
    pub skipped: Vec<String>,
}

pub fn run(parts: &[PartIn], width: f32, height: f32) -> Result<Sim, String> {
    let net = netlist(parts)?;
    let start = Instant::now();
    let states = if net.parts.iter().any(|p| p.kind == Kind::C) {
        mna::transient(&net, (2400 / net.parts.len()).clamp(8, 60))?
    } else {
        vec![mna::dc(&net)?]
    };
    let ms = start.elapsed().as_secs_f64() * 1e3;
    let fit = |v: f32| if v.is_finite() { v.clamp(CANVAS.0, CANVAS.1) } else { CANVAS.0 };
    Ok(scene::build(&net, &states, fit(width), fit(height), ms))
}

pub fn netlist(input: &[PartIn]) -> Result<Netlist, String> {
    if input.is_empty() {
        return Err("empty schematic: drag parts onto the grid".into());
    }
    if input.len() > MAX_PARTS {
        return Err(format!("more than {MAX_PARTS} parts"));
    }
    let mut nodes = vec![GROUND.to_string()];
    let mut ids = HashMap::from([(GROUND.to_string(), 0)]);
    let mut node = |s: &str| {
        let next = nodes.len();
        *ids.entry(s.to_string()).or_insert_with(|| {
            nodes.push(s.to_string());
            next
        })
    };
    let mut names = HashSet::new();
    let mut parts = Vec::with_capacity(input.len());

    for p in input {
        let err = |m: &str| Err(format!("{}: {m}", p.name));
        let (from, to) = (node(&p.from), node(&p.to));
        let ctrl = match (p.kind, p.ctrl.as_slice()) {
            (Kind::E | Kind::G, [a, b]) => Ctrl::Nodes(node(a), node(b)),
            (Kind::E | Kind::G, _) => return err("needs two control nets"),
            (Kind::F | Kind::H, [_]) => Ctrl::None, // resolved below
            (Kind::F | Kind::H, _) => return err("needs a part to sense"),
            _ => Ctrl::None,
        };
        if !p.value.is_finite() {
            return err("value not finite");
        }
        if p.kind == Kind::R && p.value == 0.0 {
            return err("0 Ω is a short");
        }
        if p.kind == Kind::C && p.value <= 0.0 {
            return err("capacitance must be > 0");
        }
        if from == to {
            return err("both pins on one net");
        }
        if !names.insert(&p.name) {
            return err("duplicate name");
        }
        parts.push(Part { name: p.name.clone(), kind: p.kind, from, to, value: p.value, ctrl });
    }

    // F/H sense by name, resolved once all names are known
    let index: HashMap<&str, usize> = input.iter().enumerate().map(|(i, p)| (p.name.as_str(), i)).collect();
    for (i, (p, raw)) in parts.iter_mut().zip(input).enumerate() {
        if matches!(p.kind, Kind::F | Kind::H) {
            p.ctrl = match (raw.ctrl[0].as_str(), index.get(raw.ctrl[0].as_str())) {
                ("", _) => return Err(format!("{}: pick a part to sense", p.name)),
                (_, Some(&c)) if c != i => Ctrl::Part(c),
                (_, Some(_)) => return Err(format!("{}: cannot sense itself", p.name)),
                (name, None) => return Err(format!("{}: no part {name} to sense", p.name)),
            };
        }
    }

    // keep what ground reaches, controls included; repeat until stable
    let mut keep = vec![true; parts.len()];
    let reach = loop {
        let mut reach = vec![false; nodes.len()];
        reach[0] = true;
        let mut grew = true;
        while grew {
            grew = false;
            for p in parts.iter().zip(&keep).filter(|x| *x.1).map(|x| x.0) {
                if reach[p.from] != reach[p.to] {
                    (reach[p.from], reach[p.to], grew) = (true, true, true);
                }
            }
        }
        let next: Vec<bool> = parts
            .iter()
            .zip(&keep)
            .map(|(p, &k)| {
                k && reach[p.from]
                    && match p.ctrl {
                        Ctrl::Nodes(a, b) => reach[a] && reach[b],
                        Ctrl::Part(c) => keep[c],
                        Ctrl::None => true,
                    }
            })
            .collect();
        if next == keep {
            break reach;
        }
        keep = next;
    };
    if !keep.contains(&true) {
        return Err("no ground: place a GND and wire it in".into());
    }

    let (mut node_at, mut part_at) = (vec![0; nodes.len()], vec![0; parts.len()]);
    let nodes: Vec<String> = nodes
        .into_iter()
        .enumerate()
        .filter(|x| reach[x.0])
        .enumerate()
        .map(|(new, (old, n))| {
            node_at[old] = new;
            n
        })
        .collect();
    let mut skipped = vec![];
    let mut kept = vec![];
    for (i, p) in parts.into_iter().enumerate() {
        if keep[i] {
            part_at[i] = kept.len();
            kept.push(p);
        } else {
            skipped.push(p.name);
        }
    }
    for p in &mut kept {
        (p.from, p.to) = (node_at[p.from], node_at[p.to]);
        p.ctrl = match p.ctrl {
            Ctrl::Nodes(a, b) => Ctrl::Nodes(node_at[a], node_at[b]),
            Ctrl::Part(c) => Ctrl::Part(part_at[c]),
            Ctrl::None => Ctrl::None,
        };
    }
    Ok(Netlist { nodes, parts: kept, skipped })
}

#[cfg(test)]
pub(crate) fn part(name: &str, kind: Kind, from: &str, to: &str, value: f64) -> PartIn {
    PartIn { name: name.into(), kind, from: from.into(), to: to.into(), value, ctrl: vec![] }
}

#[cfg(test)]
mod tests {
    use super::{mna, netlist, part, Kind::*, PartIn};

    fn dc(parts: &[PartIn]) -> Result<(Vec<f64>, Vec<f64>), String> {
        mna::dc(&netlist(parts)?).map(|s| (s.volts, s.amps))
    }

    fn with(mut p: PartIn, ctrl: &[&str]) -> PartIn {
        p.ctrl = ctrl.iter().map(|s| s.to_string()).collect();
        p
    }

    const EPS: f64 = 1e-9;

    #[test]
    fn divider() {
        let (v, i) = dc(&[
            part("V1", V, "gnd", "A", 10.0),
            part("R1", R, "A", "B", 1e3),
            part("R2", R, "B", "gnd", 1e3),
        ])
        .unwrap();
        assert!((v[2] - 5.0).abs() < EPS);
        assert!((i[0] - 5e-3).abs() < 1e-12);
    }

    // node order: gnd, A (V1 +), B (out)
    #[test]
    fn dependent() {
        let src = || part("V1", V, "gnd", "A", 2.0);
        let load = || part("RL", R, "B", "gnd", 100.0);
        let sense = || part("RS", R, "A", "gnd", 1e3); // 2 mA
        let e = dc(&[src(), load(), with(part("E1", E, "gnd", "B", 3.0), &["A", "gnd"])]).unwrap();
        assert!((e.0[2] - 6.0).abs() < EPS);
        let g = dc(&[src(), load(), with(part("G1", G, "gnd", "B", 0.01), &["A", "gnd"])]).unwrap();
        assert!((g.0[2] - 2.0).abs() < EPS);
        let f = dc(&[src(), sense(), load(), with(part("F1", F, "gnd", "B", 10.0), &["RS"])]).unwrap();
        assert!((f.0[2] - 2.0).abs() < EPS);
        let h = dc(&[src(), sense(), load(), with(part("H1", H, "gnd", "B", 500.0), &["RS"])]).unwrap();
        assert!((h.0[2] - 1.0).abs() < EPS);
    }

    #[test]
    fn rc_charges() {
        let net = netlist(&[
            part("V1", V, "gnd", "A", 1.0),
            part("R1", R, "A", "B", 1e3),
            part("C1", C, "B", "gnd", 1e-6),
        ])
        .unwrap();
        let frames = mna::transient(&net, 40).unwrap();
        let tau = frames.iter().find(|s| s.time >= 1e-3).unwrap();
        assert!((tau.volts[2] - (1.0 - (-tau.time / 1e-3f64).exp())).abs() < 0.02);
        assert!((frames.last().unwrap().volts[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn rejects() {
        assert!(netlist(&[part("R1", R, "A", "A", 5.0)]).is_err());
        assert!(netlist(&[part("R1", R, "A", "B", 5.0)]).err().unwrap().contains("no ground"));
        assert!(dc(&[part("I1", I, "gnd", "A", 1.0)]).unwrap_err().contains("net A floating"));
        let lp = [part("V1", V, "gnd", "A", 1.0), part("V2", V, "gnd", "A", 2.0)];
        assert!(dc(&lp).unwrap_err().contains("loop"));
        let loose = netlist(&[part("R1", R, "gnd", "A", 1.0), part("R2", R, "B", "C", 1.0)]).unwrap();
        assert_eq!((loose.nodes.len(), loose.skipped.as_slice()), (2, &["R2".to_string()][..]));
        assert!(netlist(&[with(part("F1", F, "gnd", "A", 1.0), &["RX"])]).err().unwrap().contains("RX"));
        let open_ctrl = [part("R1", R, "gnd", "A", 1.0), with(part("E1", E, "gnd", "A", 1.0), &["X", "gnd"])];
        assert_eq!(netlist(&open_ctrl).unwrap().skipped, ["E1"]);
    }
}
