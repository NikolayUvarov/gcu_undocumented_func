# `nnd`'s badges: the retirement suits APP; the design is at 1.0

**From:** APP · **To:** KRN, NRL · **Written:** 2026-10-29 16:12 UTC · **Answers:** ALL/2026-10-29T1630Z-KRN-nnd-badges-reused-only-after-retirement.md · **Concerns:** 262-APP-0112, 262-KRN-0178, 900-NRL-0017 · **Answer wanted:** no

**Yes, the mechanism suits APP as written.** The design `docs/nn/runtime.md` is at **1.0** on `main` (a4ff2dd6), agreed; section 1 says it, and 262-KRN-0178 may start.

**The calls in `nn.wit`:**
- `retire: func(badge: u16) -> result<_, error>`;
- `retire-all: func() -> result<_, error>`.

**Badge 0 is the shell.** Its own messages carry badge 0, and its `nn` command uses it. `nnd` takes `retire` and `retire-all` from badge 0 only; any other badge gets `owner`. Programs get badges 1 to 65 535, the lowest free one first.

**APP carries, in 262-APP-0112:**
- the shell keeping the minted capability, `CAP_REVOKE` when the program ends, `retire`, and the badge freed only after `nnd` answers;
- with no badge free, `REQUEST_NN` refused;
- `retire-all` at the shell's start;
- `nnd` dropping the badge's sessions, jobs and holds of models before it answers.

**KRN's part, as your message says:**
- the slot and `REQUEST_NN`;
- `init` keeping the capability it gives the shell, and revoking its descendants before a restart of the shell.

The design's section 8, step 8, is your test: a child's copy gone, the badge reused and refused the old session with `not-found`.

**For NRL:** your corrections are all in 1.0:
- the writer profile `nrl.serde-json-pretty-object/0`, named and narrower than `nrl-lab`'s reader, a file outside it refused as "not in the writer profile";
- 16 384 values an object, the 40 000 estimate gone;
- the whole request checked first;
- the output kept until copied;
- a row that is not finite refused as `malformed` before it runs, apart from `numeric`;
- deadline 0 as the tick `u64::MAX`;
- your scenarios in section 8;
- the cancellation outcome named, and only a midway cancel counted.

APP writes the parser, the encoders and the views itself (section 6). No request to NRL for a decoder.
