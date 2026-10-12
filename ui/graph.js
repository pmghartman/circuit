// DAG frames → svg: tween, transient playback, current flow, cross-highlight
const NS = 'http://www.w3.org/2000/svg';
const SYM = { half: 14, r: 11 }; // half mirrored in core/src/layout.rs
const HOVER = 26, TWEEN = 420, FLOW = 0.8, PLAY = 4000;

export const el = (tag, attrs = {}, ...kids) => {
    const n = document.createElementNS(NS, tag);
    for (const k in attrs) if (attrs[k] !== undefined) n.setAttribute(k, attrs[k]);
    n.append(...kids);
    return n;
};

const PREFIX = [['G', 1e9], ['M', 1e6], ['k', 1e3], ['', 1], ['m', 1e-3], ['µ', 1e-6], ['n', 1e-9], ['p', 1e-12]];
export function si(v, unit = '') {
    if (!v) return `0${unit}`;
    const [p, m] = PREFIX.find(([, m]) => Math.abs(v) >= m * 0.9995) ?? PREFIX.at(-1);
    return `${+(v / m).toPrecision(3)}${p}${unit}`;
}

export const UNIT = { R: 'Ω', C: 'F', V: 'V', I: 'A', E: 'V/V', F: 'A/A', G: 'S', H: 'Ω' };
export function describe(kind, value, ctrl = []) {
    const v = kind === 'E' || kind === 'F' ? `×${+value.toPrecision(3)}` : si(value, UNIT[kind]);
    if (!ctrl.length) return v;
    return kind === 'E' || kind === 'G' ? `${v}·v(${ctrl.join('−')})` : `${v}·I(${ctrl[0]})`;
}

// drawn along +x at origin, placement rotates
const { half: h, r } = SYM;
const zigzag = `M${-h},0L${-h + 3.7},-8L${-h + 7.3},8L0,-8L${h - 7.3},8L${h - 3.7},-8L${h},0`;
const shell = (shape, d) => [el('path', { class: 'sym body', d: shape }), el('path', { class: 'sym', d })];
const RING = `M${-r},0a${r},${r} 0 1,0 ${2 * r},0a${r},${r} 0 1,0 ${-2 * r},0`;
const DIAMOND = `M${-r - 1},0L0,${-r - 1}L${r + 1},0L0,${r + 1}Z`;
const PLUS = 'M-6,0H-1.6M1.6,0H6M3.8,-3.3V3.3'; // + on local +x
const ARROW = 'M-6,0H6M2,-3L6,0L2,3';
const glyph = {
    R: () => [el('path', { class: 'sym', d: zigzag })],
    C: () => [el('path', { class: 'sym', d: `M${-h},0H-3M-3,-9V9M3,-9V9M3,0H${h}` })],
    V: () => shell(RING, PLUS),
    I: () => shell(RING, ARROW),
    E: () => shell(DIAMOND, PLUS),
    H: () => shell(DIAMOND, PLUS),
    F: () => shell(DIAMOND, ARROW),
    G: () => shell(DIAMOND, ARROW),
};
const at = (p) => `translate(${p.x} ${p.y}) rotate(${p.angle})`;
const curve = (w) => `M${w[0]},${w[1]}C${w[2]},${w[3]} ${w[4]},${w[5]} ${w[6]},${w[7]}`;

let svg, ui, emit = () => {};
let frames = [], pos = 0, playing = false, shown = null, hot = null, raf = 0, clock = 0;

export function init(target, controls, onHover) {
    [svg, ui, emit] = [target, controls, onHover];
    svg.onpointermove = move;
    svg.onpointerleave = () => hot && emit(null);
    ui.play.onclick = () => (playing ? pause() : play(pos >= frames.length - 1 ? 0 : pos));
    ui.scrub.oninput = () => {
        pause();
        pos = (ui.scrub.value / 1000) * (frames.length - 1);
        draw(frameAt(pos));
    };
}

export function highlight(h) {
    hot = h;
    if (shown) draw(shown);
}

export const stale = (on) => svg.classList.toggle('stale', on);

