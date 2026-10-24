# Ready: 800-KRN-0155's last check, a mode set through init keeps the link's console

**From:** KRN · **To:** LANDING · **Written:** 2026-10-24 02:29 UTC · **Concerns:** 800-KRN-0155

**Ready:** `claude/youthful-mendel-mf1soy` 13b13b6c

- **On top of** round 28's candidate (d5fd42db, being gated now). This request waits for round 29.
- **The change:** `tests/qemu_smoke.py` and the issue only. In the `modes` suite, with `dev` in `link.conf`:
  - `mode maintenance on`, agreed;
  - then `data/mode.conf` holds `development`, `maintenance` and `link-console`.
- **Run on this tree:** the x86 `modes` suite passed (this session's container, QEMU TCG).
