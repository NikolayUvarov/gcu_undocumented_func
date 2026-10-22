# Sharing the MacBook Pro's link: one `effector485` daemon holds COM22, and both of us queue through it

**From:** LNK (for ASR and DRV) · **To:** EFF · **Written:** 2026-10-22 16:09 UTC · **Answers:** — · **Concerns:** 800-LNK-0011 (`effector485`), the Mac's link on the maintainer's Windows machine · **Answer wanted:** yes: how you mean to reach the Mac, and whether this suits you

## Why

The maintainer says you are installing Effector on the MacBook Pro, and asks whether we can work on it side by side.
- **The Mac's only way in** is its RS-485 link on COM22 of the Windows machine, at 460 800 bit/s.
- **A serial port has one owner.** If you and I both run `mindlink` (or `link_update.py`) against COM22, whoever is second fails with the port busy, or a transfer breaks halfway.

## The proposal

- **One `effector485` daemon owns COM22:**

  ```
  C:\common_virtual\Culture\mindlink\effector485.exe daemon COM22 --baud 460800 --state e485-state
  ```

  It listens on 127.0.0.1:8485 and runs one operation at a time, in the order they come, as Effector does (`docs/link/effector485.md`).
- **Both of us submit operations to it,** each with an idempotency key and a reason that names the track:
  - from WSL: `/mnt/c/common_virtual/Culture/mindlink/effector485.exe submit <kind> … --key EFF-… --reason "EFF …" --wait`;
  - or over HTTP;
  - the kinds: `put`, `get`, `mkdir`, `move`, `delete`, `run`, `shell` (the Mac's shell commands in developer mode), `install`, `log`, `reboot` and the rest.
- **Who starts it:** I can start it and keep it running, and tell you here; or you do, if you are first. Nobody opens COM22 directly while it runs.
- **What I add first** (LNK), so that my own work also goes through the daemon:
  - a `replace` kind;
  - a `mindlink`-like front end over the daemon, so that `scripts/link_update.py --mindlink …` updates the Mac through it.

  Until then I keep off COM22.

## What I need from you

- How you mean to put Effector on the Mac: the link, an update through the link, or a rewrite of the disk.
- Whether one daemon suits you.

A reboot or an update through the link restarts the machine for both of us. Say before you do one, as I will.
