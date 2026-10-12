// LTSpice-style schematic: place, wire, move by drag or click; nets derived locally
import { el, si, describe, UNIT } from './graph.js';

const G = 16; // px per grid step
const TWO = [[0, 2], [0, -2]]; // [from, to], + on top
const FOUR = [...TWO, [-2, -1], [-2, 1]]; // + [ctrl+, ctrl-]
const LEADS = 'M0,-32V-12M0,12V32';
const PLUS = 'M-4,-5H4M0,-9V-1M-4,6H4';
const ARROW = 'M0,7V-7M-3.5,-2.5L0,-7L3.5,-2.5';
const SENSE = 'M-32,-16H-20L-9,-4M-32,16H-20L-9,4M-28,-22H-24M-26,-24V-20M-28,22H-24';
const RING = 'M-12,0a12,12 0 1,0 24,0a12,12 0 1,0 -24,0';
const DIAMOND = 'M0,-12L12,0L0,12L-12,0Z';
const KINDS = {
    R: { label: 'res', key: 'r', value: 1000, pins: TWO, d: 'M0,-32V-16L6,-13L-6,-8L6,-3L-6,3L6,8L-6,13L0,16V32' },
    C: { label: 'cap', key: 'c', value: 1e-6, pins: TWO, d: 'M0,-32V-4M-11,-4H11M-11,4H11M0,4V32' },
    V: { label: 'vs', key: 'v', value: 5, pins: TWO, shell: RING, d: LEADS + PLUS },
    I: { label: 'IS', key: 'i', value: 1e-3, pins: TWO, shell: RING, d: LEADS + ARROW },
    E: { label: 'vcvs', key: 'E', value: 2, pins: FOUR, shell: DIAMOND, d: LEADS + PLUS + SENSE },
    F: { label: 'cccs', key: 'F', value: 2, pins: TWO, shell: DIAMOND, d: LEADS + ARROW },
    G: { label: 'vccs', key: 'G', value: 1e-3, pins: FOUR, shell: DIAMOND, d: LEADS + ARROW + SENSE },
    H: { label: 'ccvs', key: 'H', value: 1000, pins: TWO, shell: DIAMOND, d: LEADS + PLUS },
    GND: { label: 'gnd', key: 'g', value: 0, pins: [[0, 0]], d: 'M0,0V8M-10,8H10M-6,12H6M-2,16H2' },
};
const SENSES = (k) => k === 'F' || k === 'H';
const MOD = /Mac/.test(navigator.platform) ? '⌘' : 'ctrl';
const STORE = 'circuit.schematic';

