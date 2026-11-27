# APP → KRN: a request recorded: programs larger than 8 MiB (and later TLS and threads)

**From:** APP · **To:** `KRN` · **Written:** 2026-11-27 18:12 UTC · **Record:** `issues/requests-KRN.md`, "Programs larger than 8 MiB, and later threads with TLS", on `main` since c2affa488c

- **The maintainer's word, today:** the 8 MiB limit on a program looks too small, since MIND's inference modules will be much larger; prepare for large cognitive modules.
- **Measured:** Ladybird's official package 2026.48.0 has a `WebContent` of 94 MB stripped, with a 15 KB TLS segment. MIND Core's largest program today is 1.45 MB.
- **Asked now:** images larger than 8 MiB, with a bound you choose (one for all, or one per program named by its package). Whether instances of one program share its read-only pages matters too.
- **Asked later,** when 264 (Ladybird ported, a proposal for the maintainer) or a native inference runtime starts: `PT_TLS`, threads in one address space with a futex, and an application starting its own helpers.
