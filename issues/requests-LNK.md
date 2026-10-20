# Requests for the link track (LNK), not numbered yet

**Owner:** link track (the assessing session, `claude/LNK-link`, TRACKS 1.8) · **Status:** open (3 requests, 2026-10-20) · **Recorded by:** the kernel track (KRN), at the maintainer's request of 2026-10-20

The link track numbers its own tasks (`NNN-LNK-MMMM`), so requests from other tracks wait here. The link track turns each into a task and removes it from this file, and the file goes when it is empty.

## Starting a program for the master (800-KRN-0132)

**Recorded by:** the kernel track (KRN), 2026-10-20, for [800-KRN-0132](800-KRN-0132-link-starts-programs-for-the-master.md).

### Problem

The maintainer's agent can put a program into `data/link/`, but only someone at the machine's keyboard can start it (`run data/link/<name>.elf`). The maintainer wants to test programs without a reboot and without a person at the machine.

### Plan (a proposal; the link track decides)

- **What `init` gives (800-KRN-0132, in `fast-test`).** `link` holds `loader`'s client in `SLOT_LOADER` (5), as an application does.
- **A RUN request** in `link` and `mindlink` (`mindlink run <name> [args]`):
  - `link` starts `data/link/<name>` through a launch session (`begin`, `grant`, `commit`).
  - It grants the program an endpoint of its own in `SLOT_CONSOLE` (9), so what the program prints (`mind::output`) comes to `link`. It answers the program's PID.
- **Output and exit.** The program's output goes back over the console channel (1), as protocol section 5 has it. When the program ends, `link` says so with its exit status, where it can learn it.
- **Stopping** a program that does not end is not in this request. It needs another grant from `init`, still to be decided.

### Acceptance criteria

The `link` suite puts a test program into `data/link/`, starts it with `mindlink run`, reads its output and its end, and a name outside `data/link/` is refused.

## Files in subdirectories of `data/link/`, and moving them

**Recorded by:** the kernel track (KRN), 2026-10-20, at the maintainer's request ("place files and do commands, like move").

### Problem

- **Flat names only.** `link` serves only single-component names (protocol section 7). So the agent cannot lay out a tree in `data/link/`, for example:
  - a program with its data files;
  - a release in the layout `scripts/release.py` makes (`blobs/`, `releases/<version>/<arch>/`, `channels/`), which the updater could read as a directory source.
- **No file commands.** There is nothing to move, rename or make a directory with, so a file must be put again under its new name.

### Plan (a proposal; the link track decides)

- **Relative paths** under `data/link/` in every file request:
  - components separated by `/`;
  - no `..`, no empty or `.` component, no drive or volume prefix.
- **`vfs_server` is the real limit.** The scope `init` gives `link` keeps every path inside `data/link/`, whatever `link` checks.
- **New file requests:**
  - `MKDIR`;
  - `MOVE` (a rename within `data/link/`);
  - `DELETE` for an empty directory, or a recursive one if wanted;
  - `LIST` of a subdirectory.
- **Compatibility.** A slave that does not know a request answers `UNSUPPORTED`. If the protocol's version changes, say so in `docs/link/protocol.md` (MC-12.4).

### Acceptance criteria

- The `link` suite does all of these, and `ls` shows each step:
  - makes `a/b/`;
  - puts a file there;
  - moves it to `c/`;
  - deletes `a/`.
- `../x`, `a/../../x` and an absolute name are refused, by `link` and by `vfs_server`.

## `effector485`: a small Effector over the link, for local use

**Recorded by:** the kernel track (KRN), 2026-10-20, at the maintainer's request: "link has to be like a small Effector … let's create effector485 for local use … with the ability to run, move and change programs, as a pair with a Linux or Windows program".

### Problem

The maintainer's agents drive the Effector machines through operations:
- each with an id, an idempotency key and a reason;
- read for their status and output;
- cancelled by id.

A MIND Core machine on the RS-485 cable has only `mindlink`'s one-shot commands. An agent cannot hand it a batch of work and come back to it.

### Plan (a proposal; the link track decides, with `EFF` for the operation contract)

- **A pair of programs:**
  - **on the host,** `effector485` for Linux and Windows. It is a daemon with a command-line client, built on `mindlink`'s code. It keeps the queue of operations and their results, and drives the cable;
  - **on MIND Core,** `link` (or a service beside it) carries the operations out with only the grants `init` gives it.
- **Operations, each with an id, an idempotency key and a reason:**
  - put, get, move and delete files and directories in `data/link/` (the request above);
  - run a program from `data/link/` with arguments, collect its output and exit status, and stop it (800-KRN-0132, and the grant still to be decided);
  - read the system log from a sequence number;
  - fetch the hardware report;
  - later, and only with a word in `link.conf`, restart the machine through `init`.
- **"Change programs".** Install, replace and remove programs under `data/link/apps/`, run from there. Writing the volume's root, where the built-in applications live, needs a wider scope from `KRN` and the maintainer's decision.
- **Status, output and cancel** by operation id, as on Effector (`docs/effector.md`). Reuse the Effector contract (750-EFF-0001) where it fits: `EFF` is an open track.
- **Security.** The cable is unauthenticated (protocol section 7), so each power beyond files is opted into by a word in `link.conf`. `init` grants nothing more without it.
- **What it needs from other tracks:**
  - `KRN`: ending the programs `link` started; a lifecycle or reboot client if wanted; wider scopes if wanted;
  - `UPD`: an update from a release staged in `data/link/` (the updater's directory source, built but not yet tested).

### Acceptance criteria

The link track sets them when it numbers the task (a main task in 800–849, or a task of 800). At least:
- a QEMU suite drives `effector485` from the host through a batch: put, mkdir, move, run with output, log read;
- a repeated operation with the same idempotency key is done once;
- `cancel` stops a running program.
