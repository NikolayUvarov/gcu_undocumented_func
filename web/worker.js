// The kernel's thread in a tab (215-PRT-0017): fetches the kernel and the boot volume, runs the turns, draws the screen
// into the page's canvas and sends the console's text back. The page's thread only forwards keys and the pointer.
import { start, BOOT_FILES } from './mind.js';
import { rootFiles } from './fat.js';

const EV_SYN = 0, EV_KEY = 1, EV_ABS = 3, BTN_LEFT = 0x110, BTN_GEAR_DOWN = 0x150, BTN_GEAR_UP = 0x151;
const TABLET = 0, KEYBOARD = 1;
let mind = null, buttons = 0;
const say = (kind, text) => postMessage({ kind, text });

async function fetched(url, what) {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`${what}: ${response.status} ${response.statusText} at ${url}`);
    const total = Number(response.headers.get('content-length')) || 0, reader = response.body.getReader(), parts = [];
    let got = 0;
    for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        parts.push(value); got += value.length;
        say('status', `Loading ${what}: ${(got / 1048576).toFixed(1)}${total ? ` of ${(total / 1048576).toFixed(1)}` : ''} MiB`);
    }
    const out = new Uint8Array(got);
    let at = 0;
    for (const part of parts) { out.set(part, at); at += part.length; }
    return out;
}

async function boot(canvas) {
    const kernel = await fetched('build/kernel.wasm', 'the kernel');
    const volume = await fetched('build/volume.img', 'the boot volume');
    const all = rootFiles(volume);
    const files = new Map(BOOT_FILES.filter((name) => all.has(name)).map((name) => [name, all.get(name)]));
    let text = '';
    const decoder = new TextDecoder();
    say('status', 'Starting the kernel');
    const begun = performance.now();
    mind = await start({ kernel, files, volume, write: (byte) => { text += decoder.decode(Uint8Array.of(byte), { stream: true }); },
        stopped: (why) => say('status', ['The system stopped.', 'The system asked to restart: reload the page.', 'The kernel stopped on a fault: see the console.'][why] ?? 'Stopped.') });
    const context = canvas.getContext('2d');
    const image = context.createImageData(mind.width, mind.height);
    const pixels = new Uint32Array(image.data.buffer);
    let drawn = 0, prompt = false;
    const draw = () => {
        const screen = new Uint32Array(mind.screen().buffer, mind.screen().byteOffset, mind.width * mind.height);
        for (let i = 0; i < screen.length; i++) { const p = screen[i]; pixels[i] = 0xFF000000 | (p & 0xFF) << 16 | (p & 0xFF00) | (p >> 16 & 0xFF); }
        context.putImageData(image, 0, 0);
    };
    const tick = () => {
        let wait = 0;
        try { wait = mind.turn(12); } catch (error) { say('status', `The kernel trapped: ${error.message}`); draw(); return; }
        if (text) {
            // The desktop at the shell's first prompt, as a person would type it.
            if (!prompt && text.includes('MIND> ')) { prompt = true; mind.type(new TextEncoder().encode('wm\r')); say('status', `Running: the shell answered ${((performance.now() - begun) / 1000).toFixed(1)} s after the start; the desktop starts`); }
            say('console', text); text = '';
        }
        const now = performance.now();
        if (now - drawn > 40) { draw(); drawn = now; }
        if (wait >= 0) setTimeout(tick, wait === 0 ? 0 : Math.min(wait, 10));
    };
    tick();
}

onmessage = ({ data }) => {
    if (data.kind === 'start') { boot(data.canvas).catch((error) => say('status', `Not started: ${error.message}`)); return; }
    if (!mind) return;
    if (data.kind === 'key') { mind.input(KEYBOARD, EV_KEY, data.code, data.down ? 1 : 0); mind.input(KEYBOARD, EV_SYN, 0, 0); }
    if (data.kind === 'type') mind.type(new TextEncoder().encode(data.text));
    if (data.kind === 'pointer') {
        mind.input(TABLET, EV_ABS, 0, Math.round(data.x * 32767)); mind.input(TABLET, EV_ABS, 1, Math.round(data.y * 32767));
        for (let b = 0; b < 3; b++) { const bit = [1, 2, 4][b]; if ((data.buttons ^ buttons) & bit) mind.input(TABLET, EV_KEY, BTN_LEFT + [0, 1, 2][b], data.buttons & bit ? 1 : 0); }
        buttons = data.buttons;
        mind.input(TABLET, EV_SYN, 0, 0);
    }
    if (data.kind === 'wheel') {
        const code = data.down ? BTN_GEAR_DOWN : BTN_GEAR_UP;
        mind.input(TABLET, EV_KEY, code, 1); mind.input(TABLET, EV_SYN, 0, 0); mind.input(TABLET, EV_KEY, code, 0); mind.input(TABLET, EV_SYN, 0, 0);
    }
};