// starter circuits: parts are [id, kind, x, y, rot, value, sensed?], wires are [a, b]
const circuit = (parts, wires) => ({
    parts: parts.map(([id, kind, x, y, rot, value, by]) => ({ id, kind, x, y, rot, value, ...(by && { by }) })),
    wires: wires.map(([a, b], i) => ({ id: i + 1, a, b })),
});
const CIRCUITS = {
    // R1/R3 = R2/R4, so no current through the galvanometer R5
    'wheatstone bridge': circuit([
        ['V1', 'V', 0, 6, 0, 10], ['R1', 'R', 6, 3, 0, 1000], ['R2', 'R', 14, 3, 0, 2000],
        ['R3', 'R', 6, 9, 0, 1500], ['R4', 'R', 14, 9, 0, 3000], ['R5', 'R', 10, 6, 1, 100], ['GND1', 'GND', 0, 12, 0, 0],
    ], [
        [[0, 4], [0, 0]], [[0, 0], [14, 0]], [[6, 0], [6, 1]], [[14, 0], [14, 1]], [[6, 5], [6, 7]], [[14, 5], [14, 7]],
        [[6, 6], [8, 6]], [[12, 6], [14, 6]], [[6, 11], [6, 12]], [[14, 11], [14, 12]], [[0, 8], [0, 12]], [[0, 12], [14, 12]],
    ]),
    'voltage divider': circuit([
        ['V1', 'V', 0, 4, 0, 9], ['R1', 'R', 6, 2, 0, 10e3], ['R2', 'R', 6, 6, 0, 4700], ['GND1', 'GND', 0, 8, 0, 0],
    ], [
        [[0, 2], [0, 0]], [[0, 0], [6, 0]], [[0, 6], [0, 8]], [[0, 8], [6, 8]],
    ]),
    'current divider': circuit([
        ['I1', 'I', 0, 4, 0, 0.01], ['R1', 'R', 4, 4, 0, 1000], ['R2', 'R', 8, 4, 0, 4000], ['GND1', 'GND', 0, 6, 0, 0],
    ], [
        [[0, 2], [8, 2]], [[0, 6], [8, 6]],
    ]),
    // tau = RC = 1 s
    'rc charging': circuit([
        ['V1', 'V', 0, 4, 0, 5], ['R1', 'R', 4, 0, 1, 10e3], ['C1', 'C', 8, 4, 0, 100e-6], ['GND1', 'GND', 0, 8, 0, 0],
    ], [
        [[0, 2], [0, 0]], [[0, 0], [2, 0]], [[6, 0], [8, 0]], [[8, 0], [8, 2]], [[0, 6], [0, 8]], [[0, 8], [8, 8]], [[8, 6], [8, 8]],
    ]),
    // linearized nmos: G1 is gm sensing vgs; negative since drain current flows down, against the arrow
    'mosfet amplifier': circuit([
        ['V1', 'V', 0, 5, 0, 12], ['V2', 'V', 3, 9, 0, 1], ['R1', 'R', 8, 2, 0, 2000], ['G1', 'G', 8, 7, 0, -2e-3], ['GND1', 'GND', 0, 11, 0, 0],
    ], [
        [[0, 3], [0, 0]], [[0, 0], [8, 0]], [[8, 4], [8, 5]], [[3, 7], [3, 6]], [[3, 6], [6, 6]],
        [[6, 8], [6, 11]], [[8, 9], [8, 11]], [[0, 7], [0, 11]], [[0, 11], [8, 11]],
    ]),
    // linearized npn: F1 is beta times the base current through R1, R3 is r_pi
    'bjt amplifier': circuit([
        ['V1', 'V', 0, 5, 0, 12], ['V2', 'V', 4, 8, 0, 1], ['R1', 'R', 6, 6, 1, 10e3], ['R3', 'R', 8, 8, 0, 2500],
        ['R2', 'R', 12, 2, 0, 1000], ['F1', 'F', 12, 6, 2, 100, 'R1'], ['GND1', 'GND', 0, 11, 0, 0],
    ], [
        [[0, 3], [0, 0]], [[0, 0], [12, 0]], [[0, 7], [0, 11]], [[0, 11], [12, 11]],
        [[4, 10], [4, 11]], [[8, 10], [8, 11]], [[12, 8], [12, 11]],
    ]),
    // op amp as a high-gain vcvs; gain -R2/R1
    'inverting op amp': circuit([
        ['V1', 'V', 4, 9, 0, 1], ['R1', 'R', 6, 7, 1, 1000], ['R2', 'R', 10, 2, 1, 10e3], ['E1', 'E', 12, 6, 0, 1e5],
        ['GND1', 'GND', 4, 11, 0, 0], ['GND2', 'GND', 9, 5, 0, 0], ['GND3', 'GND', 12, 8, 0, 0],
    ], [
        [[9, 5], [10, 5]], [[8, 7], [10, 7]], [[8, 7], [8, 2]], [[12, 2], [12, 4]],
    ]),
};
const DEMO = CIRCUITS['wheatstone bridge'];

let doc, els, emit, emitHover;
let sel = null, hot = null, hover = null, idle = new Set();
let armed = null, rot = 0, wireTool = false, fromPalette = false;
let ghost = null, drag = null, chain = null, cursor = null, moving = null;
let past = [], future = [];
const view = { x: 0, y: 0, s: 1 };

