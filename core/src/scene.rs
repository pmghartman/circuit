// render model in screen coords; no colors, ids stable across solves
use serde::Serialize;

use crate::geom::{point, split, tangent, Cubic, P};
use crate::layout::{self, MARGIN, NODE_R, SYMBOL_HALF};
use crate::mna::State;
use crate::{Ctrl, Kind, Netlist};

const MIN_SYMBOL_SPAN: f32 = 56.0;
const LABEL_OFFSET: f32 = 22.0;
const LABEL_GAP: f32 = 15.0;
const CURRENT_EPS: f64 = 1e-6;

// one frame for DC, many for a transient
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sim {
    elapsed_ms: f64,
    skipped: Vec<String>,
    frames: Vec<Scene>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Scene {
    time: f64,
    width: f64,
    height: f64,
    node_radius: f64,
    lines: Vec<Line>,
    cols: Vec<[f64; 2]>, // [hops, x]
    edges: Vec<Edge>,
    nodes: Vec<Node>,
}

#[derive(Serialize)]
struct Line {
    voltage: f64,
    label: String,
    y: f64,
    x1: f64,
    x2: f64,
}

#[derive(Serialize)]
struct Edge {
    id: String,
    kind: char,
    value: f64,
    ctrl: Vec<String>, // control nets or sensed part
    from: String,
    to: String,
    amps: f64, // from→to
    drop: f64, // v(from) - v(to)
    wires: Vec<[f64; 8]>,
    symbol: Option<Place>,
    arrow: Option<Place>,
    mid: [f64; 2],
    tip: [f64; 2],
}

#[derive(Serialize)]
struct Place {
    x: f64,
    y: f64,
    angle: f64,
}

#[derive(Serialize)]
struct Node {
    id: String,
    ground: bool,
    volts: f64,
    x: f64,
    y: f64,
}

// 0.01 px precision
fn px(v: f32) -> f64 {
    (v as f64 * 100.0).round() / 100.0
}

fn xy(p: P) -> [f64; 2] {
    [px(p.x), px(p.y)]
}

fn place(p: P, radians: f32) -> Place {
    Place { x: px(p.x), y: px(p.y), angle: px(radians.to_degrees()) }
}

fn wire(c: &Cubic) -> [f64; 8] {
    let [a, b, c, d] = c.map(xy);
    [a[0], a[1], b[0], b[1], c[0], c[1], d[0], d[1]]
}

fn trim(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').into()
    } else {
        s
    }
}

pub fn build(net: &Netlist, states: &[State], width: f32, height: f32, ms: f64) -> Sim {
    let volts: Vec<&[f64]> = states.iter().map(|s| s.volts.as_slice()).collect();
    let scale = layout::Scale::new(&volts, height);
    Sim {
        elapsed_ms: ms,
        skipped: net.skipped.clone(),
        frames: states.iter().map(|s| frame(net, s, width, &scale)).collect(),
    }
}

