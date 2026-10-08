# MIND Core on Apple Silicon Macs

**Version:** 1.0 (2026-10-08) · **Track:** `APL`, Apple Silicon ([TRACKS.md](../TRACKS.md)) · **Issues:** [600](../issues/600-apple-silicon-mac-vm-host.md) (a virtual machine on a Mac), [210](../issues/210-apple-silicon-native.md) (natively) · **Roadmap:** [track H](../ROADMAP.md) · **Constitution:** [v1.6](../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-12.1, MC-12.3, MC-12.9 · **Russian version:** [apple-silicon_RU.md](apple-silicon_RU.md) (kept in sync; the English text is the reference)

> **Nothing in this guide has been run on a Mac yet.** CI and every local run of this repository are on Linux. The steps follow from what the scripts do and from how Homebrew and QEMU are expected to behave on macOS. Each step says what is known and what is **not yet tested on a Mac**. Whoever runs them first, please report the result: task [600-APL-0011](../issues/600-APL-0011-first-run-on-a-mac.md), and [issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on).

## At a glance

| Way to run | Status |
|---|---|
| The aarch64 build in QEMU with Apple's hypervisor (HVF) | `03_run_qemu_aarch64.sh` chooses HVF on an Apple Silicon Mac. **Not yet tested on a Mac.** The build needs Homebrew's Bash and GNU sed there (section 2). |
| The aarch64 build in QEMU under emulation (TCG) | The configuration CI tests, on Linux (`-cpu max`). On a Mac the test suites run this way (section 5); **not yet tested on a Mac**. |
| Natively, without a virtual machine | **Not supported yet** (section 7); issue [210](../issues/210-apple-silicon-native.md), track `APL`. |
| The x86-64 build | On a Mac, QEMU can only emulate an x86 machine; `03_run_qemu.sh` is written for Linux. Not covered by this guide. |

What the profile [`aarch64/QEMU-virt-0`](profile/aarch64/README.md) states was tested with QEMU `virt` under TCG on Linux, with `-cpu max`. It does not carry over to a Mac with HVF and `-cpu host` (MC-12.1, MC-12.9): that is another configuration, and nothing has been tested in it (section 4).

## 1. What you need

- A Mac with Apple Silicon (M1 or later) and [Homebrew](https://brew.sh), which lives in `/opt/homebrew` on these Macs. Homebrew installs the Xcode Command Line Tools; the build links its host-side helpers (build scripts and procedural macros of the crates) with them.
- From Homebrew:

  ```bash
  brew install qemu bash gnu-sed
  ```

  - `qemu`: `qemu-system-aarch64`, `qemu-img` and the EDK2 UEFI firmware for aarch64 (`share/qemu/edk2-aarch64-code.fd` and `share/qemu/edk2-arm-vars.fd` under `$(brew --prefix)`).
  - `bash`: the aarch64 build script needs Bash 4.4 or later; macOS's `/bin/bash` is 3.2.
  - `gnu-sed`: the aarch64 build script uses a `sed` form that macOS's `sed` is expected to reject.
- Rust through rustup, with the pinned nightly and the targets of `rust-toolchain.toml` (among them `aarch64-unknown-none-softfloat` and `aarch64-unknown-uefi`):

  ```bash
  ./01_prepare_env.sh
  source "$HOME/.cargo/env"
  ```

  These targets are bare-metal and UEFI ones, so rustup is expected to provide them on a macOS host as on Linux, and the programs link with the `rust-lld` that comes with the toolchain, not with a system linker. If Homebrew's `rust` is installed too, `command -v cargo` must print `~/.cargo/bin/cargo`. **Not yet tested on a Mac.**
- Python 3.9 or later for the USB image and the tests (macOS's own `python3` or Homebrew's).

## 2. Build on the Mac

```bash
PATH="$(brew --prefix gnu-sed)/libexec/gnubin:$PATH" "$(brew --prefix)/bin/bash" scripts/build_aarch64.sh
```

This is what `ARCH=aarch64 ./02_build.sh` runs on Linux. It builds the bootloader (`aarch64_root/EFI/BOOT/BOOTAA64.EFI`), the kernel and every program for aarch64 into `aarch64_root/`. Add `--fixtures` for the test fixtures (section 5). **Not yet tested on a Mac.**

Why not `ARCH=aarch64 ./02_build.sh` as on Linux:

- `02_build.sh` starts `scripts/build_aarch64.sh` through its first line, `#!/bin/bash`: on macOS that is Bash 3.2, which has no `mapfile`, so the script is expected to stop at once (`mapfile: command not found`).
- With a newer Bash but macOS's `sed`, the script is expected to read no programs from `02_build.sh` and stop with `USER_CRATES not found in 02_build.sh`.

The command above runs the script with Homebrew's Bash and puts GNU sed first in `PATH`. Making the build work with what macOS ships is task [600-APL-0009](../issues/600-APL-0009-aarch64-build-on-macos.md).

## 3. Run in a virtual machine with the hypervisor

```bash
./03_run_qemu_aarch64.sh -display cocoa
```

**Not yet tested on a Mac.** What the script does on an Apple Silicon Mac (it checks that `uname -s` is `Darwin` and `uname -m` is `arm64`):

| | On an Apple Silicon Mac | On Linux |
|---|---|---|
| Accelerator and CPU | `-accel hvf -cpu host`: the guest runs on the Mac's own cores | `-cpu max`, emulated (TCG) |
| Machine | `virt,gic-version=3,highmem=off`, 512 MiB, 4 CPUs | the same |
| Firmware | `edk2-aarch64-code.fd` and a private copy of `edk2-arm-vars.fd` from `share/qemu` next to the QEMU it runs, `$(brew --prefix)/share/qemu` for Homebrew's | AAVMF (Debian, Ubuntu), edk2 (Fedora) or QEMU's own |
| Disk | `aarch64_root/` as a virtual FAT disk | the same |
| Devices | `ramfb` screen, VirtIO network card (QEMU's user networking), VirtIO keyboard and tablet | the same |
| Console | the PL011 on this terminal (`-serial mon:stdio`); **Ctrl+A X** leaves QEMU | the same |

- **The window.** The script opens a window only where `DISPLAY` or `WAYLAND_DISPLAY` is set (a Linux desktop, WSLg). On macOS neither is normally set, so it passes `-display none`, and only the console on the terminal remains. `-display cocoa` after it asks QEMU for its macOS window: QEMU is expected to take the last `-display` it is given. In the window, keys go to the VirtIO keyboard and the pointer to the VirtIO tablet, so QEMU need not grab the pointer. Task [600-APL-0010](../issues/600-APL-0010-run-script-on-macos.md) makes the script open the window on a Mac by itself.
- **A native terminal.** Run the script from a terminal that runs natively (arm64). Under Rosetta, `uname -m` prints `x86_64` and the script falls back to emulation (TCG).
- **Bash.** The script's first line is `#!/usr/bin/env bash`: it runs the first `bash` in `PATH`. That is Homebrew's when Homebrew's directories come first, as Homebrew's shell set-up (`brew shellenv`) puts them. Under macOS's Bash 3.2, `MIND_NET=none`, or a `DISPLAY` that XQuartz sets, is expected to stop the script with `unbound variable` (an empty array under `set -u`). Task 600-APL-0010.
- **Firmware.** If the script says `No aarch64 UEFI firmware`, name the files:

  ```bash
  MIND_AAVMF_CODE="$(brew --prefix)/share/qemu/edk2-aarch64-code.fd" \
  MIND_AAVMF_VARS="$(brew --prefix)/share/qemu/edk2-arm-vars.fd" ./03_run_qemu_aarch64.sh -display cocoa
  ```

- **CPUs.** `MIND_CPUS=n` (default 4). Each virtual CPU is a thread of QEMU that macOS runs on any of the Mac's cores; an M1 has 8, four performance and four efficiency cores. Keep `n` at or below `sysctl -n hw.ncpu`. The kernel starts every CPU the firmware's ACPI tables list; up to 16 are tested, under TCG on Linux, none under HVF.
- **Memory.** `MIND_MEMORY=size` (default `512M`). Up to `3G` the machine keeps RAM and PCI below 4 GiB (`highmem=off`, the layout the suites test). Above `3G` the script sets `highmem=on`. Under HVF an M1 is expected to give the guest a 36-bit physical address space (64 GiB); whether QEMU fits its layout into it is not tested, so stay at `3G` or below.
- **Other settings** are those on Linux: `MIND_NET=none` leaves the network card out, `MIND_DISPLAY=none` the window, `QEMU=<binary>` names another QEMU; further arguments go to QEMU.

### What a working boot shows

On the terminal: the firmware's lines, then the kernel's, among them `MIND CORE KERNEL: BOARD GICV3 … ITS=… … CPUS n` with the layout it read from ACPI, the services' lines, `[INIT] READY` and the `MIND>` prompt. In the window: the system's screen. `reboot` restarts the machine, and `reboot --off` turns it off (PSCI). Please report what you see in [600-APL-0011](../issues/600-APL-0011-first-run-on-a-mac.md), with:

- the `MIND CORE KERNEL: BOARD …` line (whether QEMU gave an ITS);
- `[KEYSTORE] DEVICE KEY READY …` or `[KEYSTORE] NO RNDR: NO DEVICE KEY` (section 4);
- the Mac's model and chip, the output of `sw_vers` and of `qemu-system-aarch64 --version`.

### The USB image in the virtual machine

```bash
./04_make_usb_image_aarch64.sh --no-build    # after the build of section 2
./03_run_qemu_aarch64.sh --image -display cocoa
```

`04_make_usb_image_aarch64.sh` builds first by running `02_build.sh`, which stops on macOS as section 2 says; hence the build of section 2 and `--no-build`. It packs `aarch64_root/` into `dist/mind-core-usb-aarch64.img` with Python and `qemu-img` (part of Homebrew's `qemu`). `--image` boots that file as a USB stick on xHCI; QEMU writes nothing to it. **Not yet tested on a Mac.** The image is meant for boards with UEFI firmware (issue [205](../issues/205-aarch64-boards.md)); `05_write_usb_linux.sh`, which writes it to a stick, runs only on Linux. A Mac cannot boot it (section 7).

## 4. What differs from the tested configuration

- **The processor (`-cpu host`).** The guest sees the features of the Mac's cores instead of QEMU's `max` model. The key service makes the device key from RNDR, the random number instruction; whether Apple's cores have it has not been checked here. Without it the system runs, but the key service makes no device key and the TLS service refuses every connection (`[KEYSTORE] NO RNDR: NO DEVICE KEY`), as the `tls` suite shows for a Cortex-A72 under TCG. Under TCG (`-cpu max`, section 5) RNDR is there.
- **Device registers.** Under HVF, QEMU emulates an access to a device register only when the processor describes the access to the hypervisor in full: one load or store of one register, without a pair and without writing back the base register. Other forms are expected to stop QEMU with an internal error. TCG emulates every instruction, so CI cannot show whether the kernel or a driver uses such forms on device memory.
- **Interrupts.** The GICv3 is QEMU's own model, as under TCG. Whether QEMU gives the ITS (MSI-X) under HVF has not been checked; without it the kernel leaves drivers their wired lines, as the profile says for `virtio_net` (`ITS=0x0` in the `BOARD` line).
- **Time.** The generic timer counts at the Mac's frequency. The kernel reads the frequency from `CNTFRQ_EL0`, so this is expected to make no difference.
- **Speed.** The virtual CPUs share the Mac's cores with macOS. Timings differ from TCG on Linux, where the suites' time limits were set.

## 5. The tests on a Mac

The QEMU suites start QEMU themselves with `-cpu max` and no accelerator, so on a Mac they run under TCG, as CI does, and test nothing of HVF. They look for the firmware where Debian puts it; name Homebrew's:

```bash
PATH="$(brew --prefix gnu-sed)/libexec/gnubin:$PATH" "$(brew --prefix)/bin/bash" scripts/build_aarch64.sh --fixtures
FW="$(brew --prefix)/share/qemu"
python3 tests/aarch64_smoke.py --code "$FW/edk2-aarch64-code.fd" --vars "$FW/edk2-arm-vars.fd"
python3 tests/qemu_smoke.py --arch aarch64 --aavmf-code "$FW/edk2-aarch64-code.fd" --aavmf-vars "$FW/edk2-arm-vars.fd"
```

**Not yet tested on a Mac.** What is expected to differ from Linux:

- The `normal` and `smp` suites measure QEMU's processor time through `/proc/<pid>/stat`, which macOS does not have, so they are expected to fail at that check. `--suites shell,vfs,store,net,tls,busy` leaves them out.
- The `vfs` suite needs `mkfs.fat`, `fsck.fat` and mtools (`brew install dosfstools mtools`); without them it reports SKIP.
- The `tls` suite runs `openssl` for its certificates and test servers, with OpenSSL's options. macOS's own `openssl` is LibreSSL, whose options may differ: install `openssl@3` from Homebrew and put `"$(brew --prefix openssl@3)/bin"` first in `PATH`.
- The host tests (`rustc --test tests/*_host.rs`, CI's list) build for the Mac itself; not tried.

Making the suites run on a Mac, under TCG and under HVF, is task [600-APL-0012](../issues/600-APL-0012-aarch64-suites-on-a-mac.md).

## 6. UTM

[UTM](https://mac.getutm.app), a macOS front end for QEMU, runs QEMU with the same hypervisor, so the machine of section 3 can in principle be set up in it: an ARM64 `virt` machine with UEFI boot, a `ramfb` display and the USB image of section 3 as its disk. This guide gives no UTM steps, because none have been tried. If they work, task 600-APL-0011 adds them here.

## 7. Natively: not supported yet

MIND Core cannot run on an Apple Silicon Mac without a virtual machine. There is nothing to install on a Mac for it yet. The reasons are those of issue [210](../issues/210-apple-silicon-native.md); its tasks are in section 8. The hardware facts below come from the Asahi Linux project's documentation; none has been checked on a Mac here.

- **No UEFI.** Apple's iBoot boots the Mac, and `BOOTAA64.EFI` is a UEFI application. The plan is to boot as Asahi Linux does: its boot loader m1n1 starts U-Boot, which gives a UEFI environment. Installing them with the Asahi installer means lowering the boot security of the new boot entry once, in recoveryOS; macOS stays. Task 210-APL-0001. A loader of our own can later replace U-Boot after m1n1 (task 210-APL-0013), and then m1n1 too, as a raw image that iBoot starts once `kmutil` allows it (task 210-APL-0014). The cost of the second step is the per-core and power set-up m1n1 does, and an NVMe reader for updates without recoveryOS.
- **No ACPI.** The kernel reads the machine's layout from ACPI tables (MADT, SPCR, GTDT, MCFG, FADT). A Mac is described by a device tree instead. Task 210-APL-0002.
- **Not a GIC.** The interrupt controller is Apple's AIC (AIC2 on the M1 Pro, Max and Ultra and on later chips), and the timer's interrupt arrives as an FIQ, which the kernel does not take. Task 210-APL-0003.
- **No PSCI.** The other CPUs start through a spin table, not PSCI `CPU_ON` (task 210-APL-0004); reset goes through a watchdog, not PSCI `SYSTEM_RESET` (task 210-APL-0005).
- **The console.** The UART is Samsung-style, not a PL011, and is reachable only through a USB-C port switched to a debug mode. Task 210-APL-0006.
- **DMA behind IOMMUs.** Every DMA-capable device reaches memory only through its own IOMMU (DART), with 16 KiB pages where the kernel's are 4 KiB. Task 210-APL-0007. The DARTs would also give MIND Core the DMA boundary of MC-1.5, which no platform of it has yet.
- **USB.** The Type-C ports are Synopsys DWC3 controllers with Apple's PHY (ATC) and power domains (PMGR), not an xHCI on PCI as `usb_host` finds today. Task 210-APL-0008.
- **Built-in devices.** The internal keyboard and trackpad (SPI or MTP), the NVMe (ANS with RTKit firmware), Wi-Fi, sound and the display controller (DCP) are Apple's own. They come later, as separate issues, reimplemented from documentation: Linux's drivers are GPL and cannot be copied into MIND Core.

The first goal is issue 210's acceptance: an M1 Mac mini or MacBook boots to the shell on its screen from a USB stick, with an external USB keyboard.

## 8. Open tasks

Track `APL` is open: anyone may take it ([TRACKS.md](../TRACKS.md), [AGENTS.md](../AGENTS.md)). The tasks that need a Mac wait for a person with one ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)).

| Task | What |
|---|---|
| [600](../issues/600-apple-silicon-mac-vm-host.md) | A Mac as a host: the aarch64 system in a virtual machine with HVF, built, run and tested on macOS |
| [600-APL-0009](../issues/600-APL-0009-aarch64-build-on-macos.md) | The aarch64 build with the Bash and sed macOS ships |
| [600-APL-0010](../issues/600-APL-0010-run-script-on-macos.md) | `03_run_qemu_aarch64.sh` on macOS: the screen in a window, Bash 3.2, a choice of accelerator |
| [600-APL-0011](../issues/600-APL-0011-first-run-on-a-mac.md) | The first run on a Mac, by hand, recorded here and in the profile |
| [600-APL-0012](../issues/600-APL-0012-aarch64-suites-on-a-mac.md) | The aarch64 suites on a Mac under TCG and HVF; evidence for the HVF configuration |
| [210](../issues/210-apple-silicon-native.md) | Apple Silicon Macs natively, M1 first |
| [210-APL-0001](../issues/210-APL-0001-boot-through-m1n1-and-u-boot.md) | Boot through m1n1 and U-Boot: `BOOTAA64.EFI` from U-Boot's UEFI, entry at EL2 |
| [210-APL-0002](../issues/210-APL-0002-board-from-the-device-tree.md) | The board from the device tree where there is no ACPI |
| [210-APL-0003](../issues/210-APL-0003-aic-and-the-timer-fiq.md) | The AIC (AIC2) and the timer's FIQ |
| [210-APL-0004](../issues/210-APL-0004-cpus-through-the-spin-table.md) | The other CPUs through the spin table |
| [210-APL-0005](../issues/210-APL-0005-reset-without-psci.md) | Reset through the watchdog, without PSCI |
| [210-APL-0006](../issues/210-APL-0006-samsung-style-uart-console.md) | The console on the Samsung-style UART |
| [210-APL-0007](../issues/210-APL-0007-dart-dma-boundary.md) | DART: each device's DMA only through its own IOMMU; 16 KiB pages |
| [210-APL-0008](../issues/210-APL-0008-usb-on-type-c-ports.md) | USB on the Type-C ports: DWC3 in host mode, the ATC PHY, PMGR power |
| [210-APL-0013](../issues/210-APL-0013-own-stage-two-instead-of-u-boot.md) | Our own second stage after m1n1, without U-Boot |
| [210-APL-0014](../issues/210-APL-0014-own-first-stage-instead-of-m1n1.md) | Our own first stage started by iBoot, without m1n1 |

## 9. What has been tested

Nothing on a Mac. When a step of this guide has been run, its "not yet tested on a Mac" mark is replaced by what was run, on which Mac, macOS and QEMU, and the aarch64 profile records it as a configuration of its own (MC-12.1).
