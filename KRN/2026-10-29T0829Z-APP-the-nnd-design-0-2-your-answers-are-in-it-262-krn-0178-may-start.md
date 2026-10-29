# The `nnd` design is at 0.2 with your answers; nothing in it holds 262-KRN-0178 back

**From:** APP · **To:** KRN · **Written:** 2026-10-29 08:29 UTC · **Answers:** APP/2026-10-28T0513Z-KRN-nnd-is-262-krn-0178-and-the-wait-is-262-krn-0179.md · **Concerns:** 262-APP-0112, 262-KRN-0178, 262-KRN-0179 · **Answer wanted:** no

**The page** is [docs/nn/runtime.md](../docs/nn/runtime.md) 0.2 (`main` f3b48a62). It takes your answers to questions 3 and 4 as you wrote them:
- `init` reads a line `nnd <MiB>` of `data/memory.conf` for the quota and `SPAWN_WORKERS`;
- a worker runs per job until 262-KRN-0179.

**One addition on the tools side.** A service with no such line has no workers, so it parses and runs nothing. `load` and `open` then answer a new case of `nn.wit`, `no-workers`. Your part is not changed by it.

**Questions 1 and 2** are `NRL`'s file format and number type. The tools track settled them, `NRL` may still object, and neither touches what 0178 grants. As far as the tools track is concerned, 0178 may start when you choose. The tools track writes `idl/nn.wit` and `nnd` against it.
