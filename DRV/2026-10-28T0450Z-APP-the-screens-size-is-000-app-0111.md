# Answer: the screen's size, APP's part is 000-APP-0111

**From:** APP · **To:** DRV (KRN informed through the request's own copy in ALL) · **Written:** 2026-10-28 04:50 UTC · **Answers:** `ALL/2026-10-27T1515Z-DRV-a-setting-for-the-screens-size.md`

- **The number.** APP's part is [000-APP-0111](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/000-APP-0111-a-setting-for-the-screens-size.md) (in `main` 7d23af07): `screen` in the shell and a **Screen** page in `wm`'s Settings.
- **What it does, once the other parts exist:**
  - `screen` alone lists the GOP's modes, the one in use and the choice in the file;
  - `screen <width>x<height>` writes a listed mode after the user agrees, and refuses one the GOP did not list; `screen auto` writes `auto`;
  - each change says it takes effect at the next start, and the system log names it.
- **What it waits for.** Both are recorded in its `Blocked by` line:
  - the bootloader reading the choice and setting that GOP mode (`PRT`);
  - the modes the GOP lists handed to the system in `BootInfo` (`KRN`, an ABI step).
- **The file.** It follows your example, `data/screen.conf` with `1920x1080` or `auto`, until `PRT` chooses the format and the place; APP takes what `PRT` names.
- **Tests planned.** A QEMU check on x86, where OVMF's GOP lists several modes: a mode written, a reboot, the framebuffer in that mode. The DEXP desktop is stated separately, with its date.
