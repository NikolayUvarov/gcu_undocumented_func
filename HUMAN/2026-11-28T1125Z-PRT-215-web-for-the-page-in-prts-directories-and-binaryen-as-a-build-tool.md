# PRT → HUMAN: 215: `web/` for the page, as one of PRT's directories? And Binaryen as a build tool

**From:** PRT (the kernel session) · **To:** HUMAN · **About:** main task 215, step 1 (215-PRT-0017 on branch `claude/215-PRT-0017-arch-wasm`)

Two things for TRACKS.md and THIRD_PARTY.md, both yours to decide.

## 1. `web/`: the page's code, as PRT's directory

The page that runs MIND Core in a tab (and the same code in Node) needs a directory. I propose `web/` (JavaScript, MIT/Apache-2.0, no framework, as `docs/wasm/README.md` section 4.3 planned).
- Today it holds `mind.js` (the memory layout and the turns) and `node/run.mjs` (the Node runner). The tab's page and the virtio device models follow.
- **May I add `web/` to PRT's directories in TRACKS.md?** PRT already holds `kernel/src/arch/`, where `arch/wasm` now lives. A new track for the page alone seems more than it needs. If you would rather APP own the page's look later, `web/` can be split then.

Until you say, `web/` lives only on my branch.

## 2. Binaryen, to lower the kernel to wasm32

- The kernel is built for `wasm64-unknown-unknown`, so that `usize` and pointers are 64 bits, as the aarch64 programs read `BootInfo`.
- Node 22 and Safari do not take a 64-bit table yet, so Binaryen's `wasm-opt --memory64-lowering --table64-lowering` lowers the module to wasm32.
  - Binaryen is Apache-2.0, from the WebAssembly project.
  - I use the npm package `binaryen@132.0.0`, which runs wasm-opt in Node.
- It is a build tool only: nothing of it goes into an image or the module.
- I will record it in THIRD_PARTY.md with the pinned version, as the other build tools are, unless you say otherwise.

## Where it stands

- The kernel builds for wasm64 with no warning, and lowers to a 307 KB wasm32 module.
- The aarch64 programs are being built in this session to boot it in Node.
- I will tell you here and in ALL when init and the shell answer there.
