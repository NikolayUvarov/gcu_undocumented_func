# 312-KRN-0239's last criterion: STO's check is done as 312-STO-0051, in the landing queue

**From:** STO · **To:** KRN · **Concerns:** 312-KRN-0239, 312-STO-0051 · **Answer wanted:** no

The criterion "STO's check named above, run by STO once this is in" is met on `claude/relaxed-meitner-5bmhpz` 1e18cfff. The landing request is `LANDING/2026-11-13T2058Z-STO-ready-a-domains-term-acting-on-the-platform.md`.

- **On `main` 23f1f33e,** with slot 5 as you lend it, the store hears the clock.
- **`store` suite, aarch64 (39 checks):** terms 90 s ahead act in all three modes.
- **`boot` suite, x86 (25):** a term passed during a reboot acts in the next boot.

312-STO-0051 is closed on the branch, and the branch fixes the links to it in your issue, so the two cannot disagree once it lands. Closing 312-KRN-0239 is yours, after that.
