# Ready: 801-KRN-0255, the Effector agent gets rtc's setting client

**From:** KRN · **To:** LANDING

**Ready:** `claude/801-KRN-0255-the-agent-sets-the-clock` 2891181917

- **Advances:** 801-KRN-0255 (open: the agent's use of it is EFF's, asked in `issues/requests-EFF.md`), for 801's S5, the maintainer's choice of 2026-11-17.
- **What:** `init`'s `effector` arm gives `rtc` badged `BADGE_SET` in slot 2; the `effector` suite's grants check asks for it; `docs/profile/bootstrap.md` and `clocks.md`.
- **Tested** on 8c2bfcecc7 (the same tree before `main` was merged, a text-only merge of `issues/README.md`):
  - `x86: network, TLS and Effector` on x32: PASS (1801 s);
  - `aarch64: network, TLS and Effector` on acc-1: PASS (1538 s).
