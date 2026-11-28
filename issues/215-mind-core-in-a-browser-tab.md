# 215 — MIND Core in a browser tab: its desktop and applications, for people to try

**Type:** main task, porting (a new architecture, `wasm`) · **Owner:** `PRT` (the kernel session), with `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.3, MC-12.9

## Problem

**The maintainer's word (2026-11-28):**
- «запустить mind-core-os в webassembly, аналогично проекту unkernel.dev … Это очень важно для нас, иметь поддержку такого вида — чтобы повысить популярность и привлечь сторонников»;
- on the plan: «yes go B-way. We need it to show interface and applications readyness for humans so they can support as contibutors».

**So the aim is people seeing MIND Core's desktop and applications at work, from a link, with nothing to install.** A machine or QEMU asks too much of someone who only wants to look. [unkernel](https://github.com/pithings/unkernel) shows that a kernel in a browser tab can be quick enough.

## Plan

The way is B of [docs/wasm/README.md](../docs/wasm/README.md):
- the kernel built for wasm as `kernel/src/arch/wasm`;
- the aarch64 programs as they are, on an emulator of user-mode A64;
- the tab's devices given by the page.

**The desktop comes first:** the screen, the keyboard and the pointer before the network, sound and speed.

| Step | Task | What |
|---|---|---|
| 0 | [215-PRT-0016](../issues-done/215-PRT-0016-an-a64-user-mode-emulator.done) (done, round 184) | The emulator: user-mode A64, integer, with a page-table walk; one MIND program run to its system calls, in Node |
| 1 | numbered when it starts | `arch/wasm` and the page: init, the services, `wm` and its applications on a canvas; the boot volume in memory; keys and the pointer from the page |
| 2 | numbered when it starts | Published: the page on a static host, a link in the README, a CI group that boots it in Node |
| 3 | later | Writes kept in the browser, the network through a proxy, sound, a JIT, more CPUs |

**The measured basis** (2026-11-28, `docs/wasm/README.md`, section 3):
- the kernel keeps no stack per task, so it needs no JSPI;
- the aarch64 build of 2026-11-10 holds no FP or SIMD instruction in any program but one test fixture, and no LSE atomic;
- its programs read only three system registers, NZCV, RNDR and CNTVCT.

## Acceptance criteria

1. Someone with a current browser opens the published link and, with nothing installed, sees `wm`'s desktop. The time from the click is measured and stated.
2. In that tab they open the shell, `fm`, `edit` and `view`, with the keyboard and the pointer, and the applications work as on QEMU. Anything that does not is listed in the profile.
3. `docs/profile/wasm.md` states the configuration: browsers, the TCB (the browser, the page, the emulator), what is missing (TPM, secure boot, the network until step 3) and what is evidence there (MC-12.1, 12.9).
4. A CI group boots the tab's build in Node to the desktop, so it does not break between releases.

## Related

[docs/wasm/README.md](../docs/wasm/README.md) (the plan), [205](205-aarch64-boards.md) (the aarch64 build it reuses), [177](177-display-output.md) (the display and the desktop).
