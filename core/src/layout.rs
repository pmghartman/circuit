// x = hops from ground, y = potential, edges as C-arcs
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::f32::consts::PI;

use crate::geom::{point, Cubic, P};
use crate::Netlist;

pub const NODE_R: f32 = 15.0;
pub const MARGIN: f32 = 44.0;
pub const SYMBOL_HALF: f32 = 14.0; // mirrored in ui/app.js
const ANCHOR_R: f32 = NODE_R - 3.0; // under fill, not stroke

pub const ROW_GAP: f32 = NODE_R * 2.0 + 30.0;
const COL_GAP: f32 = 120.0;
const MIN_COL_GAP: f32 = 92.0;
const NODE_GAP: f32 = NODE_R * 2.0 + 18.0;
const EXIT_GAP: f32 = 26.0 * PI / 180.0;

const PARALLEL_BOW: f32 = 54.0;
const SINGLE_BOW: f32 = 18.0;
const MAX_BOW_FRACTION: f32 = 0.34;
const MAX_BOW: f32 = 120.0;
const BOW_STEP: f32 = 12.0;

const HANDLE_FACTOR: f32 = 0.4;
const HANDLE: (f32, f32) = (34.0, 90.0);

const NODE_CLEAR: f32 = NODE_R + SYMBOL_HALF + 14.0;
const SYMBOL_CLEAR: f32 = SYMBOL_HALF * 2.0 + 16.0;
const WIRE_SYMBOL_CLEAR: f32 = SYMBOL_HALF + 20.0;
const WIRE_NODE_CLEAR: f32 = NODE_R + 12.0;

const PASSES: usize = 80;
const SLIDE_STEP: f32 = 0.05;
const SLIDES: usize = 7;

pub struct Layout {
    pub width: f32,
    pub height: f32,
    pub pos: Vec<P>,
    pub rows: BTreeMap<i64, f32>,   // mV key → y
    pub cols: BTreeMap<usize, f32>, // hops → x
    pub curves: Vec<Cubic>,
    pub center: Vec<f32>,
    pub hidden: Vec<bool>,
}

// equal-voltage grouping key
pub fn key(v: f64) -> i64 {
    (v * 1000.0).round() as i64
}

fn steps(n: usize) -> f32 {
    n.saturating_sub(1) as f32
}

// voltage → y, one per solve so transient frames share rows and height
pub struct Scale {
    pub height: f32,
    anchors: Vec<(f64, f32)>, // volts ascending
}

impl Scale {
    // rows for the start, settled and extreme voltages; between them y is linear in volts
    // one frame (DC): exactly the rows of its own voltages
    pub fn new(frames: &[&[f64]], height: f32) -> Scale {
        let ends = [frames.first(), frames.last()].into_iter().flatten();
        let mut keys: BTreeSet<i64> = ends.flat_map(|f| f.iter().map(|&v| key(v))).collect();
        let all = || frames.iter().flat_map(|f| f.iter().map(|&v| key(v)));
        keys.extend(all().min());
        keys.extend(all().max());
        // canvas grown to fit, viewBox scales down
        let height = height.max(MARGIN * 2.0 + steps(keys.len()) * ROW_GAP);
        let anchors = voltage_rows(&keys, height).into_iter().map(|(k, y)| (k as f64 / 1000.0, y)).collect();
        Scale { height, anchors }
    }

    pub fn y(&self, v: f64) -> f32 {
        let a = &self.anchors;
        match a.partition_point(|&(av, _)| av < v) {
            0 => a[0].1,
            i if i == a.len() => a[i - 1].1,
            i => {
                let ((v0, y0), (v1, y1)) = (a[i - 1], a[i]);
                y0 + (y1 - y0) * ((v - v0) / (v1 - v0)) as f32
            }
        }
    }
}

