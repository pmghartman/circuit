// glue: schematic → solve (tauri invoke or http) → DAG, shared hover
import * as editor from './editor.js';
import * as graph from './graph.js';

const $ = (id) => document.getElementById(id);
const tauri = window.__TAURI__?.core;
let timer = 0, seq = 0, shown = false;

async function call(cmd, args) {
    if (tauri) return tauri.invoke(cmd, args);
    const init = args && { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(args) };
    const res = await fetch(`api/${cmd}`, init);
    const body = await res.json();
    if (!res.ok) throw new Error(body.error);
    return body;
}

function say(kind, text) {
    $('state').textContent = kind;
    $('state').className = `pill ${kind}`;
    $('status').textContent = text;
}

async function solve(edit) {
    const id = ++seq, host = $('host');
    try {
        const sim = await call('solve', { parts: editor.netlist(), width: host.clientWidth, height: host.clientHeight });
        if (id !== seq) return;
        graph.present(sim, { animate: edit && shown, restart: edit || !shown });
        graph.stale(false);
        editor.unwired(sim.skipped);
        shown = true;
        const f = sim.frames;
        const span = f.length > 1 ? ` · transient ${graph.si(f.at(-1).time, 's')}` : ' · dc';
        const loose = sim.skipped.length ? ` · unwired ${sim.skipped.join(' ')}` : '';
        say(loose ? 'warn' : 'ok', `${f[0].nodes.length - 1} nets · ${f[0].edges.length} parts${span} · ${sim.elapsedMs.toFixed(2)} ms${loose}`);
    } catch (e) {
        if (id !== seq) return;
        graph.stale(true);
        editor.unwired([]);
        say('err', e.message ?? `${e}`);
    }
}

const later = (edit, ms = 120) => {
    clearTimeout(timer);
    timer = setTimeout(() => solve(edit), ms);
};

const hover = (h) => {
    editor.highlight(h);
    graph.highlight(h);
};

graph.init($('graph'), { bar: $('player'), play: $('play'), scrub: $('scrub'), clock: $('clock') }, hover);
editor.init({
    svg: $('schematic'),
    layer: $('sheet'),
    palette: $('palette'),
    inspect: $('inspect'),
    onChange: () => later(true),
    onHover: hover,
});
new ResizeObserver(() => later(false, 150)).observe($('host'));
call('version').then((v) => ($('version').textContent = `v${v.version ?? v}`), () => {});
