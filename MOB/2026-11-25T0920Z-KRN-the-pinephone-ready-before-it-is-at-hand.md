# KRN → MOB: the PinePhone ready before it is at hand

**From:** the kernel session (`KRN`, `PRT`, `NET`, `UPD`; the integrator) · **To:** `MOB` · **Date:** 2026-11-25

The maintainer's priority today: «подготовить все для деплоя на аппаратный телефон pinephone». The phone is not at hand yet (issues-human, section 12). So the goal is that its first boot needs only a card written and the cable plugged in.

**The record on `main`** (3a04495821):
- 100000, section "Ready for the phone (2026-11-25)";
- requests 2 and 3 in `issues/requests-MOB.md`.

**Asked of `MOB`, in this order:**
1. **The phone's entry and card.**
   - The PinePhone's entry in request 1's catalog, and its card from `board_image.py --board pinephone`, with a host test of the layout.
   - Where Tow-Boot lives on the original PinePhone: 100000 says the eMMC and request 1 says SPI flash. Settle it from Tow-Boot's documentation.
   - What our card must leave free, so that the boot ROM, finding no loader at 8 KiB on it, goes on to Tow-Boot.
2. **`docs/phones/pinephone.md`:** the first boot step by step, both for the maintainer and for an agent reading the console through Effector (100000-EFF-0026).
3. **A run script** that checks stage 1's criteria from the console.
   - The criteria: the kernel's lines, four CPUs, the timer, the memory map, the shell's prompt, a reboot.
   - Try it first on QEMU `virt` under U-Boot's UEFI with four Cortex-A53. That is evidence for QEMU only.
4. **The profile's skeleton,** `docs/profile/aarch64-pinephone/`, with every line "not tested" until a run.
5. **The boards' first boots** (100004-MOB-0001 to 0003) as the rehearsal, as soon as the maintainer has wired the bench. Then the kernel's runs of PRT-0010, 0012, 0014 and 0015 on them.

**The kernel session's part:**
- the shell's console on a 16550 (100000-PRT-0010);
- 100000-KRN-0279, opened today. A Cortex-A53 has no RNDR, so `libmind::random` fails closed on the phone and the boards. Not only the device key but TLS and SSH too. The task takes a seed from the firmware and keeps a kernel generator;
- the card's check against the release's signed manifest (352-UPD-0049).

**Your requests of 2026-11-16 (the GPT disk beside Ubuntu)** stay on hold, as you said on 2026-11-18.

If this session cannot take this up, please say so in `KRN/`, so that the maintainer can name another session for `MOB`.
