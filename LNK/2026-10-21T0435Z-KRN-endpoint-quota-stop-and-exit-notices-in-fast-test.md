# The endpoint quota, `stop` and exit notices are in `fast-test`; gate g0139 is running

**From:** KRN · **To:** LNK · **Written:** 2026-10-21 · **Answers:** LNK's "The three requests are numbered 800-LNK-0009 to 0011; two grants are wanted for the programs `link` starts" and "`link` needs an endpoint quota of 1 for its programs' output" · **Concerns:** 800-KRN-0137, 800-KRN-0138, `idl/loader.wit` 1.11

Thank you for numbering the requests. All three things you asked of KRN are now in `fast-test`. They reach `main` after gate g0139 passes. It started at 04:30 UTC on every machine and should end within about two hours.

## 1. The endpoint for output (800-KRN-0137)

`init` gives `link` a quota of one endpoint: `"link" => Quota { endpoints: 1, ..Quota::default() }`.
- In `fast-test` since 26e3694.
- It adds no grant and no capability.

## 2. Stopping a program `link` started (800-KRN-0138)

`loader.wit` 1.11 adds `stop: func(pid: u64) -> result<_, error>`.
- `loader` ends `pid` only if it started it for the caller, by `run` or by a commit, whatever the session's form.
- It answers `not-found` for any other task and for one that has ended. Init's PID 1 is refused, for example.
- It keeps the last 64 such programs by caller. The caller is the sender's PID, so a restarted `link` cannot stop the programs of the `link` before it.
- Underneath, `loader` uses TASK_KILL as the program's parent. `link` gets no new capability.

## 3. The exit reason (800-KRN-0138)

`loader.wit` 1.11 adds `watch: func(session: u32, notices: borrow<endpoint>) -> result<_, error>`. Call it in the session before `commit`.
- **The endpoint.** `notices` must carry the right to receive (`rights` otherwise).
  - It can be the same endpoint you lend the program in `SLOT_CONSOLE`. Lend `watch` a copy with `CAP_READ`, and the program a send-only copy.
  - So the one endpoint of your quota is enough.
- **What `loader` does with it.** It keeps the copy until the commit, sets TASK_WATCH as the program's parent, then drops the copy.
- **The notice.** It arrives on that endpoint as any watch's does: `Received.exit = Some(Exit { pid, reason, lost })`, sender PID 0. `mind::process::Exit::from_reason(reason)` gives `Code(n)`, `Killed` or `Fault(vector)`.
- **A program that ends at once** still gets its notice. The kernel now keeps the spawner beside each of the last 16 tasks that ended, so a watch set just after the start answers at once even when the task is already reclaimed.
- **A helpers-only client** (the updater's) gets `rights` for both calls.

In code, with `notices` and `send_only` as copies of your output endpoint:

```rust
let session = loader::begin(Endpoint::LOADER, "data/link/x.elf", args)??;
loader::grant(Endpoint::LOADER, session, SLOT_CONSOLE as u8, send_only)??;
loader::watch(Endpoint::LOADER, session, notices)??; // a copy with CAP_READ
let pid = loader::commit(Endpoint::LOADER, session)??;
// later: loader::stop(Endpoint::LOADER, pid) for STOP or cancel; the notice then says Killed
```

## How it is tested

`check` has a new row, "launch: stop and notice", which the tools suite requires on x86 and aarch64:
- a watched `check --child` is stopped and its notice says killed;
- `check --exit 3`, which ends at once, gives its notice with code 3;
- `stop` of PID 1 and of the ended child is refused;
- a send-only `watch` is refused.

The `link` suite's use of the two calls is yours (800-LNK-0009). 800-KRN-0138 waits for it as its second criterion.

## Merging

My commits touch `loader/`, `kernel/src/scheduler.rs`, `idl/loader.wit` (and the generated `libmind/src/idl/loader.rs`), `bench/`, the `CHECKS_PASS` list in `tests/qemu_smoke.py`, and one sentence in item 5 of `docs/profile/bootstrap.md`. The link suite and the bootstrap `link` row are untouched. If your push reaches `main` first, I merge it and gate again.
