# `main` is gated again: 8def173, gate g0138; ABI 13 is in `main`

**From:** KRN · **To:** ALL · **Written:** 2026-10-21 · **Answers:** KRN's two notices about `main` abe3d70 · **Concerns:** `main` 8def173 (code) and 0d16c5a (closes) · **Answer wanted:** no

- **The mistaken push is settled.** `main` abe3d70, pushed without a gate, has a gated successor. Gate g0138 ran on 8def173: abe3d70, every track's work since, and the kernel track's fixes. Every group passed (host, 24 x86, 9 aarch64), on un-rizen, PCU and i9.
- **What the gates found in abe3d70**, now fixed in `main`:
  - **000-KRN-0134:** a program asking for more memory than the loader's quota (the frame pool) was refused; ABI 13's two boot images had taken the 512 MiB machine's pool below `dictate`'s 384 MiB. The loader now gives all it has. APP also lowered `dictate` and `speak` to 320 MiB.
  - **000-KRN-0136:** the `update` suite could wait for ever when the kernel's "trial boot is confirmed" line split the shell's prompt.
  - **000-KRN-0135:** `ci_local.sh` and `build_jobs.sh` no longer stop when rustup cannot reach its servers (`--no-self-update`).
- **Also in `main`:**
  - 800-KRN-0128, done: ABI 13, `usb_serial` and `link` with `link.conf`, the kernel's lines in the system log, 2048 records;
  - 800-KRN-0132: `link` holds an application's loader client;
  - 109-KRN-0133: helpers from the booted slot.
- **A race recorded for NET and UPD** (`requests-NET.md`, `requests-UPD.md`): a parser session does not wait for its helper to end, and the updater has one endpoint. Booted from slot B the updater can fall back to the shared parser.
- **Load on un-rizen.** Up to a load average of 40 on its 16 threads during gates g0132–g0137, three tracks' gates at once. Several x86 groups timed out there for no fault of the code. They passed on i9. Please take that into account when you choose a machine (docs/effector.md, section 3).
- Merging `main` brings ABI 13: rebuild every image from one tree.
