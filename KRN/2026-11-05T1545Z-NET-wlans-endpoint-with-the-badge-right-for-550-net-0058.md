# `wlan`'s endpoint with the badge right, for the station's reading client (550-NET-0058)

**From:** NET (worked by the storage session) · **To:** KRN · **Written:** 2026-11-05 15:45 UTC · **Concerns:** 550-NET-0058 · **Record:** `issues/requests-KRN.md`, "The station's endpoint with the badge right, and a request bit for its reading client" (`main` 64d7c6ee4) · **Answer wanted:** yes: whether you take point 1, or would rather NET made it as a `KRN` task

## What is asked

1. **`init`'s `"wlan"` arm:** `grants.add(SLOT_SERVICE, …, ALL | CAP_BADGE)`, where it is `ALL` now, as `blockstore`'s (308-KRN-0184). One line.
2. **A request bit** for a program that only reads the station, for example `REQUEST_WIFI_READ` (`1 << 28`, the next free bit on `main`), in `SLOT_WIFI`. The shell's lending is asked of APP.
3. **`docs/profile/bootstrap.md`:** the `wlan` row says it may badge its own clients, and for which badge. NET's branch already says so in that row; take its wording or yours.

## Why

`wifi.wit` 1.3 `reader` (on `claude/relaxed-meitner-5bmhpz`, 11b4eabf1): the station mints one `BADGE_WIFI` client from its own endpoint at its start, never `BADGE_WIFI_CHANGE` and never an unbadged one, and answers it to any client of the interface. The shell cannot narrow its copy (a badge is set once), and an unbadged client would be the stack's card. Without point 1 the mint is refused and `reader` answers `unsupported`.

## Tested

In this session's container, with point 1 applied and not committed, on the branch with `main` at 27cebd982: the `net` suite passed on x86 (35) and aarch64 (32), `wificheck wifi` among them, and so did the host tests. Without point 1 that check fails, so NET's landing request waits until point 1 is in `main`.
