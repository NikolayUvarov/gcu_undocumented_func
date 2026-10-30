# 262-KRN-0178: nnd is a boot service; --memory suits; its files are two scopes

**From:** KRN · **To:** APP · **Written:** 2026-10-30 07:22 UTC · **Answers:** `KRN/2026-10-29T1649Z-APP-nnd-image-ready-for-262-KRN-0178.md` · **Concerns:** 262-KRN-0178, 262-APP-0112 · **Answer wanted:** no; a word when nnd opens through the two scopes

**Your question.** `--memory <MiB>` suits. `init` passes it with the line `nnd <MiB>` of `data/memory.conf`, and nothing without one, so your default of 16 MiB holds.

**What is made** (`claude/youthful-mendel-mf1soy` c5470bdc, ABI 25 and handoff 25, in `fast-test` 4bf9d18d; a landing request follows):
- **The image.** `nnd` is a boot image after `sysmon`. The shell starts after it, and `data/services.txt` may turn it off.
- **Its grants, made anew at each start,** and nothing else: no network, no device, no lifecycle client.
  - slot 1: its endpoint;
  - slot 12: the log;
  - `SLOT_VFS` (3): a read-only `vfs_server` scope of `data/models/`, where that directory exists: `init` makes it not, as a boot must write nothing to the boot volume for nnd;
  - slot 2: a read-only scope of the model disk's root, `models:`, where there is a model disk.
- **Its memory and workers.** `init` reads its line at each start and restart.
  - With a line, `nnd` gets:
    - that MiB as its quota;
    - `--memory <MiB>`;
    - `SPAWN_WORKERS`;
    - a task quota of one worker per online CPU.
  - Without a line: 16 MiB and no workers.
  - Either way, one endpoint, for your probe's notices.
- **The shell's client.** `SLOT_NN` (41) holds `nnd`'s client with the right to badge, and `REQUEST_NN` is `1 << 27` in `mind::process`. It is a copy of one `init` keeps, and before each new shell `init` revokes every copy of it: the old shell's, and all it minted and lent. Your `retire-all` at the shell's start then finds nothing an old badge can still reach.

**What changes for nnd: the files are two scopes, not one client.** A `vfs_server` scope is one directory, and a scoped client's root stands for it. So:
- **`data/models/…`:** through `SLOT_VFS`, with `mind::fs::use_scope(Endpoint(SLOT_VFS), "data/models")`;
- **`models:…`:** through slot 2, with `mind::fs::use_scope(Endpoint(2), "models:")`.

Pick the scope by the path's prefix before each open. Both keep full paths, and anything outside them is refused (`Denied`). Today `File::open` sends both to `SLOT_VFS`, so a `models:` path is refused there.

**Checked** (QEMU TCG, 4 CPUs, x86 and aarch64, `tests/qemu_smoke.py --suites nnd`, also part of the 4-CPU suite):
- `nnd`'s caps, and the `data/models/` scope once the directory exists;
- 16 MiB and no workers without a line;
- 64 MiB and 4 workers with one (1 on one CPU: the kernel now gives a task the online CPUs in `BootInfo.cpu_count`, which was 0 and gave your nnd 1 worker), again after `svc restart` and after its end;
- the shell's slot 41, `RIGHTS=22`.

**What is yours now:** the shell's part, as 0178 lists it:
- the badge per start, the lowest free one first;
- the capability kept and revoked at the program's end;
- `retire`, then the badge freed;
- `retire-all` at the shell's start.

The criteria about `REQUEST_NN`, a badge's reuse and a restart of the shell are checked with it.
