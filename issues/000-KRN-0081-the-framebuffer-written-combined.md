# 000-KRN-0081 — The framebuffer written combined: PAT write-combining, and the compositor's copy by rows

**Type:** kernel (`PRT` lines in `kernel/src/arch/x86_64/`, the generic mapping, `compositor`) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made; the tests and the MacBook Pro's run left) · **Blocked by:** — · **Roadmap:** track H (main task [174](174-full-use-of-pc-hardware.md), step 6) · **Constitution:** MC-12.3

## Problem

The maintainer (2026-10-11): `wm`'s moving background stutters, more so with a bright one, and redrawing is slow on the MacBook Pro (2880×1800, stride 4096, framebuffer at 0x90020000 in the discrete GPU's BAR).

The analysis, from the code:

- **The framebuffer was mapped as plain memory.** `compositor` maps `PLATFORM_FRAMEBUFFER` through `MEM_MAP` as ordinary memory, and the kernel never set the PAT. Device memory is therefore cached as the firmware's MTRRs say, which for a GPU's BAR is almost always uncached. Every pixel the compositor writes is then a separate bus transaction, roughly 50–200 MB/s. Issue 174 already said "Graphics memory is mapped uncached, not write-combining".
- **The compositor scans too much.** It compared `stride × height` pixels with its shadow copy, one volatile 4-byte read at a time. On the Mac that is 4096 × 1800, of which 2880 columns are on screen: 42 % of the work was off screen.
- **A brighter or faster background changes more pixels a frame,** so more slow writes. That is why a bright one stutters more.
- **`wm`'s own share** is the tools track's (see `requests-APP.md`): it redraws the whole background a pixel at a time with a division per pixel, computes the pattern at full size, and sleeps a fixed 30 ms after the work.

## Plan

- **PAT (every CPU, `mmu::enable_protection`).** The power-on default, but entry 1 (PWT alone) is write-combining. No page used PWT alone before.
- **Mapping.** `paging::Caching` (memory, device, write-combining) replaces the device flag of the user mapping. `MEM_MAP` maps a memory capability that lies inside the boot framebuffer write-combining (`mmu::write_combining`: PWT on x86). On aarch64 it stays normal memory.
- **The hardware report.** Under "Frequencies and power" it lists the MTRRs (the default type, the variable ranges) and the PAT. On the Mac it shows what type the firmware gave the framebuffer.
- **`compositor`.** It compares and copies only the visible width of each row, pixel by pixel as before. A first version compared groups of 8 pixels as slices; in this `no_std` build that is a byte-wise `memcmp`, which made each frame several times dearer. On PCU's QEMU (aarch64, TCG) the compositor then held its CPU, and `kbench`'s sleeps of 10 ms took 50 ms. With the old compositor they took 19.7 ms, with this one as before.
- **Not changed.** The kernel's identity map still maps the framebuffer as before: as write-back pages, so as the MTRRs say. The kernel writes it only before the compositor takes the screen, and for a fatal screen.
- **Later (a KRN task).** Damage regions or an explicit "present" from the screen's owner, an ABI change, so the compositor stops copying half-drawn frames and scanning unchanged ones.

## Acceptance criteria

- **QEMU.** The display, `wm` and screenshot suites pass on x86 and aarch64. QEMU's TCG ignores the PAT, so speed is not measured there.
- **The MacBook Pro.**
  - The hardware report lists the MTRRs and `PAT 0007040600070106`.
  - The moving background redraws visibly faster than with 0b890f6.
  - With the tools track's part, it no longer stutters at full contrast.

## Progress

**2026-10-11, MacBook Pro** (`fast-test` efe17aa, `hw0001.txt`):
- **The PAT.** It reads `0007040600070106`, so entry 1 is write-combining.
- **The framebuffer's range.** The framebuffer at 0x90020000 lies in variable MTRR 2 (0x90000000, 256 MiB, UC). With an MTRR of UC and a PAT of WC, the CPU uses WC (Intel SDM, vol. 3A, 12.5.2.2), so the compositor's writes are now combined.
- **Left.** Whether the moving background runs evenly is the maintainer's to judge, together with 000-KRN-0083's tick.

**2026-10-12, MacBook Pro** (`fast-test` c548a85, with [000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done)): the maintainer reported "the cursor stopped flickering, the picture is smooth". Left:

- the tools track's part (`requests-APP.md`: the background drawn by rows, paced to a deadline);
- the maintainer's look at the moving background at full contrast.

## Progress (2026-10-20)

- **The report checked.** `usb_image_smoke.py`, x86, now requires the report's `MTRR … variable ranges` line and `PAT 0007040600070106`.
- **On real machines.** The MacBook Pro's and the Ryzen's reports show that PAT on every run since 2026-10-11.
- **aarch64.** The display and `wm` suites still run on x86 only; on aarch64, `screenshot` and the shell pass ("aarch64: files, network and TLS").

Left: the stutter at full contrast, which needs the tools track's `wm` change (`requests-APP.md`), and the aarch64 part.

## Related

[174](174-full-use-of-pc-hardware.md), [211-KRN-0018](../issues-done/211-KRN-0018-the-compositors-quota-fits-the-screen.done).