pub fn build(net: &Netlist, volts: &[f64], width: f32, scale: &Scale) -> Layout {
    let rank = ranks(net);
    let cols: BTreeSet<usize> = rank.iter().copied().collect();

    // canvas grown to fit, viewBox scales down
    let width = width.max(MARGIN * 2.0 + steps(cols.len()) * MIN_COL_GAP);
    let height = scale.height;

    let rows: BTreeMap<i64, f32> = volts.iter().map(|&v| (key(v), scale.y(key(v) as f64 / 1000.0))).collect();
    let cols = rank_columns(&cols, width);
    let mut pos: Vec<P> =
        (0..net.nodes.len()).map(|i| P { x: cols[&rank[i]], y: rows[&key(volts[i])] }).collect();
    separate_rows(volts, width, &mut pos);

    let angles = exit_angles(net, &pos);
    let (curves, center, hidden) = route(net, &pos, &angles);
    Layout { width, height, pos, rows, cols, curves, center, hidden }
}

// BFS from ground, ties bumped by higher index
fn ranks(net: &Netlist) -> Vec<usize> {
    let n = net.nodes.len();
    let mut adj = vec![vec![]; n];
    for p in &net.parts {
        adj[p.from].push(p.to);
        adj[p.to].push(p.from);
    }
    let mut rank = vec![usize::MAX; n];
    rank[0] = 0;
    let mut queue = VecDeque::from([0]);
    while let Some(u) = queue.pop_front() {
        for &v in &adj[u] {
            if rank[v] == usize::MAX {
                rank[v] = rank[u] + 1;
                queue.push_back(v);
            }
        }
    }
    for _ in 0..n + net.parts.len() {
        let mut changed = false;
        for p in net.parts.iter().filter(|p| p.from != 0 && p.to != 0) {
            if rank[p.from] == rank[p.to] {
                rank[p.from.max(p.to)] += 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    rank
}

// proportional rows blended toward even until min gap holds
// gaps linear in t: bisection for smallest blend
fn voltage_rows(keys: &BTreeSet<i64>, height: f32) -> BTreeMap<i64, f32> {
    let draw = (height - MARGIN * 2.0).max(1.0);
    let ks: Vec<i64> = keys.iter().rev().copied().collect();
    let count = ks.len();
    if count == 1 {
        return BTreeMap::from([(ks[0], MARGIN + draw / 2.0)]);
    }
    let v = |i: usize| ks[i] as f64 / 1000.0;
    let span = (v(0) - v(count - 1)).max(1e-9);
    let even_gap = draw / (count - 1) as f32;
    let target = ROW_GAP.min(even_gap);

    let blend = |i: usize, t: f32| {
        let prop = ((v(0) - v(i)) / span) as f32 * draw;
        prop + (even_gap * i as f32 - prop) * t
    };
    let smallest = |t: f32| (1..count).map(|i| blend(i, t) - blend(i - 1, t)).fold(f32::INFINITY, f32::min);

    let mut t = 0.0;
    if smallest(0.0) < target {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if smallest(mid) < target {
                lo = mid
            } else {
                hi = mid
            }
        }
        t = hi;
    }
    (0..count).map(|i| (ks[i], MARGIN + blend(i, t))).collect()
}

fn rank_columns(cols: &BTreeSet<usize>, width: f32) -> BTreeMap<usize, f32> {
    let draw = (width - MARGIN * 2.0).max(1.0);
    let (lo, hi) = (*cols.first().unwrap(), *cols.last().unwrap());
    if lo == hi {
        return BTreeMap::from([(lo, MARGIN + draw / 2.0)]);
    }
    let span = (hi - lo) as f32;
    let gap = COL_GAP.min(draw / span);
    let start = MARGIN + (draw - span * gap) / 2.0;
    cols.iter().map(|&r| (r, start + (r - lo) as f32 * gap)).collect()
}

// same row and column: push apart in x, recentre, stay in margins
fn separate_rows(volts: &[f64], width: f32, pos: &mut [P]) {
    let mut rows: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (i, &v) in volts.iter().enumerate() {
        rows.entry(key(v)).or_default().push(i);
    }
    let (left, right) = (MARGIN, (width - MARGIN).max(MARGIN));
    for mut m in rows.into_values().filter(|m| m.len() > 1) {
        m.sort_by(|&a, &b| pos[a].x.total_cmp(&pos[b].x).then(a.cmp(&b)));
        let mut pushed = vec![pos[m[0]].x];
        for &i in &m[1..] {
            pushed.push(pos[i].x.max(pushed.last().unwrap() + NODE_GAP));
        }
        if m.iter().zip(&pushed).all(|(&i, &x)| x <= pos[i].x) {
            continue;
        }
        let (first, last) = (pushed[0], *pushed.last().unwrap());
        let mut shift = (pos[m[0]].x + pos[*m.last().unwrap()].x - first - last) / 2.0;
        if first + shift < left {
            shift = left - first;
        }
        if last + shift > right {
            shift = right - last;
        }
        for (&i, x) in m.iter().zip(pushed) {
            pos[i].x = (x + shift).clamp(left, right);
        }
    }
}

// per part [from, to] exit angle, aimed at the other node
// spread per hemisphere against bunching
fn exit_angles(net: &Netlist, pos: &[P]) -> Vec<[f32; 2]> {
    let mut by_node = vec![vec![]; net.nodes.len()];
    for (e, p) in net.parts.iter().enumerate() {
        by_node[p.from].push((e, 0, (pos[p.to] - pos[p.from]).angle()));
        by_node[p.to].push((e, 1, (pos[p.from] - pos[p.to]).angle()));
    }
    const EPS: f32 = 0.001;
    let mut out = vec![[0.0; 2]; net.parts.len()];
    for inc in by_node {
        let (below, above): (Vec<_>, Vec<_>) = inc.into_iter().partition(|i| i.2 >= 0.0);
        spread(below, EPS, PI - EPS, &mut out);
        spread(above, -PI + EPS, -EPS, &mut out);
    }
    out
}

fn spread(mut g: Vec<(usize, usize, f32)>, lo: f32, hi: f32, out: &mut [[f32; 2]]) {
    if g.len() == 1 {
        out[g[0].0][g[0].1] = g[0].2;
    }
    if g.len() < 2 {
        return;
    }
    g.sort_by(|a, b| a.2.total_cmp(&b.2));
    let n = g.len() as f32;
    let gap = EXIT_GAP.min((hi - lo) / n);
    let mut pushed = vec![g[0].2];
    for i in &g[1..] {
        pushed.push(i.2.max(pushed.last().unwrap() + gap));
    }
    let adjusted: Vec<f32> = if *pushed.last().unwrap() > hi {
        (0..g.len()).map(|i| lo + (hi - lo) / n * (i as f32 + 0.5)).collect()
    } else {
        let shift = (g[0].2 + g[g.len() - 1].2 - pushed[0] - pushed.last().unwrap()) / 2.0;
        pushed.iter().map(|a| (a + shift).clamp(lo, hi)).collect()
    };
    for (i, a) in g.iter().zip(adjusted) {
        out[i.0][i.1] = a;
    }
}

struct Router<'a> {
    pos: &'a [P],
    ends: Vec<(usize, usize)>,
    anchor: Vec<[P; 2]>,
    bow_dir: Vec<P>,
    bow: Vec<f32>,
    cap: Vec<f32>,
    handle: Vec<f32>,
    center: Vec<f32>,
}

impl Router<'_> {
    // controls along the chord, offset sideways: an arc by construction
    fn curve(&self, e: usize) -> Cubic {
        let [p0, p3] = self.anchor[e];
        let along = (p3 - p0).unit() * self.handle[e];
        let out = self.bow_dir[e] * self.bow[e];
        [p0, p0 + along + out, p3 - along + out, p3]
    }

    fn symbol(&self, e: usize) -> P {
        point(&self.curve(e), self.center[e])
    }

    fn symbol_blocked(&self, e: usize, s: P) -> bool {
        self.pos.iter().any(|&n| s.near(n, NODE_CLEAR))
            || (0..self.ends.len()).any(|o| o != e && s.near(self.symbol(o), SYMBOL_CLEAR))
    }

    // symbol clash or wire crowding foreign node/symbol
    fn blocked(&self, e: usize) -> bool {
        if self.symbol_blocked(e, self.symbol(e)) {
            return true;
        }
        let c = self.curve(e);
        let samples: Vec<P> = (0..8).map(|k| point(&c, 0.15 + 0.1 * k as f32)).collect();
        let hits = |q: P, r: f32| samples.iter().any(|p| p.near(q, r));
        let (f, t) = self.ends[e];
        self.pos.iter().enumerate().any(|(i, &n)| i != f && i != t && hits(n, WIRE_NODE_CLEAR))
            || (0..self.ends.len()).any(|o| o != e && hits(self.symbol(o), WIRE_SYMBOL_CLEAR))
    }

    // smallest off-centre slide that frees the symbol
    fn slide(&mut self, e: usize) -> bool {
        let prev = self.center[e];
        for k in 1..=SLIDES {
            let d = k as f32 * SLIDE_STEP;
            for c in [0.5 + d, 0.5 - d] {
                self.center[e] = c;
                if !self.symbol_blocked(e, self.symbol(e)) {
                    return c != prev;
                }
            }
        }
        self.center[e] = 0.5;
        false
    }

    // bow away from whatever it crowds
    fn grow(&mut self, e: usize) -> bool {
        let ok = self.bow[e].abs() + BOW_STEP <= self.cap[e];
        if ok {
            self.bow[e] += if self.bow[e] < 0.0 { -BOW_STEP } else { BOW_STEP };
        }
        ok
    }
}

