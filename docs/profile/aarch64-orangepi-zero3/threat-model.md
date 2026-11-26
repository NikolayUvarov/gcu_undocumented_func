# Threat and fault model — `aarch64/OrangePi-Zero3-0` (a skeleton, not tested)

The assets, adversaries and faults of [`aarch64/QEMU-virt-0`](../aarch64/threat-model.md), with what a bench board changes. Nothing here has been seen on the board (MC-12.3).

## Differences

| Topic | On this board |
|---|---|
| The debug UART | Whoever holds its cable reads every boot line and types at U-Boot's prompt, which can rewrite the card, and at the shell once it runs. U-Boot's countdown stops only on the typed word `stop`. The cable is the owner's, as a PC's keyboard is. |
| The card | Removable: whoever holds it can replace the firmware before our loader, which nothing checks (MC-9.1). |
| The firmware's network boot | Off in this card's U-Boot (`CONFIG_NO_NET`, 351-KRN-0195). |
| Malicious application | As on `virt`, at EL0. |

## Fault model: differences

| Fault | Behaviour, as the code means it |
|---|---|
| A frozen system | The A31's watchdog resets the board (100004-PRT-0015). |
| A card that does not boot | The boot ROM finds no SPL at 8 KiB and goes on to its other sources, the SPI NOR flash and FEL (U-Boot's `doc/board/allwinner/sunxi.rst`); the console shows nothing of ours. |
| A wrong rail | The system leaves the AXP313a as the SPL set it (100004-MOB-0007 adds its safety rules). |
| Power cut with the adapter plugged in | The adapter's TX, 3.3 V when idle, can keep the board half on through its RX; a resistor of about 1 kΩ in series prevents it (issues-human, section 14). |
| Power and the bench's switch | To be named with the bench's power device (100004-MOB-0009). |
