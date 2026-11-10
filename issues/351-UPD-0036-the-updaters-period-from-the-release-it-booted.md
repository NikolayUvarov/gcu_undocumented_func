# 351-UPD-0036 — The updater's period from the release it booted, not only from the stick's root

**Type:** `updater`, the build's slot files · **Owner:** `UPD` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-3.1, MC-9.3

LNK's request for the maintainer, 2026-11-09 (`KRN/2026-11-09T1830Z-LNK-the-updaters-period-reached-by-an-update-not-only-by-a-rewrite.md` on `internal-dev-messages`). The maintainer wants the updater to look once a minute, and to set that without rewriting the stick.

## Problem

- **Where the period is today:** `every` lives in `update.txt` at the boot volume's root, which nothing in the running system writes.
  - So neither the Effector agent nor a release (over the cable, over Effector, or from the server) can change it.
  - Only the next write of the stick can (800-LNK-0040 writes `every 60` then).
- **Why the root is read-only:** for `source` and `pin` that is right. They name where releases come from and whom to trust.
- **`every` is no such anchor.**

## Plan (LNK's form (a))

1. **The slot carries the period.** A release may carry `update.txt` in its slot directory (`MIND/<slot>/UPDATE.TXT`), listed in its manifest and checked as the boot files are.
   - It holds `every` alone. Any other word there is refused and named in the log.
   - The build writes it when asked: `make_usb_image.py` and the release tools take the period.
2. **The booted slot's word wins.** The updater reads the booted slot's `every` and uses it in place of the root's. It logs which one it uses and why.
   - The file must match its manifest line: the size and SHA-256 the bootloader verified.
   - `source`, `pin`, `channel`, `automatic` and `tries` stay the root's alone.
3. **Docs.** `docs/update/updater.md`'s `update.txt` section.

## Acceptance criteria

- **The `updater` suite** (x86, aarch64):
  - a release whose slot carries `every 60` is applied and booted, and the updater then checks every 60 s although the root says otherwise;
  - a slot's `update.txt` with `source` or `pin` in it is refused, and the root's period kept;
  - a slot file not matching its manifest line is not read.
- **Host tests** cover the slot file's parser.

## Related

[351](351-self-update.md), 800-LNK-0040, [751-UPD-0032](751-UPD-0032-install-a-developers-release-from-the-gateway.md).