fn route(net: &Netlist, pos: &[P], angles: &[[f32; 2]]) -> (Vec<Cubic>, Vec<f32>, Vec<bool>) {
    let m = net.parts.len();
    let ends: Vec<(usize, usize)> = net.parts.iter().map(|p| (p.from, p.to)).collect();
    let anchor: Vec<[P; 2]> = ends
        .iter()
        .zip(angles)
        .map(|(&(f, t), a)| [pos[f] + P::polar(a[0]) * ANCHOR_R, pos[t] + P::polar(a[1]) * ANCHOR_R])
        .collect();
    let handle = anchor
        .iter()
        .map(|[a, b]| {
            let d = (*b - *a).len();
            (d * HANDLE_FACTOR).clamp(HANDLE.0, HANDLE.1).min(d * 0.5)
        })
        .collect();

    // parallel parts fanned out, side from exit spread
    let mut groups: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (e, &(f, t)) in ends.iter().enumerate() {
        groups.entry((f.min(t), f.max(t))).or_default().push(e);
    }
    let (mut bow_dir, mut bow, mut cap) = (vec![P::default(); m], vec![0.0; m], vec![0.0; m]);
    for ((a, b), mut g) in groups {
        let chord = pos[b] - pos[a];
        let len = chord.len();
        let perp = if len > 1e-4 { chord.perp() * (1.0 / len) } else { P { x: 0.0, y: 1.0 } };
        let mid = (pos[a] + pos[b]) * 0.5;
        let side = |e: usize| perp.dot((anchor[e][0] + anchor[e][1]) * 0.5 - mid);
        g.sort_by(|&x, &y| side(x).total_cmp(&side(y)));

        let limit = if len > 1e-4 { len * MAX_BOW_FRACTION } else { MAX_BOW };
        let n = g.len();
        for (i, &e) in g.iter().enumerate() {
            let amount = if n == 1 {
                SINGLE_BOW.copysign(side(e))
            } else {
                PARALLEL_BOW * (i as f32 - (n - 1) as f32 / 2.0)
            };
            bow_dir[e] = perp;
            bow[e] = amount.clamp(-limit, limit);
            cap[e] = limit;
        }
    }

    let mut r = Router { pos, ends, anchor, bow_dir, bow, cap, handle, center: vec![0.5; m] };
    // slide first, bow further second
    for _ in 0..PASSES {
        let mut changed = false;
        for e in 0..m {
            if r.blocked(e) && (r.slide(e) || r.grow(e)) {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // nowhere clear: draw the wire unbroken
    let hidden = (0..m).map(|e| r.symbol_blocked(e, r.symbol(e))).collect();
    ((0..m).map(|e| r.curve(e)).collect(), r.center, hidden)
}
