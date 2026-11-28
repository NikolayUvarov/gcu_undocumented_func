// Runs one MIND Core aarch64 program on the A64 emulator compiled to wasm (215-PRT-0016):
//   node a64/node/run.mjs a64_node.wasm program.elf
// The program's LOG lines go to stdout; what the stand-in kernel refused, and the count of instructions, to stderr.
import { readFileSync } from 'node:fs';

const [wasmPath, elfPath] = process.argv.slice(2);
if (!wasmPath || !elfPath) {
    console.error('usage: node run.mjs a64_node.wasm program.elf');
    process.exit(2);
}
let memory;
const text = new TextDecoder();
const results = [];
const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {
    env: {
        host_log: (at, len) => process.stdout.write(text.decode(new Uint8Array(memory.buffer, at, len))),
        host_now_ms: () => performance.now(),
        host_refused: (number) => console.error(`[a64] system call ${number} refused: the stand-in answers LOG, UPTIME, EXIT, ALLOC and FREE`),
        host_results: (at, count) => results.push(...new BigUint64Array(memory.buffer, at, count)),
    },
});
memory = instance.exports.memory;
const elf = readFileSync(elfPath);
new Uint8Array(memory.buffer, instance.exports.elf_buffer(), elf.length).set(elf);
const loaded = instance.exports.load(elf.length);
if (loaded !== 0) {
    console.error(`[a64] ${elfPath} not loaded (${loaded})`);
    process.exit(1);
}
const start = performance.now();
const code = instance.exports.go();
const ms = performance.now() - start;
const count = instance.exports.executed();
for (const [i, v] of results.entries()) console.log(`slot ${i}: 0x${v.toString(16).padStart(16, '0')}`);
console.error(`[a64] ${code === 0 ? 'ended' : `stopped (${code}) at pc 0x${instance.exports.pc().toString(16)}`}: ` +
    `${count} instructions in ${ms.toFixed(1)} ms, ${(count / ms / 1000).toFixed(1)} million a second`);
process.exit(code === 0 ? 0 : 1);