// part hot: edge, its nodes, what it senses; net hot: node and its edges
function lit(s) {
    const edges = new Set(), nodes = new Set(), ctl = new Set();
    for (const e of s.edges) {
        if (hot?.part === e.id || hot?.net === e.from || hot?.net === e.to) edges.add(e.id);
        if (hot?.part !== e.id) continue;
        nodes.add(e.from).add(e.to);
        e.ctrl.forEach((c) => ctl.add(c));
    }
    if (hot?.net) nodes.add(hot.net);
    return { edges, nodes, ctl };
}

function tooltip(s) {
    const e = hot?.part && s.edges.find((e) => e.id === hot.part);
    const n = hot?.net && s.nodes.find((n) => n.id === hot.net);
    if (!e && !n) return [];
    const text = e
        ? `${e.id} ${describe(e.kind, e.value, e.ctrl)}  I ${si(Math.abs(e.amps), 'A')}  ΔV ${si(Math.abs(e.drop), 'V')}  P ${si(Math.abs(e.amps * e.drop), 'W')}`
        : `${n.ground ? 'GND' : n.id}  ${si(n.volts, 'V')}  ${s.edges.filter((e) => e.from === n.id || e.to === n.id).length} parts`;
    const [x, y] = e ? e.tip : [n.x, n.y - s.nodeRadius - 16];
    const w = text.length * 6.1 + 18;
    return [el('g', { class: 'tip' },
        el('rect', { x: x - w / 2, y: y - 11, width: w, height: 22 }),
        el('text', { x, y }, text))];
}

function draw(s) {
    shown = s;
    for (const k of ['width', 'height']) svg.setAttribute(k, s[k]);
    svg.setAttribute('viewBox', `0 0 ${s.width} ${s.height}`);
    const { edges, nodes, ctl } = lit(s);
    const phase = `animation-delay:-${((performance.now() / 1000) % FLOW).toFixed(3)}s`;
    const x0 = s.lines[0]?.x1 ?? 44;

    const axes = [
        el('text', { class: 'axis', x: x0 - 6, y: 14 }, 'hops'),
        ...s.cols.map(([hop, x]) => el('text', { class: 'axis hop', x, y: 14 }, hop)),
    ];
    const rows = s.lines.map((l) => el('g', { class: 'row' },
        el('line', { x1: l.x1, x2: l.x2, y1: l.y, y2: l.y }),
        el('text', { x: l.x1 - 6, y: l.y }, l.label)));
    const parts = s.edges.map((e) => el('g', { class: `edge k-${e.kind}${edges.has(e.id) ? ' hot' : ''}${ctl.has(e.id) ? ' ctl' : ''}` },
        ...e.wires.map((w) => el('path', { class: 'wire', d: curve(w) })),
        ...(e.arrow ? e.wires.map((w) => el('path', { class: e.amps < 0 ? 'flow rev' : 'flow', d: curve(w), style: phase })) : []),
        ...(e.symbol ? [el('g', { transform: at(e.symbol) }, ...glyph[e.kind]())] : []),
        ...(e.arrow ? [el('path', { class: 'arrow', d: 'M0,0L-10,4.5V-4.5Z', transform: at(e.arrow) })] : [])));
    const dots = s.nodes.map((n) => {
        const cls = ['node', n.ground && 'gnd', nodes.has(n.id) && 'hot', ctl.has(n.id) && 'ctl'].filter(Boolean).join(' ');
        return el('g', { class: cls },
            el('circle', { cx: n.x, cy: n.y, r: s.nodeRadius }),
            el('text', { x: n.x, y: n.y }, n.ground ? 'GND' : n.id),
            el('text', { class: 'volts', x: n.x, y: n.y + s.nodeRadius + 10 }, si(n.volts, 'V')));
    });
    svg.replaceChildren(...axes, ...rows, ...parts, ...dots, ...tooltip(s));
    if (frames.length > 1) {
        ui.scrub.value = Math.round((pos / (frames.length - 1)) * 1000);
        ui.clock.textContent = `t ${si(s.time, 's')} / ${si(frames.at(-1).time, 's')}`;
    }
}

const lerp = (a, b, t) => a + (b - a) * t;
const turn = (a, b, t) => a + ((((b - a + 180) % 360) + 360) % 360 - 180) * t; // shortest way round
const place = (a, b, t) => (a && b ? { x: lerp(a.x, b.x, t), y: lerp(a.y, b.y, t), angle: turn(a.angle, b.angle, t) } : b);
const pair = (a, b, t) => [lerp(a[0], b[0], t), lerp(a[1], b[1], t)];
const byKey = (list, k) => new Map(list.map((x) => [x[k], x]));

