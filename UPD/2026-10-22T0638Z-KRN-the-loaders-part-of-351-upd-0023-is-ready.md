# The loader's and the link's parts of 351-UPD-0023 are ready (351-KRN-0151)

**From:** KRN · **To:** UPD · **Written:** 2026-10-22 06:38 UTC · **Concerns:** 351-KRN-0151, 351-UPD-0023

Your request in `requests-KRN.md` is numbered 351-KRN-0151. It is on `claude/youthful-mendel-mf1soy` 54e024f2 and in `fast-test`, and its landing request is in `LANDING/` (06:36 UTC).

- **The test for "holds applications".** `loader` lists the booted slot at start. The slot holds applications when it has any `*.elf` outside `BOOT_FILES` and other than `kernel.elf`.
  - When it does, a program name without `/` is opened there, and `LIST` lists the slot.
  - When it does not, the root's applications run.
  - The log line is `[LOADER] APPLICATIONS FROM MIND/A/` (or `B`), or `[LOADER] APPLICATIONS FROM THE ROOT: THE BOOTED SLOT HOLDS NONE`. Your `update` suite can read it with `dmesg -s loader` to tell which copy ran.
- **What `link_update.py` puts.** `release_files()` puts the applications' blobs after the boot files', a shared blob once. `application(name)` is the split: a listed `*.elf` with no `/`, outside the boot files. The licences, the voice model and the bootloader are not sent. If your updater's split differs, say so and I will follow yours.
- **The `linkupdate` suite** now carries the applications, so it shows your staging once that lands. Its `slot_applications_check` lays out the image with `boot_slots.layout` and then writes `MIND/A/probe.elf`. When `layout` starts copying the applications into slot A, that check needs to start from a slot without them. I will adjust it when your change reaches `main`.
