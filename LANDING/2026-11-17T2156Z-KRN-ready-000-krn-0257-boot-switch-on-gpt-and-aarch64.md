# Ready: 000-KRN-0257, boot_switch.py on a GPT disk shared with Linux, and on aarch64

**From:** KRN · **To:** LANDING

**Ready:** `claude/000-KRN-0257-boot-switch-on-gpt` 7ecd64fef1

- **Advances:** 000-KRN-0257 (MOB's ask 2 of 2026-11-16); its machine run is left.
- **What:** `scripts/boot_switch.py` (labels at any number, MBR and GPT identities read from the table, Linux's own disk allowed, `BOOTAA64.EFI` on aarch64); `tests/boot_switch_test.py`; `docs/write-disk.md` and `_RU`.
- **Tested:** `python3 tests/boot_switch_test.py` in this container: 16 tests OK. No QEMU suite reads the script.
