# 800-LNK-0017 — `effector485`'s page in a browser: a live feed over a WebSocket, and buttons for the usual operations

**Type:** tool · **Owner:** `LNK` · **Priority:** P2 · **Status:** in progress (2026-10-22) · **Blocked by:** — · **Main task:** [800](800-mind-link.md), through [800-LNK-0011](800-LNK-0011-effector485.md) · **Constitution:** MC-2.3, MC-10.2, MC-12.1, MC-12.3

## Problem

The maintainer asked on 2026-10-22 whether `effector485` shows that it works and what it was asked:
- "is there an indication of its work and a log of the commands, to open in a browser and watch? If not, let's add it";
- "better still, not only the log but a row of commands that can be called with a button";
- "the page should show the data through a WebSocket, and sending to the page must in no way hinder the main exchange".

Until then the daemon answered JSON alone, and only to programs.

## Plan

1. **`GET /`:** one page, built into the daemon, needing nothing from the internet:
   - the cable's other end, the queue, the newest operations, and the output of the one clicked;
   - buttons for `hello`, the log, `ip`, `netstat -i`, `ps`, a shell line, a program, a directory's list, `replace` and its way back, and `reboot`, each an operation with its own idempotency key and the reason typed on the page.
2. **`GET /api/v1/stream`:** a WebSocket feed (RFC 6455, written here, with no new dependency).
3. **The cable first:**
   - a feed never waits for the state's lock (`try_lock`; a busy lock skips a tick);
   - it looks at the state only when a change counter moved, or every 2 s;
   - it copies a few fields and at most 48 KiB of output under the lock, and writes to the browser after letting it go;
   - a browser that stops reading for 2 s is dropped, and at most 8 feeds run at once.
4. **Other sites:** a request whose `Origin` is not the daemon's own, or whose `Host` is not a loopback name with its port, is refused (403). With `--key-file`, the page and the feed ask for the key.
5. **The worker** marks a change only when it takes an operation. Before, it marked one on every turn, so the state file was written five times a second with nothing new.

## Acceptance criteria

- [x] **Host tests** (`tools/mindlink/tests/effector485.rs`, `the_page_its_feed_and_other_sites`):
  - the page;
  - the feed following a program's output to its end, line for line;
  - ten operations done in their usual time while eight pages never read, and a ninth page refused;
  - the key asked and a wrong one closing the feed;
  - another site's `Origin` and another `Host` refused.
- [x] **The page in a headless Chromium** against `mindlink serve`: the feed live, the operations, an output followed from the address, light and dark, 1400 and 420 pixels wide.
- [ ] **On the maintainer's machine:** the Windows build on COM22 with the MacBook Pro, the page open beside the agents' operations.

Evidence for the configurations named only (MC-12.1). The page shows what the daemon records. It adds no check of its own on what an operation does (MC-12.3).

## Progress

- **2026-10-22:** written; host tests and the headless page pass. The Windows build is `C:\common_virtual\Culture\mindlink\effector485-page.exe`. It waits for the daemon on COM22 to be restarted with it, by EFF, who runs it, or by the maintainer.

## Related

[800-LNK-0011](800-LNK-0011-effector485.md), [docs/link/effector485.md](../docs/link/effector485.md), [docs/effector.md](../docs/effector.md).