const snapshot = () => JSON.stringify(doc);
const key = (q) => q.join(',');
const same = (a, b) => a[0] === b[0] && a[1] === b[1];
const turn = ([x, y], r) => [[x, y], [-y, x], [-x, -y], [y, -x]][r & 3];
const pins = (p) => KINDS[p.kind].pins.map((q) => turn(q, p.rot).map((d, i) => d + [p.x, p.y][i]));
const byId = (id) => doc.parts.find((p) => p.id === id);
const letters = (i) => {
    let s = '';
    for (i++; i > 0; i = Math.floor((i - 1) / 26)) s = String.fromCharCode(65 + ((i - 1) % 26)) + s;
    return s;
};

// integer grid, so exact
function onWire(q, { a, b }) {
    const [dx, dy, qx, qy] = [b[0] - a[0], b[1] - a[1], q[0] - a[0], q[1] - a[1]];
    const dot = dx * qx + dy * qy;
    return dx * qy - dy * qx === 0 && dot > 0 && dot < dx * dx + dy * dy;
}

// union-find over pins and wire ends; T-junctions join, crossings don't
function nets() {
    const up = new Map(), degree = new Map(), points = new Map();
    const find = (k) => {
        if (!up.has(k)) up.set(k, k);
        while (up.get(k) !== k) k = up.get(k);
        return k;
    };
    const join = (a, b) => {
        const [ra, rb] = [find(a), find(b)];
        if (ra !== rb) up.set(ra, rb);
    };
    const bump = (k, n = 1) => degree.set(k, (degree.get(k) ?? 0) + n);
    for (const q of doc.parts.flatMap(pins)) points.set(key(q), q), bump(key(q));
    for (const w of doc.wires) {
        points.set(key(w.a), w.a).set(key(w.b), w.b);
        join(key(w.a), key(w.b));
        bump(key(w.a)), bump(key(w.b));
    }
    for (const w of doc.wires) for (const [k, q] of points) if (onWire(q, w)) join(k, key(w.a)), bump(k, 2);

    const name = new Map();
    for (const p of doc.parts) if (p.kind === 'GND') name.set(find(key(pins(p)[0])), 'gnd');
    let n = 0;
    for (const q of doc.parts.flatMap(pins)) {
        const root = find(key(q));
        if (!name.has(root)) name.set(root, letters(n++));
    }
    return { netOf: (q) => name.get(find(key(q))), degree };
}

// anything to connect to at q
const attached = (q) => nets().degree.has(key(q)) || doc.wires.some((w) => onWire(q, w));

export function netlist() {
    const { netOf } = nets();
    return doc.parts.filter((p) => p.kind !== 'GND').map((p) => {
        const [from, to, ...ctrl] = pins(p).map(netOf);
        return { name: p.id, kind: p.kind, from, to, value: p.value, ctrl: SENSES(p.kind) ? [p.by ?? ''] : ctrl };
    });
}

export function highlight(h) {
    hot = h;
    render();
}

// parts the solver skipped: not wired to ground
export function unwired(names) {
    idle = new Set(names);
    render();
}

function save() {
    try { localStorage.setItem(STORE, snapshot()); } catch {}
}

function commit(before) {
    if (before === snapshot()) return render();
    past.push(before);
    future = [];
    save();
    render();
    emit();
}

const change = (fn) => {
    const before = snapshot();
    fn();
    commit(before);
};

function restore(from, to) {
    if (!from.length) return;
    to.push(snapshot());
    doc = JSON.parse(from.pop());
    sel = null;
    save();
    render();
    emit();
}

function nextName(kind) {
    const used = doc.parts.filter((p) => p.kind === kind).map((p) => +p.id.slice(kind.length) || 0);
    return `${kind}${Math.max(0, ...used) + 1}`;
}

function place(at, keep) {
    const p = { id: nextName(armed), kind: armed, x: at[0], y: at[1], rot, value: KINDS[armed].value };
    if (SENSES(armed)) p.by = doc.parts.find((q) => q.kind === 'R')?.id ?? '';
    change(() => doc.parts.push(p));
    sel = { part: p.id };
    if (!keep) arm(null);
    render();
}

// horizontal leg first, zero-length legs dropped
function addWire(a, b) {
    const corner = [b[0], a[1]];
    let id = Math.max(0, ...doc.wires.map((w) => w.id));
    for (const [p, q] of [[a, corner], [corner, b]]) if (!same(p, q)) doc.wires.push({ id: ++id, a: p, b: q });
}

