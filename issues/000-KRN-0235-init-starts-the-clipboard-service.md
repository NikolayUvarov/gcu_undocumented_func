# 000-KRN-0235 — `init` starts the clipboard service and gives the shell its client to mint copies from

**Type:** kernel track (`init`, the boot images) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** — ([000-APP-0032](000-APP-0032-system-clipboard.md)) · **Roadmap:** track G · **Constitution:** MC-2.3, MC-3.4, MC-12.4

## Problem and plan

APP's request of 2026-11-12 (from `requests-KRN.md`), for [000-APP-0032](000-APP-0032-system-clipboard.md). The request as it was written, kept whole:

**Asked by** APP on 2026-11-12, for [000-APP-0032](000-APP-0032-system-clipboard.md). [211-KRN-0058](../issues-done/211-KRN-0058-slots-for-the-shell-and-the-clipboard.done) gave `SLOT_CLIPBOARD` (31) and `REQUEST_CLIPBOARD` (bit 19), and left `init` starting the service until the tools track wrote it.

- **The service:** `clipboard` (`clipboard/`, `idl/clipboard.wit` 1.0), written by APP on its branch; this request's own notice will name the commit.
  - It holds one text, in memory only, up to 60 KiB, and answers `set` and `get` only for the program a manager names as in front. It needs no device, no disk and no network.
- **Asked of `init`:**
  - **A boot image,** `clipboard.elf`, started at boot before the shell, with its server endpoint in `SLOT_SERVICE` and nothing else, quota one endpoint; it is not essential: a boot without it runs as before, and every program keeps its own copy buffer.
  - **The shell's client:** in `SLOT_CLIPBOARD`, a client of its endpoint with `CAP_BADGE`, as `SLOT_NN` is given for `nnd` (262-KRN-0178), so that the shell mints the badged copies itself: a manager's (`mind::clipboard::BADGE_MANAGER`) for itself and for a window manager it starts, a program's (`BADGE_PROGRAM`) for a program that asks `REQUEST_CLIPBOARD`.
  - No other task gets a client of it from `init`.
- **No ABI change:** the slot and the bit are in the ABI since 211-KRN-0058.
- **Checked by** the `tools` suite, once it is in: text copied in `edit` pasted into the shell's line and into a second `edit`; a program that did not ask holds nothing in the slot; a program behind another is refused `get`.

## Acceptance criteria

- What the request's "Checked by" names, run by the asking track once this is in; `init`'s or the loader's own lines in the log.
- The tests the change touches pass on x86 and aarch64 in QEMU.

## Progress

**Made (2026-11-12)** on `claude/000-KRN-0235-clipboard`, on 112-NET-0072's branch (handoff 34), after APP's service reached `main` (round 124, 60b995bd):
- **Handoff 35:** `clipboard` a boot image after `sshd`, last before the shell (`common/abi.rs`: 52 images; `tests/boot_abi_host.rs`: BootInfo 1192 bytes); `clipboard.elf` in `02_build.sh`'s crates, as APP asked (`KRN/2026-11-12T1931Z`).
- **`init`:** its endpoint and nothing else, one endpoint of quota, configurable (`services::CONFIGURABLE`); the shell's client in `SLOT_CLIPBOARD` with `CAP_BADGE` when it runs, kept by `init` and revoked with every copy the shell minted before a new shell starts, as `SLOT_NN`.
- **Docs:** `docs/profile/bootstrap.md` (its row, and the shell's).
- `tests/qemu_smoke.py`'s service list names it, so every suite's boot checks it runs.
- **Tested** (this container): `tests/boot_abi_host.rs`, `tests/services_host.rs`; `init` and `clipboard` build for both targets.
- **On the maintainer's machines** (Effector, QEMU under TCG) at f3405836: "x86: boot, display, shell, memory, clock" (713 s) and "x86: keys, shell, tools" (455 s) on x32, "aarch64: boot and fault containment" (164 s) on acc-1 passed; each boot's service list holds `clipboard`.
- **Left:** APP's `tools` checks of its own (text copied in `edit`, pasted into the shell and another `edit`); the gate.

**Returned by round 125's gate (2026-11-13)** in two groups, both where `clipboard` now runs:
- **"x86: the window manager":** `wm` logs `STARTED fm PID … WITH window,files,sysinfo,shell WITHOUT clipboard`, and the suite's check split that whole line. It now reads the lent list before `WITHOUT` (a4217c78).
- **"x86: one CPU":** `clipboard` logs each program the shell names in front, `dmesg` itself among them, and that record may follow the forged line `dmesg -n 1` looked for. The check now looks in the last three records (a4217c78).
- **A gap this shows, not closed here:** `wm` lends its programs no clipboard copy. The shell lends it one that may badge, through the loader's `grant`. But the IDL runtime passes no capability with the right to badge (`idl::wire::PASSED`, [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done)), so `wm`'s own mints are refused. Read from the code and `wm`'s log; no trace was taken. Its programs keep their own copy buffers, the fallback 000-APP-0032 names. The shell's console and the programs the shell starts are not affected. Recorded for APP in [requests-APP.md](requests-APP.md), with two ways out.

**The first way taken (2026-11-13):**
- **APP's part** (`KRN/2026-11-13T0216Z-APP-the-first-way-clipboard-wit-1-1-is-ready.md`, LANDING 0246Z): `clipboard.wit` 1.1 `program-client`, answered on a `BADGE_MANAGER` copy alone, moves to the caller a `BADGE_PROGRAM` client the service mints from `SLOT_SERVICE`. The shell lends a window manager a `BADGE_MANAGER` copy, and `wm` asks the service for each program's.
- **`init`'s part:** the service's server endpoint with `CAP_BADGE` in `SLOT_SERVICE`, as `blockstore`'s; `docs/profile/bootstrap.md`'s row says so.
- Either lands alone without harm: without the right, `wm`'s programs keep their own copy buffers, as before.

## Related

- [requests-KRN.md](requests-KRN.md), where it was asked.
