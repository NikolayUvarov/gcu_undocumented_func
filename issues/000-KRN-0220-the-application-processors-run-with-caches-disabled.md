# 000-KRN-0220 — The application processors run with their caches disabled (CR0.CD and CR0.NW left set)

**Type:** kernel (`arch/x86_64/cpu.rs`'s AP start, `mmu.rs`) · **Owner:** kernel session · **Priority:** P1 · **Status:** open (made and tested in QEMU, below; the run on a machine left) · **Blocked by:** — · **Roadmap:** track A, main task [174](174-full-use-of-pc-hardware.md) · **Constitution:** MC-12.1, MC-12.2

## Problem

**Seen in round 91's record** (000-KRN-0219). In QEMU:
- CPU 0 had `CR0=0x80010033`;
- CPUs 1–3 had `CR0=0xE0010013`, that is CD (bit 30) and NW (bit 29) set: caching disabled.

**Why.**
- **After INIT,** an AP's `CR0` is `0x60000010`, with CD and NW set.
- **The trampoline** (`cpu.rs`, `ap_boot`) keeps them: `and eax, 0xfffffff3` clears EM and TS alone, and `or eax, 0x80010003` sets PG, WP, MP and PE.
- **`mmu::enable_protection`** keeps them too (`(cr0 | WP | MP) & !12`).
- **Only the BSP** clears them, at the end of `activate_boot_map`.

**What it means.**
- **Under QEMU's TCG,** nothing: TCG does not model caches. So no suite can see it.
- **On a real machine** every AP runs uncached, from memory and with every write going through, unless the processor ignores CD with NW set (Intel SDM, vol. 3A, 12.5.1: CD=1 NW=1 is "not a valid setting" for most processors, and behaves as no-fill).
  - Every kernel path and every task on CPUs 1 and up would be many times slower than on CPU 0.
  - That is a candidate, not a cause (MC-12.2), for [000-KRN-0084](000-KRN-0084-an-ipc-round-trip-between-two-cpus.md)'s 60 µs round trip between two CPUs on the MacBook Pro against 7 µs at best: `kbench --child` runs on another CPU.

## Plan

1. **Each AP enables its caches once its PAT is written** (`enable_protection(bsp = false)`):
   - the PAT written while CD is still set, as Intel SDM 12.12.4 asks;
   - `wbinvd`, a `CR3` reload, then CD and NW cleared.

   The MTRRs stay the firmware's on every CPU, as the BSP keeps them (`activate_boot_map` writes back `MTRR_DEF_TYPE` unchanged).
2. **The trampoline** keeps CD set until then.
3. **A line per AP** is not needed. Instead the hardware report gives each CPU's `CR0` as the kernel read it after start, so a machine's report shows it.
4. **Measured on a real machine** before and after, with the build named (MC-12.1):
   - `kbench`'s call-and-reply row between two CPUs (000-KRN-0084);
   - a row of the same work on CPU 0 and on CPU 1.

   The MacBook Pro's or the DEXP's `kbench --quick` would do.

## Acceptance criteria

- Every CPU's `CR0` in the hardware report has CD and NW clear, in QEMU (x86, 4 and 16 CPUs) and on one real machine.
- The gate passes.
- `kbench` on that machine before and after, recorded in this issue and in 000-KRN-0084.

## Progress (2026-11-09)

- **`mmu::enable_protection(false)`:** an AP writes its PAT while CD and NW are still set, as before; then `wbinvd`, a `CR3` reload, and CD and NW cleared. The MTRRs stay the firmware's. The trampoline keeps CD set until then, as plan step 2 asks.
- **The hardware report** gives each CPU's `CR0` as the kernel read it once that CPU started (`cpu::CR0`): the line `CR0 after start` under `CPU`, the BSP's read after `activate_boot_map`.
- **Tests:**
  - **The `normal` suite on x86** reads every CPU's `CR0` from QEMU's monitor (`info registers -a`) and asks CD and NW clear.
    - **Without the change** it fails: `['80010033', 'E0010013', 'E0010013', 'E0010013']`, the APs' CD and NW set, as round 91's record showed.
    - **With it,** it passed on 4 and on 16 CPUs.
  - **`tests/usb_image_smoke.py`** reads the line from the report on the image, every CPU's bits clear; it passed.
  - **The groups on one CPU and the scheduler's,** on 9baf987b (with 100000-KRN-0171 and the kernel's budget at 7400): `x86: services, storage, audio` (785 s), `x86: scheduling, isolation, heap` (146 s), `x86: one CPU` (142 s) and `x86: AVX state, one CPU` (48 s), through `scripts/ci_local.sh`; all passed.
  - All in the kernel session's container (QEMU 8.2.2, OVMF, TCG). TCG models no cache, so these show the bits, not a speed (MC-12.1).
- **Left:** the run on a machine, `kbench` before and after (plan step 4). It waits for a build with this on the MacBook Pro or the DEXP.

## Related

[000-KRN-0219](000-KRN-0219-a-cpu-starved-of-the-schedulers-lock.md) (the record), [000-KRN-0084](000-KRN-0084-an-ipc-round-trip-between-two-cpus.md), [174](174-full-use-of-pc-hardware.md).