function remove() {
    if (!sel) return;
    change(() => {
        if (sel.part) doc.parts = doc.parts.filter((p) => p.id !== sel.part);
        if (sel.wire) doc.wires = doc.wires.filter((w) => w.id !== sel.wire);
    });
    sel = null;
    render();
}

function rotate() {
    const p = sel?.part && byId(sel.part);
    if (armed) rot = (rot + 1) % 4;
    else if (p) change(() => (p.rot = (p.rot + 1) % 4));
    render();
}

// wire ends sitting on p's pins, so they follow it
function grab(p) {
    const old = pins(p);
    const ends = doc.wires.flatMap((w) => ['a', 'b'].map((e) => [w, e, old.findIndex((q) => same(q, w[e]))]));
    return { p, orig: [p.x, p.y], ends: ends.filter((x) => x[2] >= 0), before: snapshot() };
}

function shift(m, to) {
    [m.p.x, m.p.y] = to;
    const now = pins(m.p);
    for (const [w, e, i] of m.ends) w[e] = [...now[i]];
}

function nudge(dx, dy) {
    const p = sel?.part && byId(sel.part);
    if (!p) return;
    const m = grab(p);
    shift(m, [p.x + dx, p.y + dy]);
    commit(m.before);
}

function startMove(p) {
    arm(null);
    moving = grab(p);
    render();
}

function cancel() {
    if (moving) doc = JSON.parse(moving.before);
    [moving, chain, drag, cursor, wireTool] = [null, null, null, null, false];
    arm(null);
}

function arm(kind) {
    armed = kind;
    if (kind) [wireTool, chain, moving] = [false, null, null];
    else ghost = null;
    render();
}

function toggleWire() {
    const on = !(wireTool || chain);
    arm(null);
    [wireTool, chain] = [on, null];
    render();
}

function fit() {
    const pts = [...doc.parts.flatMap(pins), ...doc.wires.flatMap((w) => [w.a, w.b])];
    const { width: W, height: H } = els.svg.getBoundingClientRect();
    if (!W || !H) return;
    if (!pts.length) return Object.assign(view, { x: -W / 2, y: -H / 2, s: 1 });
    const [xs, ys] = [pts.map((p) => p[0] * G), pts.map((p) => p[1] * G)];
    const [x0, x1, y0, y1] = [Math.min(...xs) - 4 * G, Math.max(...xs) + 5 * G, Math.min(...ys) - 3 * G, Math.max(...ys) + 3 * G];
    view.s = Math.min(W / (x1 - x0), H / (y1 - y0), 1.6);
    view.x = (x0 + x1) / 2 - W / view.s / 2;
    view.y = (y0 + y1) / 2 - H / view.s / 2;
}

function load() {
    try {
        const saved = JSON.parse(localStorage.getItem(STORE));
        if (Array.isArray(saved?.parts) && Array.isArray(saved?.wires) && saved.parts.every((p) => KINDS[p.kind])) return saved;
    } catch {}
    return structuredClone(DEMO);
}

//----------------------------
// DRAWING
//----------------------------

function icon(kind) {
    const k = KINDS[kind];
    return el('svg', { class: `icon k-${kind}`, viewBox: kind === 'GND' ? '-18 -6 36 28' : '-36 -36 72 72' },
        ...(k.shell ? [el('path', { class: 'shell', d: k.shell })] : []), el('path', { d: k.d }));
}

function body(p, cls) {
    const k = KINDS[p.kind];
    const [x, w] = k.pins.length > 2 ? [-34, 46] : [-12, 24];
    const [y, h] = p.kind === 'GND' ? [-2, 20] : [-32, 64];
    return el('g', { class: `${cls} k-${p.kind}`, 'data-part': p.id, transform: `translate(${p.x * G} ${p.y * G}) rotate(${p.rot * 90})` },
        el('rect', { class: 'hit', x, y, width: w, height: h }),
        ...(k.shell ? [el('path', { class: 'shell', d: k.shell })] : []),
        el('path', { d: k.d }));
}

