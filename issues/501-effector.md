# 501 — The effector: tests on real hardware, driven by a test server

**Type:** main task · **Owner:** `ASR` track (open) · **Priority:** P2 · **Status:** open (proposed) · **Blocked by:** [550](550-network-on-real-hardware.md) (the network on the MacBook Pro); from `NET` ([requests-NET.md](requests-NET.md), recorded for 351): TLS for a service other than the shell, and a persistent device key · **Roadmap:** Assurance "evidence tied to configuration" · **Constitution:** MC-12.1, MC-12.2, MC-12.9, MC-2.3, MC-10.2, MC-11.9

Asked by the maintainer (2026-10-08): tests should run on real hardware without the maintainer typing them. Opened by the kernel session; the `ASR` track is open.

## Problem

Every check on a real machine today is typed by the maintainer and read from photographs of the screen ([211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md)). The QEMU suites (`tests/qemu_smoke.py`) drive a guest through its serial line and QMP, which a laptop does not have. Evidence for a physical configuration is therefore rare, slow and made by hand. MC-12.1 asks for evidence per configuration, and QEMU's results do not carry over to a machine (MC-12.9).

## Design (proposed)

- **The effector** is a service on the target. It connects out over TLS (`tls`) to the one server named in its configuration, and checks the server's pinned key. The target needs no listening port.
- **It authenticates with the device key** (`keystore`), so the key must persist across boots (the `NET` request). The server keeps a list of the devices it accepts.
- **It receives a test bundle:** programs, `msh` scripts and their expected outputs, under a bundle ID.
- **It runs them in a test account:** a session with only the grants the bundle declares (as an `msh` script's `requires:` does), within a memory and time budget. A test gets none of the effector's own capabilities (TLS, the device key) and cannot widen its grants.
- **It streams back** the system log (from `logd`, under a read grant: MC-10.2), screenshots (as the shell's `screenshot` takes them, issue 086), and each test's output, exit status and time.
- **A new image only through self-update** ([351](351-self-update.md)). The server publishes a signed test release; the effector asks `updater` to stage and apply it, and the trial boot with last-known-good protects the machine. The effector never writes disks or boot files itself. Which key may sign a test release is a key-role question for 351-UPD-0009.
- **The server** is a small host program (in `scripts/` or `tools/`, proposed). It reuses `tests/qemu_smoke.py`'s checks where it can, so the same suite runs in QEMU and on hardware.
- **Evidence.** Each run is recorded with its configuration: the machine, the firmware and its settings, the image's manifest hash (350), the bundle and any seed. The profile cites a run for that configuration only. A passing run is evidence of what was run, not a proof (MC-12.2).
- **Its own risk.** The effector lets the server run code on the device, within the test account. It is off by default, turned on by the machine's owner, and [docs/profile/threat-model.md](../docs/profile/threat-model.md) names it when it lands.

## Plan: tasks

| Task | Track | What |
|---|---|---|
| 501-ASR-0007 | `ASR` (open) | The protocol: messages, the bundle format, result records and their versions, as an IDL file or a documented wire format (MC-2.3); and the host server, tested against a guest in QEMU |
| 501-ASR-0008 | `ASR` (open) | The client service on the target: connection, authentication, the test account, log and screenshot streaming, updates through `updater` |
| `init`'s grants (not numbered yet) | `KRN` | TLS, a `netpolicy` flow to the server, the log read grant, screen capture and the test account's spawn rights for the effector; numbered by the kernel track when 501-ASR-0008 needs them |

501-ASR-0007 and 501-ASR-0008 are numbered here; their files are written when work on them starts.

## Acceptance criteria

- In QEMU: the server sends a bundle, the effector runs it in the test account, and the results, the log and a screenshot come back. A test that uses a capability its bundle did not declare is refused. A broken connection is resumed or reported.
- On the MacBook Pro, over the network of 550: one of the QEMU suites' checks runs unattended, and its record names the machine's configuration.
- [docs/assurance/README.md](../docs/assurance/README.md) and the profile say what the effector's runs cover, and that they are not proofs.

## Related

[500](500-fuzzing-abi-and-idl.md), [350](350-signed-boot-images.md), [351](351-self-update.md), [550](550-network-on-real-hardware.md), [650](650-building-on-the-target.md) (builds through a server), [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), `tests/qemu_smoke.py`, [docs/profile/evidence.md](../docs/profile/evidence.md).
