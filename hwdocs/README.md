# hwdocs/ — hardware descriptions kept apart from the system

**Status:** format proposed; the first files come with issue [207](../issues/207-gpio-service.md).

Facts about hardware that the system cannot read from the hardware itself — which signal each multiplexed function of a pin carries, how a board's header is numbered, which pins the board's firmware uses — are taken from vendors' documentation and kept here, as small text files. They are **not** part of the kernel or of the boot image: `02_build.sh` does not copy them; `scripts/make_usb_image.py --hwdocs` (to be added with 207) puts them in `/hwdocs` on the boot volume, and the tools (`pins`) and services (`gpio`) read them through the VFS when they are there. Without them everything still works and shows function numbers instead of names.

## Layout

```
hwdocs/
  socs/<soc>.pins       one SoC's pin controller: its pins and their functions (bcm2711.pins, pl061.pins)
  boards/<board>.board  one board: its SoC, how it is identified, its header, the pins its firmware uses (rpi4b.board)
```

## `.pins`

The values in the two examples below show the format only; the real files are checked against their sources.

```
soc bcm2711
source "BCM2711 ARM Peripherals", Raspberry Pi Ltd, version 1 (2022), section 5.3 "Alternative Function Assignments"
pins 58
functions in out alt0 alt1 alt2 alt3 alt4 alt5
# pin  default-pull  alt0  alt1  alt2  alt3  alt4  alt5     (- : reserved / no function)
14     none          TXD0  SD6   -     -     RTS5  TXD1
```

One line per pin; a field is one word. `in` and `out` are implicit for every pin of a GPIO controller and are not listed.

## `.board`

```
board rpi4b
name "Raspberry Pi 4 Model B"
soc bcm2711
match acpi-oem "RPIFDN" "RPI4    "      # FADT OEM ID and OEM table ID as the kernel reports them (issue 206)
header J8 40
# position  pin
8           14
10          15
reserved 14 15  console UART (the firmware's serial console)
reserved 40 41  example: pins a board's firmware keeps for itself
```

## Rules

- Each file names its **source** (document, publisher, version, section). Tables transcribed from a datasheet are facts about the hardware; the source is also recorded in [THIRD_PARTY.md](../THIRD_PARTY.md). No vendor PDFs are stored here.
- A file is checked by a host test (the `gpio` parser, issue 207) before it is used.
- Small and plain: no generated or binary files.
