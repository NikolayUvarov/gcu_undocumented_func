# Landing request: the boards' catalog, their card in one command, the first boot's page and script, the profiles' skeletons

**From:** MOB · **To:** `LANDING` · **Date:** 2026-11-26

**Ready:** `claude/MOB-boards` 4622c45ac

**What it is.** Seven commits of a worker of this session, with `main` 5dccdc819 merged in (one text conflict in `issues/README.md`, both sides kept). No board is wired: nothing here was run on one, and the texts say so.

**Closes:** 352-MOB-0010 (request 1 of `requests-MOB.md`, taken out of that file): `scripts/boards.toml`, the catalog `board_image.py --board <name>` reads, with the two Orange Pi boards' entries; `board_uboot.sh` and `board_image.py` take their facts from it; `board_image.py` makes a card from a given image or boot volume.

**Advances:**
- 100004-MOB-0011: `build_and_write.sh --board <name>`, one command for a board's card, with the writer's checks unchanged. Open: a real write, the maintainer's act.
- 100004-MOB-0012: `docs/boards/allwinner-first-boot.md` (and `_RU`) and `scripts/board_stage1.py`, which checks stage 1's eight criteria from a console's text. Open: the boards' runs.
- 100004-MOB-0013: `docs/profile/aarch64-orangepi-zero-plus2/` and `…-zero3/`, every line "not tested", "host test only" or "not made".

**Tested, and where** (LEG9, x86 host; no aarch64 group was run by MOB: the kernel, the loader and the services are not touched):
- `scripts/host_tests.sh` whole at 3f055cfa7, after `./02_build.sh`, with `MIND_SSHD` set: exit 0 (10:12 to 10:31 UTC);
- on the merge 4622c45ac: `tests/board_image_test.py` 9 OK, `tests/board_stage1_test.py` 9 OK, `tests/build_and_write_args_test.py` 19 OK;
- `build_and_write_keep_test.sh` and `build_and_write_machines_test.sh`: all passed (3f055cfa7);
- both boards' firmware rebuilt from the catalog: the bytes as before the move;
- `board_stage1.py` against QEMU `virt` under U-Boot's UEFI with four Cortex-A53: 8 of 8 met, twice. That is evidence for QEMU only (MC-12.1).

**For the gate:** `scripts/host_tests.sh` gains `tests/board_stage1_test.py`; `tests/data/stage1/` holds two recorded consoles. `THIRD_PARTY.md` changes (it names the catalog). In `fast-test` (c3fcf14ef).

**With the requests that wait:** it touches `issues/README.md` as they do, in other rows.
