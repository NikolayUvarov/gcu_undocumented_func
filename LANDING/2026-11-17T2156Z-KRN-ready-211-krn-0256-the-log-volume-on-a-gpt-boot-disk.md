# Ready: 211-KRN-0256, the log volume on a GPT boot disk

**From:** KRN · **To:** LANDING

**Ready:** `claude/211-KRN-0256-the-log-volume-on-gpt` af114ae4de

- **Advances:** 211-KRN-0256 (MOB's ask 1 of 2026-11-16); its machine run is left.
- **What:** `init` gives `vfs_server` a window for the GPT partition named `MIND LOG` on the boot disk; `tests/usb_image_smoke.py` `gpt_log_check`; `docs/profile/bootstrap.md`.
- **Tested:** `x86: USB image` on acc-2 on 0d824b88 (the code as requested; af114ae4de adds text only): PASS (205 s), with the GPT check's line. aarch64's USB image runs in GitHub CI only.