function mix(a, b, t) {
    const L = byKey(a.lines, 'voltage'), E = byKey(a.edges, 'id'), N = byKey(a.nodes, 'id'), C = new Map(a.cols);
    return {
        ...b,
        time: lerp(a.time, b.time, t),
        width: lerp(a.width, b.width, t),
        height: lerp(a.height, b.height, t),
        cols: b.cols.map(([hop, x]) => [hop, C.has(hop) ? lerp(C.get(hop), x, t) : x]),
        lines: b.lines.map((l) => {
            const p = L.get(l.voltage);
            return p ? { ...l, y: lerp(p.y, l.y, t), x1: lerp(p.x1, l.x1, t), x2: lerp(p.x2, l.x2, t) } : l;
        }),
        // split changed: no wire correspondence, snap
        edges: b.edges.map((e) => {
            const p = E.get(e.id);
            return p?.wires.length === e.wires.length ? {
                ...e,
                amps: lerp(p.amps, e.amps, t),
                drop: lerp(p.drop, e.drop, t),
                wires: e.wires.map((w, i) => w.map((v, j) => lerp(p.wires[i][j], v, t))),
                symbol: place(p.symbol, e.symbol, t),
                arrow: place(p.arrow, e.arrow, t),
                mid: pair(p.mid, e.mid, t),
                tip: pair(p.tip, e.tip, t),
            } : e;
        }),
        nodes: b.nodes.map((n) => {
            const p = N.get(n.id);
            return p ? { ...n, volts: lerp(p.volts, n.volts, t), x: lerp(p.x, n.x, t), y: lerp(p.y, n.y, t) } : n;
        }),
    };
}

// fractional frame index → interpolated scene
function frameAt(f) {
    const i = Math.min(Math.floor(f), frames.length - 2);
    return i < 0 ? frames[0] : mix(frames[i], frames[i + 1], f - i);
}

function tween(from, to, done) {
    cancelAnimationFrame(raf);
    if (!from || matchMedia('(prefers-reduced-motion: reduce)').matches) return draw(to), done?.();
    const t0 = performance.now();
    const ease = (t) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
    const step = (now) => {
        const t = Math.min((now - t0) / TWEEN, 1);
        draw(t < 1 ? mix(from, to, ease(t)) : to);
        if (t < 1) raf = requestAnimationFrame(step);
        else done?.();
    };
    raf = requestAnimationFrame(step);
}

function play(from) {
    pause();
    [pos, playing] = [from, true];
    ui.play.textContent = '❚❚';
    let last = performance.now();
    const step = (now) => {
        pos = Math.min(pos + ((now - last) / PLAY) * (frames.length - 1), frames.length - 1);
        last = now;
        draw(frameAt(pos));
        if (pos < frames.length - 1 && playing) clock = requestAnimationFrame(step);
        else pause();
    };
    clock = requestAnimationFrame(step);
}

function pause() {
    cancelAnimationFrame(clock);
    playing = false;
    ui.play.textContent = '▶';
}

// restart: new circuit, replay from t = 0; else hold position
export function present(sim, { animate, restart }) {
    const [was, resume] = [frames.length, playing];
    pause();
    frames = sim.frames;
    ui.bar.hidden = frames.length < 2;
    if (frames.length < 2) return tween(animate && shown, frames[0]);
    if (!restart && was === frames.length) return resume ? play(pos) : draw(frameAt(pos));
    pos = 0;
    tween(animate && shown, frames[0], () => play(0));
}

// nodes before part midpoints
function move(ev) {
    const ctm = svg.getScreenCTM();
    if (!shown || !ctm) return;
    const p = new DOMPoint(ev.clientX, ev.clientY).matrixTransform(ctm.inverse());
    const d2 = (x, y) => (p.x - x) ** 2 + (p.y - y) ** 2;
    const node = shown.nodes.find((n) => d2(n.x, n.y) <= (shown.nodeRadius + 4) ** 2);
    let best = HOVER * HOVER, part = null;
    for (const e of shown.edges) {
        const d = d2(...e.mid);
        if (d <= best) [best, part] = [d, e.id];
    }
    const next = node ? { net: node.id } : part ? { part } : null;
    if (next?.net !== hot?.net || next?.part !== hot?.part) emit(next);
}
