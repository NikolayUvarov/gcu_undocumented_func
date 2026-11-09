# 000-KRN-0226 — An IDL client's call buffer is wiped before it is freed

**Type:** libmind (the IDL generator's clients) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress (made and tested on the host and in QEMU, x86 and aarch64, 2026-11-09; the gate left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-1.8, MC-11.9

**Asked by:** APP, 2026-11-09, in `requests-KRN.md` ("The generated binding's buffer of `shell.wit` `secret-line` wiped before it is freed"), from NET's remark in [000-NET-0048](../issues-done/000-NET-0048-get-a-wget-and-curl-analog.done)'s resolution.

## Problem

- **The secret line's path:** a console program's secret line (`shell.wit` 1.12, [000-APP-0126](../issues-done/000-APP-0126-a-console-program-reads-a-secret.done)) comes back in the call's buffer.
- **The generated binding** (`mind::idl::shell::secret_line`) copies it into the caller's `out` and drops its `Pages`, which are freed (`SYSCALL_FREE`) with the line still in them.
- **What the kernel does:** it zeroes task memory when it gives it out (the profile, MC-1.8), so no other task reads it. Between the free and the next allocation, the bytes stay in the frame.
- **Other secrets travel the same way:** keystore's PMK (`wifi-remember`, `list<u8, 32>`), the keys `wifi-keys` returns (`list<u8, 48>`), signatures and secrets in `bytes`. So a mark on `bytes` results alone would miss some.

## Plan (the owner's choice of APP's two ways)

- **The generator wipes every client's call buffer,** whatever its types:
  - `mind::idl::wire::Wiped` wraps the call's `Pages` and overwrites them with zeros, by volatile word stores, when it is dropped, before the pages are freed;
  - every buffered client function makes its buffer as a `Wiped`, so every way out wipes it, an error's too.
- **Why not in the kernel at `FREE`:** every free of every program would pay for it, a 64 MiB block too. The buffers are 64 KiB at most, and are wiped where the secret was.
- **No interface change:** the wire and the WIT do not change, only the generated code (MC-12.4). The generated files are regenerated with `scripts/mind_idl.py`.
- **Not covered:** a word call's message words, which the kernel's mailbox page holds until the next call; and the server's copies, which are each server's to wipe (the shell wipes its line, `keystore` its keys).

## Acceptance criteria

- **A host test:** a client's buffer is freed with no byte left in it, after a request's bytes, a reply's bytes and a refused reply. The test is shown to fail without `Wiped`.
- **QEMU:** the suites that use the bindings pass, `get -u`'s among them.
- **docs:** the profile's MC-1.8 line and `docs/idl/README.md` say so.

## Progress (2026-11-09)

- **Made:**
  - `libmind/src/idl/wire.rs`: `Wiped`, which the IDL's host tests compile with their stand-in `Pages`;
  - `scripts/mind_idl.py`: buffered clients make a `Wiped`;
  - the 33 generated interfaces and `tests/idl/sample.rs` regenerated.
- **Tested** on the host:
  - `tests/idl_host.rs`, `a_clients_buffer_is_wiped_before_it_is_freed`: its stand-in `Pages` counts buffers freed with a byte not zero. None was, after `put`, `fetch`, a refused `fetch` and `find`.
  - With the sample regenerated without `Wiped`, the test failed.
  - `tests/idl_fuzz_host.rs` passed, and `mind_idl.py --check` found every file up to date.
- **Tested in QEMU** (the kernel session's container, QEMU 8.2.2, TCG, 4 CPUs), after fresh builds of the tree:
  - x86: the `shell` and `net` suites, 54 checks, none skipped. The secret line's check (`secret_line_check`) and `get_runs` with `-u` typed are among them;
  - aarch64 `virt`: the same suites, 51 checks. The 3 skipped need x86's test programs, as on every aarch64 run.

## Related

[000-APP-0126](../issues-done/000-APP-0126-a-console-program-reads-a-secret.done), [000-NET-0048](../issues-done/000-NET-0048-get-a-wget-and-curl-analog.done), [docs/idl/README.md](../docs/idl/README.md).