fn frame(net: &Netlist, s: &State, width: f32, scale: &layout::Scale) -> Scene {
    let (volts, amps) = (&s.volts, &s.amps);
    let g = layout::build(net, volts, width, scale);

    let mut last = f32::NEG_INFINITY;
    let lines = g
        .rows
        .iter()
        .map(|(&k, &y)| {
            let voltage = k as f64 / 1000.0;
            let labelled = (y - last).abs() >= LABEL_GAP;
            if labelled {
                last = y;
            }
            Line {
                voltage,
                label: if labelled { format!("{}V", trim(voltage, 2)) } else { String::new() },
                y: px(y),
                x1: px(MARGIN),
                x2: px((g.width - MARGIN).max(MARGIN)),
            }
        })
        .collect();

    let edges = net
        .parts
        .iter()
        .enumerate()
        .map(|(e, p)| {
            let (c, t) = (&g.curves[e], g.center[e]);
            let (vf, vt) = (volts[p.from], volts[p.to]);
            // glyph faces + or current direction
            let flipped = match p.kind {
                Kind::R | Kind::C => vf < vt,
                Kind::V | Kind::E | Kind::H => vf > vt,
                Kind::I | Kind::F | Kind::G => amps[e] < 0.0,
            };
            let ctrl = match p.ctrl {
                Ctrl::Nodes(a, b) => vec![net.nodes[a].clone(), net.nodes[b].clone()],
                Ctrl::Part(c) => vec![net.parts[c].name.clone()],
                Ctrl::None => vec![],
            };
            let chord = c[3] - c[0];
            let len = chord.len();

            let (wires, symbol, mid) = if len < MIN_SYMBOL_SPAN || g.hidden[e] {
                (vec![wire(c)], None, point(c, t))
            } else {
                // cap keeps leads on a slid symbol
                let half = (SYMBOL_HALF / len).min(t.min(1.0 - t) - 0.05);
                let (a, b) = split(c, t - half, t + half);
                let mid = (a[3] + b[0]) * 0.5;
                let angle = tangent(c, t).angle() + if flipped { std::f32::consts::PI } else { 0.0 };
                (vec![wire(&a), wire(&b)], Some(place(mid, angle)), mid)
            };

            // arrow at whichever end current flows into
            let arrow = (amps[e].abs() > CURRENT_EPS).then(|| {
                let back = amps[e] < 0.0;
                let dir = tangent(c, if back { 0.0 } else { 1.0 }) * if back { -1.0 } else { 1.0 };
                place(if back { c[0] } else { c[3] }, dir.angle())
            });

            Edge {
                id: p.name.clone(),
                kind: format!("{:?}", p.kind).chars().next().unwrap(),
                value: p.value,
                ctrl,
                from: net.nodes[p.from].clone(),
                to: net.nodes[p.to].clone(),
                amps: amps[e],
                drop: vf - vt,
                wires,
                symbol,
                arrow,
                mid: xy(mid),
                tip: xy(mid + chord.unit().perp() * LABEL_OFFSET),
            }
        })
        .collect();

    let nodes = net
        .nodes
        .iter()
        .zip(&g.pos)
        .enumerate()
        .map(|(i, (id, p))| Node { id: id.clone(), ground: i == 0, volts: volts[i], x: px(p.x), y: px(p.y) })
        .collect();

    Scene {
        time: s.time,
        width: px(g.width),
        height: px(g.height),
        node_radius: px(NODE_R),
        lines,
        cols: g.cols.iter().map(|(&r, &x)| [r as f64, px(x)]).collect(),
        edges,
        nodes,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn frames() {
        use crate::{part, Kind::*};
        let dc = [
            part("V1", V, "gnd", "A", 12.0),
            part("R1", R, "A", "B", 100.0),
            part("R2", R, "B", "gnd", 100.0),
        ];
        let s = crate::run(&dc, 900.0, 640.0).unwrap();
        assert_eq!(s.frames.len(), 1);
        assert_eq!((s.frames[0].nodes.len(), s.frames[0].edges.len()), (3, 3));
        assert!(s.frames[0].edges.iter().all(|e| e.arrow.is_some()));
        let rc =
            [part("V1", V, "gnd", "A", 1.0), part("R1", R, "A", "B", 1e3), part("C1", C, "B", "gnd", 1e-6)];
        let t = crate::run(&rc, 900.0, 640.0).unwrap();
        assert!(t.frames.len() > 8);
        // shared rows: fixed canvas, charging node heads straight for its settled row
        let f = &t.frames;
        assert!(f.iter().all(|s| s.height == f[0].height));
        let b = |s: &super::Scene| s.nodes.iter().find(|n| n.id == "B").unwrap().y;
        let gap = |s| (b(s) - b(f.last().unwrap())).abs();
        assert!(f.windows(2).all(|w| gap(&w[1]) <= gap(&w[0])));
    }
}
