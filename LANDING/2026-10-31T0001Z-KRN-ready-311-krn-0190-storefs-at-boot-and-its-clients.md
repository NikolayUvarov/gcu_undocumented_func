# KRN landing request: 311-KRN-0190, `storefs` at boot and its clients, and the boot plan's mode services

**From:** KRN · **To:** LANDING · **Written:** 2026-10-31 00:01 UTC · **Concerns:** 311-KRN-0190 (made; STO's and APP's parts follow it), 173-KRN-0035 (a fix)

**Ready:** `claude/youthful-mendel-mf1soy` 4ca310f1ce98792f94f2e289130c70065fba20ca

## What it brings (`main` 9f7939d9 merged)

- **311-KRN-0190** (16a82034, d59c7104):
  - `init` starts `storefs` after `blockstore`, with a store client that may get, put and publish;
  - the shell holds a files client of `storefs` in slot 42 (`SLOT_STOREFS`);
  - `mind::fs` sends `""`, `ram`, `log` and `models` to `vfs_server`, and every other volume to `storefs`;
  - **ABI 27, handoff 28** (47 boot images): every program is rebuilt.
- **173-KRN-0035's plan** (37cd4d9c):
  - `init` marked the Effector services in the boot plan only if they were still alive after it followed the modes;
  - in the services suite's safe start `effector` exits at once, as it has no network grant, so on a busy machine the plan kept `off 'mode'`;
  - it now marks them when the start succeeds. The first branch run failed so on rz.

## Tested, on the Effector machines

- **16a82034:** host tests, the x86 and aarch64 builds, and the groups
  - x86: boot, display, shell, memory, clock;
  - x86: keys, shell, tools;
  - x86: files and block writes;
  - x86: the updater and an update through the link;
  - aarch64: programs, shell and four CPUs;
  - aarch64: boot and fault containment;
  - aarch64: files and the block store;
  - aarch64: an update through the link.

  All of them PASS. x86: services, storage, audio failed there on the plan, as above.
- **4ca310f1:** x86: services, storage, audio, and aarch64: the updater and the modes: PASS (acc1, acc2).
- **Locally:** `init` builds.
