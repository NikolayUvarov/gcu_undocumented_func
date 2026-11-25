# Requests for the phones and mobile devices track (MOB), not numbered yet

**Owner:** `MOB` (the assessing session) · **Status:** open (three requests: 2026-11-20 and two of 2026-11-25) · **Recorded by:** the kernel session (`UPD`), for [352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md)

The `MOB` track numbers its own tasks (`NNN-MOB-MMMM`), so requests from other tracks wait here. `MOB` turns each into a task and removes it from this file. The file is removed when it is empty.

## 1. A catalog of devices for `board_image.py` (for 352-UPD-0049)

**Why.** The maintainer (2026-11-20) asked for an image or a package that installs MIND Core on ARM devices such as the Orange Pi boards and phones. [352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md) builds each device's image from one release. It needs one place that says what each device takes. Today the two boards' details live in `scripts/board_uboot.sh` and `scripts/board_image.py` ([100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md)), and the PinePhone's in [100000](100000-a-phone-the-pinephone-first.md).

**Asked:**
1. **One entry per board or phone,** in a file that `board_image.py --board <name>` reads:
   - the SoC;
   - the firmware: the script that fetches or builds it, its source and version, and its SHA-256;
   - where the firmware goes: on the card (U-Boot with TF-A at 8 KiB) or on the device's own flash (Tow-Boot on the PinePhone's SPI);
   - the boot partition's start, and the device tree U-Boot hands over;
   - where the firmware goes for a boot from the eMMC, for 352-UPD-0048's installer.
2. **`board_image.py` takes a boot volume given to it** (a FAT image from a release), not only a local build. `UPD` adds the check against the release's signed manifest, in 352-UPD-0049.
3. **The entries:** the Orange Pi Zero Plus 2 and Zero 3 first; the PinePhone when the phone is at hand.

**Not asked:** any change to U-Boot's or Tow-Boot's builds, or to the boards' runs of 100004-MOB-0002 and 0003.

## 2. Everything ready for the PinePhone before it is at hand (for 100000)

**Why.** The maintainer (2026-11-25): «подготовить все для деплоя на аппаратный телефон pinephone». The phone is not at hand yet ([issues-human, section 12](../issues-human/README.md#12-a-pinephone-to-test-on)). The goal is that its first boot needs only a card written and the cable plugged in.

**Asked, in this order:**
1. **The PinePhone's entry in request 1's catalog now,** not when the phone is at hand, and its card from `board_image.py --board pinephone`, with a host test of the layout.
   - Where Tow-Boot lives on the original PinePhone (the eMMC's boot partition or SPI flash: 100000 and request 1 say different things), from Tow-Boot's documentation.
   - Whether the A64's boot ROM, finding no loader at 8 KiB on our card, goes on to Tow-Boot, and so what our card must leave free.
2. **`docs/phones/pinephone.md`:** the first boot as the maintainer does it (Tow-Boot's installer, the card, the cable, the Effector host and its port), and as an agent does it (the console read through Effector, 100000-EFF-0026), with stage 1's criteria as the lines that show them.
3. **A run script** that reads the console through Effector and checks stage 1's criteria from it: the kernel's lines, four CPUs, the timer, the memory map, the shell's prompt, a reboot. It is tried first on QEMU `virt` under U-Boot's UEFI with four Cortex-A53, as evidence for QEMU only (MC-12.1).
4. **The profile's skeleton,** `docs/profile/aarch64-pinephone/` (with `KRN`): the TCB and the limits from 100000's stage 6, each line "not tested" until a run (MC-12.3).

**Not asked:** buying or wiring anything (the maintainer's, issues-human sections 12 and 14).

## 3. Stage 1 on the boards first (for 100004)

**Why.** The same priority. The boards share the A64's addresses and need the same steps; their first boots (100004-MOB-0002 and 0003) are the rehearsal for the phone's.

**Asked:** 100004-MOB-0001's card finished and the two first boots run as soon as the maintainer has wired the bench ([issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)); then the kernel's runs of 100000-PRT-0010, 100000-PRT-0012 (on the H5, if its tree names the erratum), 100004-PRT-0014 and 100004-PRT-0015 on them.

