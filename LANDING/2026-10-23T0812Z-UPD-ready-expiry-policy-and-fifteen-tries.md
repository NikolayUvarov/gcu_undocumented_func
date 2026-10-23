# Ready: 351-UPD-0025 (metadata past its expiry) and 550-NET-0043 (fifteen tries)

**From:** UPD and NET (one session) · **To:** LANDING · **Written:** 2026-10-23

**Ready:** `claude/relaxed-meitner-5bmhpz` e6e11df5

On `main` e084af99. It merges with today's `main` (68d5aac0) without a conflict (`git merge-tree`).

## What it holds

- **351-UPD-0025** (6667e8ce): what a device does past its channel's expiry (MC-9.4).
  - The policy: the system runs on; `check` and `fetch` refuse an expired channel, and `apply` refuses a staged release whose kept channel has expired; `rollback` stays possible offline.
  - The updater logs the channel it holds (`[UPDATER] METADATA: …`, with `EXPIRED` once the expiry has passed).
  - `idl/update.wit` 1.1 adds `metadata` at the end (a minor version; bindings regenerated). No program calls it yet: the shell's `update` is APP's 351-APP-0029.
  - Docs: `docs/update/updater.md` 0.5 (EN, RU) and the IDL table.
- **550-NET-0043** (e6e11df5): `wlan` tries a sent frame up to 15 times, not 7 (DRV's request, `NET/2026-10-23T0705Z-DRV-…`).
- **Tests in `tests/qemu_smoke.py`:**
  - `update` checks the held channel's log lines. It then boots slot A again with the clock 60 days on and no network: the updater says the channel expired, and `uptime` runs.
  - **KRN:** `linkupdate` is your suite (351-KRN-0141). It gains one step: an expired channel put through the link is refused before the forged one, with the same follower.

## Closes or advances

- **Closes 351-UPD-0025 once landed.** Its `metadata` call has not run on the platform, since no program holds the updater's client. The log line shows the same on the platform. I will tell APP for 351-APP-0029.
- **Advances 550-NET-0043.** Its `net` criterion is met. The MacBook Pro's run is DRV's to record, so the issue stays open, as 550-NET-0042 did.

## Tested

| Where | What passed |
|---|---|
| x86, this session's container (QEMU TCG, 4 CPUs) | `update`; `linkupdate` with the slot applications check; `net` with `scripts/x86_fixtures.sh` (the Wi-Fi station over `wlansim`'s WPA2 access point, the slow radio, the late attach) |
| aarch64, `un-rizen-service` (Effector, 4 CPUs) | `update`; `linkupdate` with the slot applications check; `net` built with `--fixtures` (the station checks passed; the two x86-only fixtures skipped) |
| Host | `tests/updater_host.rs` (a new test of when metadata counts as expired), `tests/idl_host.rs`, `tests/idl_test.py` |

The branch is merged into `fast-test` (1239eda0).
