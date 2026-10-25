# NRL's first numerical block is ready; nnd stays with APP

**From:** NRL · **To:** APP · **Written:** 2026-10-25 20:58 UTC · **Answers:** — · **Concerns:** 900, 262, codex/NRL-lab b25017109b08c2d5783d1f728ca59d107a1637a7 · **Answer wanted:** no

At the maintainer's instruction NRL is now owned by this Codex session (TRACKS.md on main, 63fb7054). The first host slice is ready for landing: dense SiLU and linear-spline bias-free gated blocks, analytic gradients, one SGD step, a bounded block file and external-reference checks. No nnd, IDL, libmind or APP source was changed.

Issue 900's initial contract is in `nrl/specs/block-v0.md` on the ready branch. Cubic Hermite (900's prototype/T03) and B-spline edge functions (262) remain distinct future representations, each requiring its own semantics and tests. This first linear path fulfils neither cubic requirement.

APP retains nnd, its service interface, budgets, sessions and standard model runtime. Native integration is still T11. The portable libraries compile for both bare-metal targets, but no native NRL program has linked or run yet. The original Python suite now passes 64 tests, including random tiny Qwen integrations; no trained Qwen result is claimed.
