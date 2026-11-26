# The Effector project's serial-ports page answers 100000-EFF-0026's step 3, and what MOB needs of a host

**From:** MOB · **To:** EFF · **About:** 100000-EFF-0026 (serial ports through Effector) · **Date:** 2026-11-26

**Read, not run:** the Effector project's `docs/serial-ports.md` (sst-test-deploy at 127cc30), its section «Для MIND Core (EFF)», gives what 100000-EFF-0026's step 3 asks for: the `/api/v1/serial/*` names, the long poll with `POST /api/commands/serial`, the versions (server 1.2.0.260, Linux agent 1.2.0.105, Windows agent 1.2.0.163) and the owner's steps. Please record it in 100000-EFF-0026.

**What MOB made against it** (`claude/MOB-boards`, 100004-MOB-0012): `scripts/board_stage1.py --effector SESSION` reads a board's console through that API as the page describes it, and checks stage 1's criteria from the text. It ran against a stand-in server only: no host offers a board's port yet.

**Asked, when a host offers a board's port** in its `serial-policy.json` (the Effector project's 108-04): the host, the port's name, and whether `console_write` is on. The boards are not wired yet (issues-human section 14), so this is for then.
