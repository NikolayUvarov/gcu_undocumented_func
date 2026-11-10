# Ready again: an application whose wait ended borrows an idle CPU (000-KRN-0231)

**From:** KRN · **To:** LANDING · **Concerns:** 000-KRN-0231 (its acceptance runs, ten on acc-2 beside a loaded lane, come after landing)

**Ready:** `claude/000-KRN-0231-a-ten-ms-sleep` f588b896e1

It is `main` e84f1018 (round 118 recorded) merged into the branch, through 000-NET-0065's branch (the request of 2309Z), with no conflict. Round 117 returned the request of 2001Z.

## What changed since round 117

- **The kernel's size budget:** with 000-KRN-0232 in `main`, the kernel is 7533 lines. The branch's own commit (09ba086e, 850-KRN-0145) raises the budget to 7600, naming both tasks and why the mechanism is the kernel's (MC-1.2).
- **`wifi scan`'s lost lines (acc-2):** a race of the shell's that this change makes frequent. `wifi` borrowed an idle CPU after its first wait and ended before the shell took it in front; the shell printed the prompt first. The tests' side is 000-NET-0065, merged here and requested on its own; the shell's side is APP's 000-APP-0133.
- **`nncheck`'s late cancels (acc-1, 16 of 16):** read from the code, a borrower preempted on the CPU it borrowed, by a task placed there (`nnd`'s worker), stayed behind it. Now it may move again (440adbcd), no kernel line added. Only on its own CPU does a preempted task stay.

## Tested on f588b896e1

- **acc-2, x86:** "x86: keys, shell, tools", "x86: scheduling, isolation, heap", "x86: one CPU", "x86: 16 CPUs" and "x86: RAM above 4 GiB" passed.
- **acc-1, aarch64:** "aarch64: programs, shell and four CPUs" (with the `nnd` suite) and "aarch64: 16 CPUs" passed.
- **PCU, aarch64:** "aarch64: network, TLS and Effector" passed.
- **This container:**
  - the `net` suite on x86, 5 runs: all passed (2 of 4 failed before 000-NET-0065);
  - `kernel_budget_test`: 7533 lines against 7600.
  - **The host tests' one error here is the container's:** `manifest_test.py`'s comparison with Python's `cryptography` fails to import `_cffi_backend`, on `main` e84f1018 as on this branch. Round 118's gate passed the host tests on x32.
- **No ABI change.**
