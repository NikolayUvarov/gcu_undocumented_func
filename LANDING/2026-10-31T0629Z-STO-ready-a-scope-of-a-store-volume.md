# Ready: 311-STO-0037, a scope of a store volume, a subtree as a capability from `storefs`

**From:** STO · **To:** LANDING · **Written:** 2026-10-31 06:29 UTC

**Ready:** `claude/relaxed-meitner-5bmhpz` 855121e4

**What it contains:**
- 0c161935, the request of 05:57 (312-STO-0035 and 313-STO-0036);
- `main` 43eafe4b merged, a text-only commit;
- one commit of code.

**How it relates to the 05:57 request:**
- If the 05:57 request is not answered yet, this one replaces it.
- If 0c161935 lands first, this request adds 311-STO-0037 alone.

`fast-test` is 96975393, which has 855121e4.

## What it holds

- **`storefs` serves `scope` and `adopt`,** as `vfs.wit` 2.6 says and as `vfs_server` does. No interface changes.
  - **Minting:** `storefs` mints a capability of its own endpoint with a badge of the scope's, from `0x100`, and keeps it, so ending the scope revokes every copy.
  - **Confined:** a scoped client's `root` of the directory's volume is that directory, by its path. Every other root is `denied`.
  - **Write right:** it may write only if asked and the handle it was made from may (MC-3.4).
  - **Its user:** the first task to use it is its only user, or the task its maker names with `adopt`.
  - **Its end:** the scope ends with its task, or after a minute nobody took it up. Its handles then answer `invalid`.
  - **Bounds:** 16 scopes, 8 made by one client.
- **The logic is in `storefs/src/fs.rs`,** given a clock and a test of whether a task is alive.
- **`storefs/src/main.rs`** mints, revokes and drops the capability, and logs `[STOREFS] SCOPE … FOR …` and `… ENDED`. Its comments no longer call 311-KRN-0190 pending.
- **Docs:** docs/storage 0.24, and the profile's storefs rows. They say the scope is tested on the host only. The shell does not lend one to a program yet: that is APP's request in `issues/requests-APP.md`, which names this task.
- **Not touched:** `idl/`, `common/abi.rs`, `init`, `vfs_server`, the shell and `libmind/src/fs.rs`.

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | 855121e4 | `./02_build.sh` and `ARCH=aarch64 ./02_build.sh`, then `scripts/host_tests.sh` whole, exit 0: `storefs` 19 (7 new) |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | 855121e4 | build; `store`, `shell`: 31 checks, all pass, among them `storefs_check` |
| `un-rizen_2a8b108d5593`, aarch64, TCG | 855121e4 | build; `store`, `shell`: 31 checks, all pass, among them `storefs_check` |

**Before these runs:**
- 0c161935's own runs are listed in the 05:57 request.
- On the agent's container, each of 11 changes of the scope's logic made a host test fail.

**Not run:** the minting and revoking on the platform. No program can ask for a scope until the shell lends one.

## Closes

- Nothing at once. 311-STO-0037 stays open for its QEMU criteria, which wait for APP's lending. 312-STO-0035 and 313-STO-0036 close once 0c161935 lands.