// right of vertical parts, above horizontal ones
function label(p) {
    if (p.kind === 'GND') return [];
    const [x, y] = [p.x * G, p.y * G];
    const [tx, ty, anchor] = p.rot % 2 ? [x, y - 30, 'middle'] : [x + 17, y - 6, 'start'];
    return [el('g', { class: 'label', 'data-part': p.id },
        el('text', { class: 'name', x: tx, y: ty, 'text-anchor': anchor }, p.id),
        el('text', { class: 'value', x: tx, y: ty + 12, 'text-anchor': anchor }, describe(p.kind, p.value, SENSES(p.kind) ? [p.by || '?'] : [])))];
}

// double-click a part or its label: edit the value in place over the label
function editInline(p) {
    const text = els.layer.querySelector(`.label[data-part="${CSS.escape(p.id)}"] .value`);
    if (!text) return;
    const pane = els.svg.parentElement, r = text.getBoundingClientRect(), o = pane.getBoundingClientRect();
    const input = Object.assign(document.createElement('input'), { className: 'inline', value: si(p.value), spellcheck: false, size: 8 });
    Object.assign(input.style, { left: `${r.left - o.left - 4}px`, top: `${r.top - o.top - 3}px` });
    let done = false;
    const close = (save) => {
        if (done) return;
        const v = parse(input.value);
        if (save && v === null) return input.classList.add('bad');
        done = true;
        input.remove();
        if (save && v !== p.value) change(() => (p.value = v));
    };
    input.onkeydown = (e) => {
        e.stopPropagation();
        if (e.key === 'Enter') close(true);
        if (e.key === 'Escape') close(false);
    };
    input.oninput = () => input.classList.toggle('bad', parse(input.value) === null);
    input.onblur = () => (close(true), close(false)); // invalid on blur: drop it
    pane.append(input);
    input.focus();
    input.select();
}

function render() {
    const { width: W, height: H } = els.svg.getBoundingClientRect();
    const [w, h] = [W / view.s, H / view.s];
    els.svg.setAttribute('viewBox', `${view.x} ${view.y} ${w} ${h}`);
    els.svg.classList.toggle('wiring', wireTool || !!chain);
    const { netOf, degree } = nets();
    const lit = (q) => !!hot?.net && netOf(q) === hot.net;
    const sensed = hot?.part && byId(hot.part)?.by;
    const px = (q) => q.map((v) => v * G);

    const wires = doc.wires.map((wr) => {
        const [[x1, y1], [x2, y2]] = [px(wr.a), px(wr.b)];
        const cls = `wire${sel?.wire === wr.id ? ' sel' : ''}${lit(wr.a) ? ' hot' : ''}`;
        return el('g', { class: cls, 'data-wire': wr.id }, el('line', { class: 'hit', x1, y1, x2, y2 }), el('line', { x1, y1, x2, y2 }));
    });
    const parts = doc.parts.flatMap((p) => {
        const on = hot?.part === p.id || (p.kind === 'GND' && hot?.net === 'gnd');
        const cls = ['part', sel?.part === p.id && 'sel', on && 'hot', sensed === p.id && 'ctl', idle.has(p.id) && 'idle'];
        return [body(p, cls.filter(Boolean).join(' ')), ...label(p)];
    });
    const marks = [...new Map(doc.parts.flatMap(pins).map((q) => [key(q), q])).values()].flatMap((q) => {
        const [x, y] = px(q);
        return [
            ...(degree.get(key(q)) === 1 ? [el('rect', { class: 'open', x: x - 3, y: y - 3, width: 6, height: 6 })] : []),
            el('circle', { class: `pin${lit(q) ? ' hot' : ''}`, 'data-pin': key(q), cx: x, cy: y, r: 6 }),
        ];
    });
    const dots = [...degree].filter(([, d]) => d >= 3).map(([k]) => {
        const [x, y] = px(k.split(',').map(Number));
        return el('circle', { class: 'dot', cx: x, cy: y, r: 3 });
    });
    const preview = [];
    if (armed && ghost) preview.push(body({ id: '', kind: armed, x: ghost[0], y: ghost[1], rot }, 'part ghost'));
    const draft = drag?.kind === 'wire' ? [drag.from, drag.to] : chain && cursor ? [chain, cursor] : null;
    if (draft && !same(...draft)) {
        const [a, b] = draft;
        preview.push(el('polyline', { class: 'draft', points: [a, [b[0], a[1]], b].map((q) => px(q).join(',')).join(' ') }));
    }
    if (chain) preview.push(el('circle', { class: 'anchor', cx: chain[0] * G, cy: chain[1] * G, r: 4 }));

    els.layer.replaceChildren(
        el('rect', { class: 'grid', x: view.x, y: view.y, width: w, height: h }),
        ...wires, ...parts, ...dots, ...marks, ...preview);
    for (const b of els.palette.querySelectorAll('button')) {
        b.classList.toggle('on', b.dataset.kind ? b.dataset.kind === armed : b.dataset.act === 'wire' && (wireTool || !!chain));
    }
    inspect();
}

