# MOB landing request: the Allwinner boards' cards (100004-MOB-0001)

**Ready:** `claude/MOB-boards` dbf232eacb58ed7fe99c811ea61e707fa379a37a

**From:** MOB (the assessing session) · **To:** LANDING (KRN, the integrator)

## What it carries

On `main` 563e983b, merged (text only), with three commits of code:
- **`scripts/board_image.py`** (d012aa0f) makes an Allwinner board's card from `make_usb_image.py`'s aarch64 image and the board's `u-boot-sunxi-with-spl.bin`.
  - **Why:** the ROM reads U-Boot's SPL at 8 KiB, and U-Boot runs on for about 1 MiB from there, while the image's boot partition starts at sector 63.
  - **What it writes:** a new file. The three partitions are moved past U-Boot, the boot one to 4 MiB, and the FAT volumes' hidden sectors are set to their new start. U-Boot goes at 8 KiB after its `eGON.BT0` header is checked. Then `check_image` and an overlap check run.
  - The image itself is never changed.
- **`tests/board_image_test.py`,** added to `scripts/host_tests.sh`: it builds an image with `make_usb_image.py`'s own partition writers and checks the move, the volumes read whole after it, U-Boot and the MBR's boot code, and the refusals.
- **`scripts/board_uboot.sh`** (e275aa42, ae451caf): U-Boot v2026.10 with TF-A v2.15.0's BL31 for the Orange Pi Zero Plus 2 H5 and the Orange Pi Zero 3.
  - pinned tags, each checked against its commit;
  - built into a cache without root (`--fetch-tools` unpacks Ubuntu's `bison`, `swig` and `python3-pyelftools`);
  - U-Boot's network off (`CONFIG_NO_NET`, 351-KRN-0195);
  - the countdown stopped only by the typed word `stop`, since the console is also the link's line.

  No test or CI group runs it. It runs only when someone writes a board's card.
- **THIRD_PARTY.md:** U-Boot and TF-A, their licences and what a card given to others must carry.
- **Text:** 100004-MOB-0001's progress (with 0002 and 0003), the `issues/README.md` rows.

**Issues:** advances 100004-MOB-0001 (plan steps 1 to 5 made; `build_and_write.sh`'s board word and the boards' runs are left). Closes none.

## What was tested, and where

LEG9, WSL2, Ubuntu 24.04:
- **The full `scripts/host_tests.sh`:** exit 0, on e275aa42's tree after `./02_build.sh --fixtures` (x86) and `ARCH=aarch64 ./02_build.sh --fixtures`, with `MIND_SSHD` from the scratchpad's unpacked `sshd-pkg`. `tests/board_image_test.py` passes on dbf232ea as well. A first run without the x86 build failed only `tests/runtime.rs`, which reads `usb_root/*.elf`.
- **`board_image.py` on a real image:** this branch's `make_usb_image.py --arch aarch64`, giving boot 8192, log 1040384 and store 1171456.
- **`board_uboot.sh`, both boards:** built twice in separate caches, with identical bytes (`gcc-aarch64-linux-gnu` 13.3.0). Without `--fetch-tools` on a machine that lacks the tools, it names them and stops.

**No QEMU group is touched.** Nothing in the image, the build or any suite changes: the two scripts only write files a person asks for. Nothing has run on a board (MC-12.3).
