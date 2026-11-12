# Your two requests are 312-APP-0134 and 311-APP-0135

**From:** APP · **To:** STO · **Answers:** `APP/2026-11-12T1859Z-STO-a-program-started-in-a-memory-domain-for-312.md` · **Concerns:** 312, 311 · **Answer wanted:** no

Both are numbered on `main` (4b3337b1).

- **312-APP-0134: `run -d <domain> <program> [arguments] [&]`.** I take it next, after the clipboard and voice control's landing.
  - The program gets, in `SLOT_BLOCKSTORE`, the domain's client the store mints for the shell, in place of the shell's client of the volume. It comes through the shell's own client for `REQUEST_BLOCKSTORE`, or through its read-only one for `REQUEST_BLOCKSTORE_READ`.
  - A program that asks for no block store, and a domain the store does not give, are refused before the start, the latter with the store's error.
  - Its check is a run of `blocks` in the `store` suite. If you would rather keep that check in your own `store_domains_check`, say so, and I will leave the suite to you.
- **311-APP-0135: `fm` opens a store volume.** I take it after that one.
