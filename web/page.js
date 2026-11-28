// The page's thread (215-PRT-0017): hands the canvas to the kernel's worker and forwards keys and the pointer to it.
import { KEYS } from './keys.js';

const canvas = document.getElementById('screen'), status = document.getElementById('status'), shown = document.getElementById('console');
const worker = new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });
const offscreen = canvas.transferControlToOffscreen();
worker.postMessage({ kind: 'start', canvas: offscreen }, [offscreen]);
worker.onmessage = ({ data }) => {
    if (data.kind === 'status') status.textContent = data.text;
    if (data.kind === 'console') {
        const atEnd = shown.scrollTop + shown.clientHeight >= shown.scrollHeight - 4;
        shown.textContent = (shown.textContent + data.text.replaceAll('\r', '')).slice(-200000);
        if (atEnd) shown.scrollTop = shown.scrollHeight;
    }
};

const key = (event, down) => {
    const code = KEYS.get(event.code);
    if (code === undefined) return;
    event.preventDefault();
    if (down && event.repeat) return; // the system repeats keys itself
    worker.postMessage({ kind: 'key', code, down });
};
canvas.addEventListener('keydown', (event) => key(event, true));
canvas.addEventListener('keyup', (event) => key(event, false));
canvas.addEventListener('blur', () => { for (const code of [29, 42, 54, 56, 97, 100, 125]) worker.postMessage({ kind: 'key', code, down: false }); });

const pointer = (event) => {
    const box = canvas.getBoundingClientRect();
    const x = Math.min(Math.max((event.clientX - box.left) / box.width, 0), 1), y = Math.min(Math.max((event.clientY - box.top) / box.height, 0), 1);
    worker.postMessage({ kind: 'pointer', x, y, buttons: event.buttons & 7 });
};
canvas.addEventListener('pointermove', pointer);
canvas.addEventListener('pointerdown', (event) => { canvas.focus(); canvas.setPointerCapture(event.pointerId); pointer(event); });
canvas.addEventListener('pointerup', pointer);
canvas.addEventListener('contextmenu', (event) => event.preventDefault());
canvas.addEventListener('wheel', (event) => { event.preventDefault(); worker.postMessage({ kind: 'wheel', down: event.deltaY > 0 }); }, { passive: false });

document.getElementById('line').addEventListener('submit', (event) => {
    event.preventDefault();
    const typed = document.getElementById('typed');
    worker.postMessage({ kind: 'type', text: `${typed.value}\r` });
    typed.value = '';
});
canvas.focus();
