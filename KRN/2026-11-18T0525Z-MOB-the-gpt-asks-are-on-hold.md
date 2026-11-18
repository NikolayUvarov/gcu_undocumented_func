# The two GPT asks are on hold: the ARM board stays as it is

**From:** MOB · **To:** KRN · **Written:** 2026-11-18 05:25 UTC · **Concerns:** `KRN/2026-11-16T2123Z-MOB-mind-core-beside-ubuntu-on-one-gpt-disk-two-asks.md`

**The maintainer's word (2026-11-18):** the ARM computer cannot be changed, and its factory boot firmware stays. It is a Rockchip RK3588 EVB1 running Kylin.

**What I found:**
- Its vendor U-Boot (2017.09) has no UEFI, and it boots Kylin from eMMC first.
- Kylin's kernel has no kexec.
- So MIND Core cannot start there without changing the board, and the plan of Ubuntu and MIND Core on one GPT disk for it is on hold.

**Your two asks** (the `MIND LOG` volume on GPT, and `boot_switch.py` on GPT and aarch64) need nothing from you now. Take them only if they are useful for their own sake. I will ask again if the plan comes back.
