# Requests for the phones and mobile devices track (MOB), not numbered yet

**Owner:** `MOB` (the assessing session) · **Status:** every request numbered (2026-11-28); the phone's own parts of request 2 wait for the phone · **Recorded by:** the kernel session (`UPD`, `KRN`)

The `MOB` track numbers its own tasks (`NNN-MOB-MMMM`), so requests from other tracks wait here. `MOB` turns each into a task and removes it from this file. The file is removed when it is empty.

## What the requests became (2026-11-28)

- **Request 2, for the bench's board,** as the maintainer's word below has it:
  - item 1, the card: [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md), and the board's word in `build_and_write.sh`, [100004-MOB-0011](100004-MOB-0011-the-boards-word-in-build-and-write.md);
  - items 2 and 3, the first boot step by step and its run script: [100004-MOB-0012](100004-MOB-0012-the-boards-first-boot-step-by-step-and-its-run-script.md);
  - item 4, the profile's skeleton: [100004-MOB-0013](100004-MOB-0013-the-boards-profiles-as-skeletons.md).
- **Request 2, the phone's own parts** (Tow-Boot's place on the PinePhone, the phone's card, its page and profile) wait for the phone at hand ([issues-human, section 12](../issues-human/README.md#12-a-pinephone-to-test-on)). They get numbers under [100000](100000-a-phone-the-pinephone-first.md) then.
- **Request 3:** the card is 100004-MOB-0001, and the two first boots are [100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md) and [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md). Each waits for the bench to be wired (issues-human, section 14).

## The maintainer's word of 2026-11-26 on requests 2 and 3

«MOB будет делать работу на стенде - это не телефон, а плата разработки с добавленными компонентами»: `MOB` works on the bench, a development board with components added to it, not on the phone. So:
- **request 3 comes first,** on that board; the board and its components are to be named in [issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench);
- **request 2's items 1 to 3** are made for the bench's board: its card, its first boot in `docs/phones/` (or the board's own page), its run script through Effector;
  - what is the phone's alone (where Tow-Boot lives, the phone's card) waits for the phone;
- **request 2's item 4** becomes the board's profile, with the components it carries; the phone's profile waits for the phone.

## 2. Everything ready for the PinePhone before it is at hand (for 100000)

**Why.** The maintainer (2026-11-25): «подготовить все для деплоя на аппаратный телефон pinephone». The phone is not at hand yet ([issues-human, section 12](../issues-human/README.md#12-a-pinephone-to-test-on)). The goal is that its first boot needs only a card written and the cable plugged in.

**Asked, in this order:**
1. **The PinePhone's entry in the catalog of devices now** (`scripts/boards.toml`, [352-MOB-0010](../issues-done/352-MOB-0010-a-catalog-of-devices-for-board-image.done)), not when the phone is at hand, and its card from `board_image.py --board pinephone`, with a host test of the layout.
   - Where Tow-Boot lives on the original PinePhone (the eMMC's boot partition or SPI flash: 100000 and 352-UPD-0049 say different things), from Tow-Boot's documentation.
   - Whether the A64's boot ROM, finding no loader at 8 KiB on our card, goes on to Tow-Boot, and so what our card must leave free.
2. **`docs/phones/pinephone.md`:** the first boot as the maintainer does it (Tow-Boot's installer, the card, the cable, the Effector host and its port), and as an agent does it (the console read through Effector, 100000-EFF-0026), with stage 1's criteria as the lines that show them.
3. **A run script** that reads the console through Effector and checks stage 1's criteria from it: the kernel's lines, four CPUs, the timer, the memory map, the shell's prompt, a reboot. It is tried first on QEMU `virt` under U-Boot's UEFI with four Cortex-A53, as evidence for QEMU only (MC-12.1).
4. **The profile's skeleton,** `docs/profile/aarch64-pinephone/` (with `KRN`): the TCB and the limits from 100000's stage 6, each line "not tested" until a run (MC-12.3).

**Not asked:** buying or wiring anything (the maintainer's, issues-human sections 12 and 14).

## 3. Stage 1 on the boards first (for 100004)

**Why.** The same priority. The boards share the A64's addresses and need the same steps; their first boots (100004-MOB-0002 and 0003) are the rehearsal for the phone's.

**Asked:** 100004-MOB-0001's card finished and the two first boots run as soon as the maintainer has wired the bench ([issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)); then the kernel's runs of 100000-PRT-0010, 100000-PRT-0012 (on the H5, if its tree names the erratum), 100004-PRT-0014 and 100004-PRT-0015 on them.