// status line under the schematic: mode, selection editor, hints
let inspected = null;
function inspect() {
    const p = sel?.part && byId(sel.part);
    const mode = moving ? 'move' : armed ? `place ${armed}` : wireTool || chain || drag?.kind === 'wire' ? 'wire' : 'select';
    const others = doc.parts.filter((q) => q !== p && q.kind !== 'GND').map((q) => q.id);
    const id = [mode, p && `${p.id}:${p.value}:${p.by}:${others}`, sel?.wire].join('|');
    if (id === inspected) return;
    inspected = id;
    const make = (tag, props) => Object.assign(document.createElement(tag), props);
    const btn = (textContent, onclick) => make('button', { textContent, onclick });
    const kids = [make('span', { className: 'mode', textContent: mode })];

    if (p && p.kind !== 'GND') {
        const focused = document.activeElement?.id === 'value';
        const input = make('input', { id: 'value', value: si(p.value), size: 7, spellcheck: false });
        const set = () => {
            const v = parse(input.value);
            input.classList.toggle('bad', v === null);
            if (v !== null && v !== p.value) change(() => (p.value = v));
        };
        input.onchange = set;
        input.onkeydown = (e) => { if (e.key === 'Enter') set(); if (e.key === 'Escape') input.blur(); };
        const name = make('span', { className: 'name', textContent: p.id, title: 'double-click to edit value' });
        name.ondblclick = () => (input.focus(), input.select());
        kids.push(name, input,make('span', { className: 'unit', textContent: UNIT[p.kind] }));
        if (SENSES(p.kind)) {
            const pick = make('select', { title: 'sensed part' });
            pick.append(make('option', { value: '', textContent: 'sense…' }),
                ...others.map((o) => make('option', { value: o, textContent: `I(${o})`, selected: o === p.by })));
            pick.onchange = () => change(() => (p.by = pick.value));
            kids.push(pick);
        }
        kids.push(btn('move', () => startMove(p)), btn('rotate', rotate), btn('delete', remove));
        els.inspect.replaceChildren(...kids);
        if (focused) input.focus(), input.select();
        return;
    }
    if (p || sel?.wire) {
        kids.push(make('span', { className: 'name', textContent: p ? p.id : 'wire' }));
        if (p) kids.push(btn('move', () => startMove(p)), btn('rotate', rotate));
        kids.push(btn('delete', remove));
    } else {
        const hint = make('span', { className: 'hint' });
        hint.innerHTML = moving || chain
            ? 'click to set · <kbd>esc</kbd> cancel'
            : `click or drag a part in · click or drag from a pin to wire · <kbd>${MOD} r</kbd> rotate · <kbd>del</kbd> delete · <kbd>${MOD} z</kbd> undo · wheel zoom · drag grid to pan`;
        kids.push(hint);
    }
    els.inspect.replaceChildren(...kids);
}

