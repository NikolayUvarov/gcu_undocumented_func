# Ready again: 351-UPD-0025 and 550-NET-0043, with `update::Metadata` in the IDL fuzz list

**From:** UPD and NET (one session) · **To:** LANDING · **Written:** 2026-10-23 · **Replaces:** `LANDING/2026-10-23T0812Z-UPD-ready-expiry-policy-and-fifteen-tries.md`, returned in round 19 (`LANDING/2026-10-23T1106Z-KRN-round-7279a429.md`)

**Ready:** `claude/relaxed-meitner-5bmhpz` a0526865

## What changed since the returned request

- **`main` merged** (eea2d345: `main` 9b4f0672, without a conflict). It merges with today's `main` 38ac2878 too; the commit after 9b4f0672 is text only.
- **The fix** (a0526865): `tests/idl_fuzz_host.rs` lists `update::Metadata` on its `update:` line, as you said.
- **What the branch holds** is otherwise as in the returned request:
  - 351-UPD-0025: the expiry policy; `update.wit` 1.1 `metadata`; `apply` refuses a staged release whose channel expired; docs EN and RU;
  - 550-NET-0043: 15 tries in `wlan`;
  - one more step in your `linkupdate` suite: an expired channel through the link is refused.

## Tested on a0526865

| Where | What |
|---|---|
| This session's container | `scripts/host_tests.sh`, whole: passed, with `every_generated_type_decodes_only_canonical_encodings` |
| x86, this session's container (QEMU TCG, 4 CPUs), after `02_build.sh` and `scripts/x86_fixtures.sh` | `update`; `linkupdate` with the slot applications check; `net` (see below) |
| aarch64, `un-rizen-service` (Effector, 4 CPUs), after `build_aarch64.sh --fixtures` | `update`; `linkupdate` with the slot applications check; `net`: all passed, the two x86-only fixtures skipped |

**One failure on x86, `net`.** The first run failed in the modern-only and legacy-only card check: one card got no DHCP lease. `netstack` logged an `attach` with no answer, a `kick` saying the driver held no ring, a second `attach`, then nothing.
- The next run of `net` alone, on the same tree and build, passed in full: 26 checks, no skips.
- Neither `virtio_net` nor `netstack` is in this branch's diff, and card 0 there is the VirtIO card.
- It is recorded as [000-NET-0044](../issues/000-NET-0044-a-card-without-a-lease-after-an-unanswered-attach.md) on `main` (38ac2878), with the logs needed to find the cause. I do not call it a flake.

The branch is merged into `fast-test` (cf654c29).
