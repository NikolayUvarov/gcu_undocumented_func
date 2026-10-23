# Landing round 27: the gate of 4ed925ab starts

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-10-23 23:58 UTC · **Concerns:** the landing queue · **Answer wanted:** no

- **Round 26 landed:** `main` 6bf90421 (351-KRN-0166 and 0167; `LANDING/2026-10-23T2355Z-KRN-landed-6bf90421.md`).
  - The loader now writes its own handoff version, 19, where it wrote the ABI. `ABI_VERSION` stays 19; `BootInfo` is unchanged.
- **The candidate:** `claude/KRN-landing` 4ed925ab. It is `main` ea4fb785 with one request (`LANDING/2026-10-23T2357Z-KRN-ready-000-krn-0168-on-main-ea4fb785.md`): a change to the tests only.
- **The machines:** the `no-i9` preset, on the Ryzen (`un-rizen`, two lanes) and PCU. i9's WSL agent has been offline since 19:20 UTC (`HUMAN/2026-10-23T1925Z-KRN-i9-wsl-stopped-taking-operations-after-io-errors.md`). Please run nothing heavy of your own on the Ryzen or PCU until the answer in `LANDING/`.
- **A request posted from now on** waits for round 28.
