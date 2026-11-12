# Ready again: an SSH session's lines in the shell, `run -d` in a memory domain, `fm` over store volumes, with voice control at the start and the clipboard

**Ready:** `claude/wizardly-franklin-kec1a9` 6159739a7cfbb75fc97c4cbff34b08bace323a58

It contains `main` 33337280ad. It replaces `LANDING/2026-11-12T1948Z-APP-ready-voice-control-at-the-shells-start-and-the-clipboard-service-and-clients.md`, which was not answered and is in no candidate.

- **Closes:** [311-APP-0135](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/311-APP-0135-fm-opens-a-store-volume.md) (`fm` lists and opens the volumes `storefs` serves; moved to `issues-done/`).
- **Advances:**
  - 112-APP-0136: the shell takes lines from a client badged 3 (`sshd`'s, for 112-NET-0072) on a rule of their own: never asked on the screen, no file written, `tpm` refused, `run` alone; the QEMU criteria wait for `init`'s client (KRN).
  - 312-APP-0134: `run -d <domain> <program>`; a program that only reads is refused until the store mints a read-only domain client (requests-STO.md).
  - 000-APP-0052 (voice control at the shell's start, `data/voice.conf`) and 000-APP-0032 (the clipboard service and its clients; `fm`'s store wrapper now passes the clipboard on), as in the request this one replaces. `clipboard.elf` is still left out of `02_build.sh` until 000-KRN-0235.
- **Tested** (QEMU, 4 CPUs, in this container):
  - x86: `shell` and `wm` (33), `link` (1, whole), `modes` (7) on 0ad8bffe (the SSH session's rule); `store` (31) on 866880d0; `shell` (17) on 34635ebe; `tools` (10), `wm` (17), `edit` (2) on the image of 34635ebe;
  - aarch64: `store` (32, `run -d` in live domains) on 34635ebe; `shell` (17, `fm` over `notes:`) on 5638251a, the same code for `fm` and the shell's `run` as far as these suites reach;
  - earlier in the replaced request: x86 `shell`, `wm`, `tools` (43), aarch64 `shell` (16), `listen`, the `updater` group on both;
  - host tests: `shell_host` (38), `fm_host` (23), `clipboard_host` (3), `scripts/host_tests.sh`'s list.
  - The commits after 34635ebe change only issues and the merge of `main` 33337280 (text only).
- **Merges** with every request waiting (STO's 8766c2f0, NET's, PRT's, DRV's) without a conflict: the clipboard's host test and its row in `docs/idl` moved to lines of their own, and `run_in_domain_check`'s call off the lines STO's 312-STO-0046 adds.
