# 801-EFF-0037 — A cut download goes on from where it stopped (`Range`), and a body may pause 60 s

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-21) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S7, S8) · **Constitution:** MC-12.1

## Problem

KRN's request of 2026-11-19, "A cut archive download goes on from where it stopped" in `issues/requests-EFF.md` (with 801-NET-0088), taken here. DRV's reports agree (`EFF/2026-11-19T2235Z-DRV-…`, `EFF/2026-11-20T1739Z-DRV-…`, and its message of 2026-11-21).

- **The DEXP, 2026-11-19:** six of ten `update_agent` downloads of a 29 MB release were cut after 184 to 439 s, with `EFFECTOR: /api/commands/file BODY: Transport` and the command ending `upload`. The four taken lasted about 9 minutes each.
- **The Ryzen and the DEXP, 2026-11-19 and 20:** one 5 s stall from the server's address each time, and 50 s and 149 s of download lost.
- **The Ryzen, 2026-11-21:** two runs ended in `upload`, one with `BODY: Closed`. Each rtw89 restart drops netstack's address and aborts every connection (213-DRV-0032 makes them rarer, not impossible).
- **Why each was lost:**
  - `Link::receive` waited `REQUEST_MS` (10 s) for a byte, in a body as in a short request;
  - every try began again from the first byte.

## Plan

1. **A download's body waits `BODY_MS` (60 s) for a byte.** A short request still waits 10 s, and init's stop notice still cuts either short (801-EFF-0036).
2. **A cut download goes on** with `Range: bytes=N-`, from the bytes the gateway already holds: a whole number of its 32 KiB pieces, as a piece not yet written is fetched again.
   - Up to `RESUMES` (5) times, after a pause that doubles from 2 s, and never past the command's time (`Origin.deadline_ms`).
   - `send` checks the answer: a `206` must start at N (else the try ends `upload`), and a `200` is the whole file, whose first N bytes are skipped. The server answers ranges through Go's `http.ServeContent` on both routes (`server/artifact_stream.go`, `server/api.go`).
   - The gateway checks the whole file's size and SHA-256 at the commit, as before.
3. **Each try is logged** with its offset: `EFFECTOR: <id>: THE DOWNLOAD CUT AT <N> OF <size> BYTES: GOING ON FROM THERE IN <s> S (<try> OF 5)`, or why it was given up.
4. **The fixture** answers `Range` as `http.ServeContent` does (206 with `Content-Range`, or 416), and its `cut_download` fault works inside a range too.
5. **The `effector-update` suite** cuts the body once halfway. The agent's second request asks the rest from the bytes the gateway held, and the update is taken.

## Acceptance criteria

- [ ] Fixture tests (`tests/effector_server_test.py`): a range answered 206 with its `Content-Range`, a cut inside it, 416 past the end.
- [ ] The `effector-update` suite, x86 and aarch64: cut once halfway, resumed with `Range` from a whole number of pieces, and the update taken.
- [ ] On the bench (DRV's Ryzen, over Wi-Fi alone): a release taken through `update_agent` across a provoked link reset (MC-12.1).

## Related

[801-EFF-0038](801-EFF-0038-the-heartbeat-throughout-a-download.md), [801-EFF-0036](801-EFF-0036-the-last-heartbeat-however-a-request-hangs.md), [751-EFF-0023](751-EFF-0023-the-agent-installs-a-developers-release.md), 801-NET-0088, 213-DRV-0032.
