# Evidence — `aarch64/OrangePi-ZeroPlus2-0` (a skeleton: no run on the board yet)

A statement of this profile becomes tested only by a run on this board, recorded here with its configuration: the board's revision, the card's SHA-256, U-Boot's and TF-A's versions from the banner, the adapter's chip, the host and its port (MC-12.1). Tests are not proofs (MC-12.2).

## Stage 1 on the board (100004-MOB-0002)

Checked from the console by `scripts/board_stage1.py` ([docs/boards/allwinner-first-boot.md](../../boards/allwinner-first-boot.md)).

| Statement | Run | Status |
|---|---|---|
| U-Boot's banner, then our loader's lines | — | not tested |
| The kernel's board line from U-Boot's tree, then `INIT STARTED` | — | not tested |
| Four CPUs online, each ticking | — | not tested |
| The kernel's clock against the host's | — | not tested |
| The memory map, 512 MiB less the firmware's | — | not tested |
| The shell's prompt on UART0 | — | not tested |
| A reboot through PSCI | — | not tested |
| The bench's components | — | to be named |

## What exists before the board, and what it is evidence of

| Statement | Evidence | Of what only |
|---|---|---|
| The kernel's reader takes the GIC, the timer, PSCI, four CPUs and the 16550 console from Linux v6.12's tree of this board | `tests/fdt_host.rs` (`the_orange_pi_zero_plus2s_tree`), `tests/nodes_host.rs` | Linux's tree, not U-Boot's copy, on the host |
| The card's layout: U-Boot at 8 KiB, the partitions from 4 MiB, the volumes read whole | `tests/board_image_test.py` | the card's bytes, made on the host |
| MIND Core boots to the shell under U-Boot's UEFI at EL2 on four Cortex-A53 | the CI group `aarch64: the board from a device tree, under U-Boot` | QEMU `virt` only (MC-12.1) |