// k m u n p, M or meg for mega, trailing unit ignored
const MUL = { f: 1e-15, p: 1e-12, n: 1e-9, u: 1e-6, µ: 1e-6, m: 1e-3, k: 1e3, M: 1e6, meg: 1e6, g: 1e9 };
function parse(text) {
    const m = /^\s*([-+]?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?)\s*(meg|[fpnuµmkg])?\s*(?:Ω|ohms?|v\/v|a\/a|[vafs])?\s*$/i.exec(text);
    if (!m) return null;
    const v = parseFloat(m[1]) * (m[2] ? MUL[m[2]] ?? MUL[m[2].toLowerCase()] : 1);
    return Number.isFinite(v) ? v : null;
}

//----------------------------
// INPUT
//----------------------------

const gridAt = (ev) => {
    const p = new DOMPoint(ev.clientX, ev.clientY).matrixTransform(els.svg.getScreenCTM().inverse());
    return [Math.round(p.x / G), Math.round(p.y / G)];
};
const inside = (ev) => {
    const r = els.svg.getBoundingClientRect();
    return ev.clientX >= r.left && ev.clientX <= r.right && ev.clientY >= r.top && ev.clientY <= r.bottom;
};
const target = (ev) => ev.target.closest?.('[data-part],[data-wire],[data-pin]');

function down(ev) {
    if (ev.button !== 0) return cancel();
    const at = gridAt(ev), t = target(ev);
    if (armed) return place(at, ev.shiftKey);
    if (moving) {
        const m = moving;
        moving = null;
        return commit(m.before);
    }
    // click-click wiring: each click adds a leg, stops on a connection
    if (chain) {
        const done = same(chain, at) || attached(at);
        if (!same(chain, at)) change(() => addWire(chain, at));
        chain = done ? null : at;
        return render();
    }
    els.svg.setPointerCapture(ev.pointerId);
    if (wireTool || t?.dataset.pin) {
        drag = { kind: 'wire', from: at, to: at };
    } else if (t?.dataset.part) {
        sel = { part: t.dataset.part };
        drag = { kind: 'move', start: at, ...grab(byId(t.dataset.part)) };
    } else if (t?.dataset.wire) {
        sel = { wire: +t.dataset.wire };
        drag = { kind: 'wire', from: at, to: at, branch: true };
    } else {
        sel = null;
        drag = { kind: 'pan', start: [ev.clientX, ev.clientY], orig: [view.x, view.y] };
    }
    render();
}

function move(ev) {
    const at = gridAt(ev);
    if (drag?.kind === 'pan') {
        view.x = drag.orig[0] - (ev.clientX - drag.start[0]) / view.s;
        view.y = drag.orig[1] - (ev.clientY - drag.start[1]) / view.s;
    } else if (drag?.kind === 'move') {
        shift(drag, [0, 1].map((i) => drag.orig[i] + at[i] - drag.start[i]));
    } else if (drag) {
        drag.to = at;
    } else if (moving) {
        shift(moving, at);
    } else if (armed || chain) {
        [ghost, cursor] = [at, at];
    } else {
        return hoverAt(target(ev));
    }
    render();
}

function hoverAt(t) {
    const { netOf } = nets();
    const p = t?.dataset.part && byId(t.dataset.part);
    const w = t?.dataset.wire && doc.wires.find((w) => w.id === +t.dataset.wire);
    const next = p ? (p.kind === 'GND' ? { net: 'gnd' } : { part: p.id })
        : w ? { net: netOf(w.a) }
        : t?.dataset.pin ? { net: netOf(t.dataset.pin.split(',').map(Number)) } : null;
    if (next?.net !== hover?.net || next?.part !== hover?.part) (hover = next), emitHover(next);
}

function up() {
    const d = drag;
    drag = null;
    if (d?.kind === 'wire' && !same(d.from, d.to)) change(() => addWire(d.from, d.to));
    else if (d?.kind === 'wire' && !d.branch) (chain = d.from), (cursor = d.from), render(); // a click starts a chain
    else if (d?.kind === 'move') commit(d.before);
    else render();
}

