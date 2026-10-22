# 800-LNK-0018 — `effector485` shows the cable's traffic: bytes each way since it started, and the speed

**Type:** tool · **Owner:** `LNK` · **Priority:** P2 · **Status:** in progress (2026-10-22) · **Blocked by:** — · **Main task:** [800](800-mind-link.md), through [800-LNK-0017](800-LNK-0017-effector485-page.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

The maintainer asked on 2026-10-22 for "the bytes sent and received in total (on the current effector485 run) and the speed, in the browser and, if possible, in the console too, when data is transferred". Until then neither the page nor the daemon's log showed how much went over the cable, or how fast.

## Plan

- **The ports** (`port_unix.rs`, `port_windows.rs`) count the bytes written and read, in two process-wide atomic counters.
- **The feed** sends them every second as `traffic` (`sent`, `received`, the daemon's clock in ms, `baud`). It reads them with no lock, so it adds nothing to the cable's work (800-LNK-0017).
- **The page** shows ↑ sent and ↓ received, the speed each way over the last 4 s, and how busy the line is at ten bits a byte.
- **The daemon's log**, every 5 s in which bytes moved: `the cable: sent … (…/s), received … (…/s); the line …% busy`.
- **`GET /api/v1/endpoints`** answers `sent_bytes` and `received_bytes`.

## Acceptance criteria

- [x] **Host tests:** the feed's `traffic` with bytes each way after the operations, and the endpoint's totals (`the_page_its_feed_and_other_sites`); the sizes' format (a unit test).
- [x] **Against `mindlink serve` over a pseudo-terminal**, a 3 MB put: the log's line, the feed's messages each second during the put, and the page's header in a headless Chromium.
- [ ] **On the maintainer's machine:** the Windows build on COM22, the speed seen during a put to the MacBook Pro.

The figures are what this daemon wrote to and read from its port, frames and their headers included. They are not the payload's rate (MC-12.3). Over a pseudo-terminal or TCP the line has no speed of its own, so the share of `--baud` can pass 100%.

## Related

[800-LNK-0017](800-LNK-0017-effector485-page.md), [docs/link/effector485.md](../docs/link/effector485.md).
