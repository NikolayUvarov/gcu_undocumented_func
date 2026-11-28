// MIND Core's wasm kernel in Node (215-PRT-0017): the console on stdout, keys from stdin or from a script.
//   node web/node/run.mjs KERNEL.wasm AARCH64_ROOT [--volume FAT.img] [--seconds N] [--until TEXT] [--screenshot OUT.png]
//                         [--after TEXT --type TEXT]...
// Each --after/--type pair types TEXT (\n as Enter) once the console has shown AFTER since the step before. With --until
// the run ends at TEXT, 0; at --seconds without it, 1; when the kernel stops, 2 unless it halted (0). --screenshot
// writes the screen as it is at the end.
import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { start, BOOT_FILES } from '../mind.js';
import { png } from './png.mjs';
import { writeFileSync } from 'node:fs';

const args = process.argv.slice(2);
const [kernelPath, root] = args;
if (!kernelPath || !root) { console.error('usage: node run.mjs KERNEL.wasm AARCH64_ROOT [--volume FAT.img] [--seconds N] [--until TEXT] [--after TEXT --type TEXT]...'); process.exit(2); }
let volumePath = null, seconds = 600, until = null, screenshot = null;
const steps = [];
for (let i = 2; i < args.length; i += 2) {
    const [flag, value] = [args[i], args[i + 1]];
    if (flag === '--volume') volumePath = value;
    else if (flag === '--seconds') seconds = Number(value);
    else if (flag === '--until') until = value;
    else if (flag === '--screenshot') screenshot = value;
    else if (flag === '--after') steps.push({ after: value, type: null });
    else if (flag === '--type' && steps.length && steps.at(-1).type === null) steps.at(-1).type = value.replaceAll('\\n', '\r');
    else { console.error(`unknown or misplaced ${flag}`); process.exit(2); }
}

const files = new Map();
for (const name of BOOT_FILES) { const path = join(root, name); if (existsSync(path)) files.set(name, readFileSync(path)); }
const begun = performance.now();
let seen = '', since = 0, step = 0, done = null;
const decoder = new TextDecoder();
let mind;
try { mind = await start({
    kernel: readFileSync(kernelPath),
    files,
    volume: volumePath ? readFileSync(volumePath) : null,
    write: (byte) => {
        process.stdout.write(Uint8Array.of(byte));
        seen += decoder.decode(Uint8Array.of(byte), { stream: true });
        if (seen.length > 1 << 20) { since -= seen.length - (1 << 19); seen = seen.slice(-(1 << 19)); since = Math.max(since, 0); }
    },
    stopped: (why) => { done ??= why === 0 ? 0 : 2; console.error(`\n[mind] the kernel stopped (${['halted', 'a restart asked', 'frozen'][why] ?? why})`); },
}); } catch (error) { console.error(`\n[mind] the kernel trapped while booting: ${error.message}`); process.exit(2); }
console.error(`[mind] started in ${(performance.now() - begun).toFixed(0)} ms: ${files.size} of ${BOOT_FILES.length} programs, volume ${volumePath ? 'given' : 'none'}`);
if (process.stdin.isTTY) { process.stdin.setRawMode(true); }
process.stdin.on('data', (bytes) => { if (bytes[0] === 3) process.exit(130); mind.type(bytes); });

const elapsed = () => ((performance.now() - begun) / 1000).toFixed(2);
function turn() {
    while (done === null) {
        if (until !== null && seen.includes(until)) { console.error(`\n[mind] "${until}" after ${elapsed()} s`); done = 0; break; }
        if (step < steps.length && seen.indexOf(steps[step].after, since) >= 0) {
            since = seen.indexOf(steps[step].after, since) + steps[step].after.length;
            mind.type(new TextEncoder().encode(steps[step].type ?? ''));
            step++;
        }
        if (performance.now() - begun > seconds * 1000) { console.error(`\n[mind] ${seconds} s passed${until === null ? '' : ` without "${until}"`}`); done = until === null ? 0 : 1; break; }
        let wait;
        try { wait = mind.turn(50); } catch (error) { console.error(`\n[mind] the kernel trapped after ${elapsed()} s: ${error.message}`); done = 2; break; }
        if (wait < 0) break;
        if (wait > 0) { setTimeout(turn, Math.min(wait, 10)); return; }
        setImmediate(turn); return;
    }
    if (screenshot) { writeFileSync(screenshot, png(mind.screen(), mind.width, mind.height)); console.error(`[mind] the screen is in ${screenshot}`); }
    process.exit(done ?? 2);
}
turn();