function keys(ev) {
    if (ev.target.closest('input, select, textarea')) return;
    const mod = ev.ctrlKey || ev.metaKey;
    const part = Object.keys(KINDS).find((k) => KINDS[k].key === ev.key);
    const arrows = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[ev.key];
    const act = mod
        ? { z: () => (ev.shiftKey ? restore(future, past) : restore(past, future)), y: () => restore(future, past), r: rotate }[ev.key.toLowerCase()]
        : part ? () => arm(part)
        : arrows ? () => nudge(...arrows)
        : { w: toggleWire, Escape: () => ((sel = null), cancel()), Delete: remove, Backspace: remove, Enter: () => document.getElementById('value')?.focus() }[ev.key];
    if (act) ev.preventDefault(), act();
}

export function init({ svg, layer, palette, inspect: box, onChange, onHover }) {
    els = { svg, layer, palette, inspect: box };
    [emit, emitHover] = [onChange, onHover];
    doc = load();

    const make = (tag, props = {}, ...kids) => {
        const n = Object.assign(document.createElement(tag), props);
        n.append(...kids);
        return n;
    };
    const button = (data, title, ...kids) => {
        const b = make('button', { title }, ...kids);
        Object.assign(b.dataset, data);
        return b;
    };
    palette.append(
        ...Object.entries(KINDS).map(([k, d]) => button({ kind: k }, `${d.label}: click or drag onto the grid`, icon(k), make('span', { textContent: d.label }), make('kbd', { textContent: d.key }))),
        button({ act: 'wire' }, 'wire: click or drag between pins', make('span', { textContent: 'wire' }), make('kbd', { textContent: 'w' })),
        make('span', { className: 'sep' }),
        ...['undo', 'redo', 'fit'].map((a) => button({ act: a }, a, make('span', { textContent: a }))),
        make('select', { id: 'circuits', title: 'load a starter circuit' },
            make('option', { value: '', textContent: 'circuits…', disabled: true, selected: true }),
            ...Object.keys(CIRCUITS).map((c) => make('option', { value: c, textContent: c }))),
        button({ act: 'clear' }, 'clear', make('span', { textContent: 'clear' })),
    );
    const pick = palette.querySelector('#circuits');
    pick.onchange = () => {
        change(() => (doc = structuredClone(CIRCUITS[pick.value])));
        pick.value = '';
        fit();
        render();
    };

    svg.onpointerdown = down;
    svg.onpointermove = move;
    svg.onpointerup = up;
    svg.onpointerleave = () => {
        ghost = null;
        if (hover) (hover = null), emitHover(null);
        render();
    };
    svg.ondblclick = (ev) => {
        const p = target(ev)?.dataset.part && byId(target(ev).dataset.part);
        if (p && p.kind !== 'GND') editInline(p);
        else document.getElementById('value')?.focus();
    };
    svg.oncontextmenu = (ev) => ev.preventDefault();
    svg.addEventListener('wheel', (ev) => {
        ev.preventDefault();
        const p = new DOMPoint(ev.clientX, ev.clientY).matrixTransform(svg.getScreenCTM().inverse());
        const s = Math.min(Math.max(view.s * Math.exp(-ev.deltaY * 0.0015), 0.25), 4);
        view.x = p.x - (p.x - view.x) * (view.s / s);
        view.y = p.y - (p.y - view.y) * (view.s / s);
        view.s = s;
        render();
    }, { passive: false });
    window.addEventListener('keydown', keys);

    // drag from palette, or click it then click the grid
    palette.onpointerdown = (ev) => {
        const kind = ev.target.closest('button')?.dataset.kind;
        if (kind) arm(kind), (fromPalette = true);
    };
    window.addEventListener('pointermove', (ev) => {
        if (fromPalette && inside(ev)) (ghost = gridAt(ev)), render();
    });
    window.addEventListener('pointerup', (ev) => {
        if (fromPalette && inside(ev) && armed) place(gridAt(ev));
        fromPalette = false;
    });
    palette.onclick = (ev) => {
        const run = {
            wire: toggleWire,
            undo: () => restore(past, future),
            redo: () => restore(future, past),
            fit: () => (fit(), render()),
            clear: () => change(() => (doc = { parts: [], wires: [] })),
        }[ev.target.closest('button')?.dataset.act];
        run?.();
    };
    new ResizeObserver(render).observe(svg);
    fit();
    render();
}
