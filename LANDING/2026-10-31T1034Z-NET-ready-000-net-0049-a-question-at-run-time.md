# NET landing request: 000-NET-0049's question at run time, the stack's and the broker's part (socket.wit 2.8, netpolicy.wit 1.3)

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-10-31 10:34 UTC · **Concerns:** 000-NET-0049 (advanced; the shell's part is APP's 000-APP-0120)

**Ready:** `claude/youthful-mendel-mf1soy` dea25f46ddf497b012654761da0c335a381b5882

`main` ceeb301d is merged. dea25f46 differs from the tested 32ea39e1 in issue files only.

## What it holds

- **`idl/socket.wit` 2.8**, for a grant's badge: `ask(host, port, why)` and `asked(id)`.
  - For the policy badge: `questions()` and `decide(id, address, allowed, once)`.
  - `netstack/src/questions.rs` keeps the questions: one a grant, 8 waiting.
  - `policy.rs` adds the answered rule to the grant. A rule given once ends at the grant's first connection through it.
- **`idl/netpolicy.wit` 1.3:** `questions()` and `answer(id, choice)` for the shell's client; a launcher's is refused.
  - The broker takes the stack's questions on its 1 s pass and names each one's program and PID.
  - It refuses a question at 60 s, and drops one whose program ended.
  - `always` adds the line through `add`.
- **`netcheck ask:HOST:PORT:SECONDS`**, and the `net` suite's `browse_check` checks the two ends that need no shell.
- **Generated bindings, the IDL fuzzer's types, `docs/network/policy.md` 0.6.**
- **Not changed:** the ABI, and every earlier call of either interface (both are minor versions).

## Tested (32ea39e1, through Effector)

- Host tests: PASS (`un-rizen`), with `net_questions`, `idl_fuzz` and `netpolicy`.
- "x86: network, TLS and Effector": PASS (`acc-2`).
- "aarch64: network, TLS and Effector": PASS (PCU).
- "aarch64: network above 4 GiB": PASS (`acc-1`).
- "aarch64: GICv2 with GICv2m": PASS (`un-rizen`).
- "x86: services, storage, audio": PASS (`acc-1`).
- "x86: RAM above 4 GiB": PASS (i9).

The question's line was in the `net` suite on both architectures. Locally (x86, TCG), `browse_check` alone passed too.
