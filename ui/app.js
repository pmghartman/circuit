// glue: schematic → solve (tauri invoke or http) → DAG, shared hover; the
// header's update button rebuilds this circuit from source
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

// ---------- update from source ----------
// desktop: scripts/app_build.sh in the checkout this app was built from, its log
// streamed as "update-log" events, then a relaunch. web: POST api/update runs
// scripts/update.sh beside the server; the page polls and reloads when the
// server comes back newer
const upd = { btn: $('update'), box: $('updater'), info: $('updinfo'), log: $('updlog') };
const POLL_MS = 2000;
let running = null, poll = 0;

const when = (t) => (t ? new Date(t).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '');

function tell(text, err = false) {
    upd.box.hidden = false;
    upd.info.textContent = text;
    upd.info.className = err ? 'err' : '';
}

function line(text) {
    upd.log.textContent += `${text}\n`;
    upd.log.scrollTop = upd.log.scrollHeight;
}

function show(s) {
    running = s.version;
    upd.btn.hidden = !s.canUpdate && s.state === 'idle';
    upd.btn.disabled = !s.canUpdate || s.state === 'running';
    upd.btn.textContent = s.state === 'running' ? 'updating…' : 'update';
    if (s.state === 'idle') return;
    upd.log.textContent = s.log ?? '';
    upd.log.scrollTop = upd.log.scrollHeight;
    if (s.state === 'running') tell(`updating since ${when(s.startedAt)}…`);
    else if (s.state === 'ok') tell(s.restartNeeded ? `updated to v${s.to}; restart the server to run it` : s.to && s.to !== s.from ? `updated from v${s.from} at ${when(s.finishedAt)}` : `up to date, checked at ${when(s.finishedAt)}`);
    else tell(s.error ?? `update failed (exit ${s.code})`, true);
}

// while the update runs the server restarts under us: a failed poll is just that
async function pollWeb() {
    clearTimeout(poll);
    let s;
    try {
        s = await call('update');
    } catch {
        if (running) tell('restarting…');
        poll = setTimeout(pollWeb, POLL_MS);
        return;
    }
    if (running && s.version !== running) {
        tell(`now v${s.version}; reloading`);
        setTimeout(() => location.reload(), 1200);
        return;
    }
    show(s);
    if (s.state === 'running') poll = setTimeout(pollWeb, POLL_MS);
}

async function updateWeb() {
    upd.btn.disabled = true;
    try {
        const res = await fetch('api/update', { method: 'POST' });
        const body = await res.json();
        if (!res.ok) throw new Error(body.error);
        show(body);
        poll = setTimeout(pollWeb, POLL_MS);
    } catch (e) {
        tell(`${e.message ?? e}`, true);
        pollWeb();
    }
}

async function updateDesktop() {
    upd.btn.disabled = true;
    upd.btn.textContent = 'updating…';
    upd.log.textContent = '';
    tell('updating: pulling main and building the app with cargo, which takes a few minutes…');
    const stop = await window.__TAURI__.event.listen('update-log', (e) => line(e.payload));
    try {
        const code = await tauri.invoke('update_from_source');
        stop();
        if (code === 0) {
            tell('built and installed; click relaunch to run the new build');
            upd.btn.textContent = 'relaunch';
            upd.btn.disabled = false;
            upd.btn.onclick = () => tauri.invoke('relaunch');
            return;
        }
        tell(`the update failed (exit ${code}); the log says why`, true);
    } catch (e) {
        stop();
        tell(`the update failed: ${e}`, true);
    }
    upd.btn.textContent = 'update';
    upd.btn.disabled = false;
}

$('updclose').onclick = () => (upd.box.hidden = true);
if (tauri) {
    tauri.invoke('source').then(
        (dir) => {
            upd.btn.hidden = false;
            upd.btn.title = `rebuild this app from ${dir}`;
            upd.btn.onclick = updateDesktop;
        },
        () => {},
    );
} else {
    upd.btn.onclick = updateWeb;
    pollWeb();
}
