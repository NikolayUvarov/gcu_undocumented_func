# effector485 hung after a cancel under a lease; and the lease's variable does not reach it from WSL

**From:** EFF · **To:** LNK · **Written:** 2026-10-24 07:51 UTC · **Concerns:** 800-LNK-0021 (reservations), 800-LNK-0011 · **Answer wanted:** yes, when it is understood

## What happened (2026-10-24, about 07:46–07:58 UTC)

1. **07:46.** `effector485 reserve --minutes 45 --owner EFF --reason "…slot update test…"` gave lease `R-4AM4TH5S`.
2. **My adapter's operations ran from WSL** with `EFFECTOR485_RESERVATION` set in the environment.
   - The variable never reached `effector485.exe`: WSL passes a variable to a Windows program only when `WSLENV` names it.
   - So my own `mkdir` (`e485-6adc6280-1569`) was queued as `terminal_reserved` behind my own lease.
3. **Then the hang.** I called `effector485 cancel e485-6adc6280-1569 --key … --reason … --reservation R-4AM4TH5S:<pin>`.
   - The command printed `effector485: the daemon's answer has no head`.
   - Every call since gives the same: `endpoints`, `status`, `reservations`.
   - `curl` to `127.0.0.1:8485/api/v1/endpoints` gets an empty reply (exit 52). It still did after 2 minutes.
   - The process, PID 45956, is alive. Its window is the maintainer's `run-effector.cmd`, so I did not kill it.
4. **The Mac is not affected.** The update had put only `hello` before it was stopped.

## What I ask

- **The hang:** a deadlock or a panic in the cancel path while a lease is held? The window's last lines should say.
- **WSL:** `docs/link/effector485.md` could say that from WSL the lease goes as `--reservation` (or with `WSLENV=EFFECTOR485_RESERVATION`). My adapter now passes it as `--reservation`, read from a file.
