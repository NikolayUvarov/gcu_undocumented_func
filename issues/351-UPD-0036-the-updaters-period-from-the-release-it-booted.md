# 351-UPD-0036 — The updater's period from the release it booted, not only from the stick's root

**Type:** `updater`, the build's slot files · **Owner:** `UPD` (the kernel session) · **Priority:** P2 · **Status:** open (made and tested on both architectures, 2026-11-10; landing left) · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-3.1, MC-9.3

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

## Progress (2026-11-10)

On the branch `claude/351-UPD-0036-the-slots-period`:

- **The file's name is `period.txt`, not the plan's `UPDATE.TXT`.** A release's `update.txt` would be a file of the root to the updater (`plan::kind`: what is not a boot file, the loader or an application is shared), so it would replace the root's.
- **`updater/src/plan.rs`:**
  - `Kind::Period` for `period.txt`, a file of the slot;
  - `plan::period` reads one `every SECONDS` with comments, and refuses any other line, a second `every` or none.
- **The updater at start**, for the slot it booted:
  - It checks three things: the slot's manifest is the one booted (its digest the launch record's), `period.txt` matches its line, and it holds `every` alone.
  - If all hold, its period replaces `update.txt`'s: `[UPDATER] EVERY 7 S, BY SLOT B'S period.txt (update.txt'S: 20 S)`.
  - Otherwise `update.txt`'s stays, and the log says why: `… IS NOT AS THE MANIFEST LISTS IT`, `… REFUSED (source …)`, or a manifest not the one booted.
  - `source`, `pin`, `channel`, `automatic` and `tries` stay the root's.
- **Staging and applying:**
  - `fetch` stages it with the slot's files.
  - `apply` checks it as listed and holding `every` alone, and refuses a slot with one the manifest does not list.
  - The block store keeps it with the release (351-UPD-0033).
- **The build:**
  - `MIND_UPDATE_EVERY=60` makes `02_build.sh` and `scripts/build_aarch64.sh` write it before signing. Without the variable a stale one is removed.
  - `sign_manifest.py` lists it.
  - `make_usb_image.py --slots` and `boot_slots.py layout` put it in slot A.
- **Docs:** `docs/update/updater.md`, "A release's own period: period.txt".

**Tested** at 3fa050ae:
- **Host:**
  - `tests/updater_host.rs`: the kind and the parser, 17 tests;
  - `tests/usb_image_test.py`: the slot layout.
- **The `update` suite** checks all three criteria:
  - release 5's `period.txt` names a source, and is refused at the first boot;
  - release 6's `every 7` is staged into slot B, used there, and three checks follow, each under 18 s after the one before (update.txt's would be 20 s and the check's time);
  - slot A's, changed on the disk before the last boot, no longer matches the manifest and is not read.
- **Where it ran, through Effector:**
  - acc-1: "x86: the updater and an update through the link", passed in 835 s;
  - acc-2: "aarch64: the updater and the modes", passed in 751 s.

## Related

[351](351-self-update.md), 800-LNK-0040, [751-UPD-0032](751-UPD-0032-install-a-developers-release-from-the-gateway.md).
