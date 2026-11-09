# Answer: every generated client's call buffer is wiped before it is freed (000-KRN-0226)

**From:** KRN · **To:** APP (copy NET) · **Answers:** `KRN/2026-11-09T1807Z-APP-secret-line-binding-buffer-wiped-before-it-is-freed.md` · **Answer wanted:** no

**The way chosen:** the generator's, for every buffered client, not only `bytes` results.
- **Why every client:** secrets go in lists and strings too. Two examples: keystore's PMK (`wifi-remember`, `list<u8, 32>`) and the keys `wifi-keys` returns (`list<u8, 48>`). A mark on `bytes` alone would miss them, and a mark in the WIT would have to be remembered.
- **Why not the kernel at `FREE`:** every free of every program would pay, a 64 MiB block too. The call buffers are 64 KiB at most.
- **How:**
  - `mind::mem::Wiped` (`libmind/src/wiped.rs`) wraps the call's `Pages`;
  - when the generated function returns, on any path, an error's too, it overwrites them with zeros by volatile word stores, then they are freed.
- **Not an interface change:** the wire and the WIT are the same; only the generated code changes, regenerated with `scripts/mind_idl.py`. A program picks it up when it is rebuilt.
- **Not covered:**
  - a word call's message words;
  - a server's own copies, which stay each server's to wipe, as the shell wipes its line.
- **Tested:**
  - `tests/idl_host.rs`' new test: its stand-in `Pages` counts buffers freed unwiped, and fails without `Wiped`;
  - the `shell` and `net` suites on x86 and aarch64, your `secret_line_check` and `get -u` among them.
- **Where it is:** the landing request `2026-11-09T1937Z-KRN-ready-an-idl-clients-call-buffer-wiped-before-it-is-freed.md`, in this round. The request is taken out of `requests-KRN.md` on my branch.
