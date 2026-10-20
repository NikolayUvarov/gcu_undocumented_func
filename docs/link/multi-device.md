# The MIND Link multi-device

Russian: [multi-device_RU.md](multi-device_RU.md).

The multi-device is two RP2040 or RP2350 boards joined by two wires and ground ([800-LNK-0005](../../issues/800-LNK-0005-the-rp2-multi-device-firmware.md)). Each board plugs into its own machine's USB port. Two machines cannot otherwise talk over USB, since both are hosts. The pair gives each machine:
- a serial port to the other machine;
- a fast data port, for [`mindlink`](mindlink.md) and its files;
- control of the other machine's keyboard, mouse and tablet.

Both boards run the same firmware, from `devices/rp2-link/`.

**Status (2026-10-20):** the firmware builds for every board kind, and its logic passes host tests. It has run on no board yet, so nothing on this page is measured. The figures below are design values.

## Building and flashing

```bash
devices/rp2-link/build.sh                 # every board, or name some: build.sh rp2040-zero rp2350-zero
# devices/rp2-link/dist/mind-link-<board>.uf2
```

- **Needs:**
  - `arm-none-eabi-gcc`, `cmake` and `ninja`;
  - the Pico SDK 2.2: `$PICO_SDK_PATH`, or the maintainer's `/opt/rp2040sens01-toolchain`.
- **The script first runs the host tests.**
- **To flash:** hold BOOTSEL while plugging the board in, then copy its UF2 to the drive that appears.

| Board | File | LED |
|---|---|---|
| Waveshare RP2040-Zero | `mind-link-rp2040-zero.uf2` | WS2812 on GP16 |
| YD-RP2040 | `mind-link-rp2040-yd.uf2` | WS2812 on GP23 |
| Raspberry Pi Pico | `mind-link-rp2040-pico.uf2` | the green LED on GP25, on or off |
| Waveshare RP2350-Zero | `mind-link-rp2350-zero.uf2` | WS2812 on GP16 |
| Raspberry Pi Pico 2 | `mind-link-rp2350-pico2.uf2` | the green LED on GP25, on or off |

The two boards need not be the same kind.

## Wiring

| From board A | To board B |
|---|---|
| GP4 (sends) | GP5 (receives) |
| GP5 (receives) | GP4 (sends) |
| GND | GND |

- **The line runs at 15 Mbit/s,** 8N1, 3.3 V, with a pull-up on the receiving pin. Keep the wires short, about 20 cm. For longer ones, build both boards for a lower rate:
  ```bash
  cmake -DLINK_BAUD=7500000 ...
  ```
  `LINK_TX_PIN` and `LINK_RX_PIN` move the line to other pins.
- **Do not join the two boards' 3.3 V or 5 V pins.** Each is powered by its own machine.

## What each machine sees

One USB device, "MIND Core MIND Link", whose serial number is the board's unique ID. Each class has a driver in Linux, Windows 10 and later, macOS and MIND Core (800-LNK-0006), so nothing is installed.

| Interface | What | Linux name |
|---|---|---|
| 0–1 | **serial**: a CDC-ACM port | `/dev/serial/by-id/usb-MIND_Core_MIND_Link_<serial>-if00` |
| 2–3 | **data**: a CDC-ACM port | `…-if02` |
| 4–5 | **control**: a CDC-ACM port | `…-if04` |
| 6 | keyboard, boot protocol | — |
| 7 | mouse, boot protocol | — |
| 8 | tablet: an absolute pointer, 0–32767 on each axis | — |

- **Serial and data ports:**
  - the bytes written to one come out of the other board's port of the same name;
  - the rate set on a port does not matter;
  - each direction is flow-controlled end to end: a board takes no more from its machine than the other board has room for;
  - bytes for a port that nobody has open are dropped, as on a serial line.
- **Files:** `mindlink` runs over the data port as over any serial port:
  ```bash
  # the machine under test:   mindlink /dev/serial/by-id/...-if02 serve ~/link
  # the development machine:  mindlink /dev/serial/by-id/...-if02 put build.img
  ```

## The control port

Lines of text, each ended by LF. `mindlink <control port> board <line>` sends one and prints the answer.

Input commands are played on the **other** board's keyboard, mouse or tablet, so on the other machine. They answer `ok`, or `err <reason>`.

| Line | Does |
|---|---|
| `hello` | `MIND Link 1 <board> <serial> peer <the other board's serial, or none>` |
| `status` | the other board's state and the counts: bytes each way, bytes lost, damaged frames |
| `type <text>` | types ASCII text on the US layout, at most 249 characters a line |
| `key <chord>` | presses a chord and releases it: `key ctrl+alt+delete`, `key gui+r`, `key f2`, `key enter` |
| `move <dx> <dy> [<wheel>]` | moves the mouse by that much |
| `wheel <n>` | turns the wheel |
| `click [left\|right\|middle]` | a click |
| `press <button>`, `release <button>` | holds or lets go a button; moves carry it, for dragging |
| `abs <x> <y>` | puts the tablet's pointer at x, y, from 0 to 32767 across the screen |

**Key names:**
- letters, digits and punctuation;
- `enter`, `esc`, `backspace`, `tab`, `space`, `delete`, `insert`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`, `capslock`, `printscreen`, `scrolllock`, `pause`, `menu` and `f1`–`f12`;
- the modifiers `ctrl`, `shift`, `alt`, `gui` (`win`), and their right-hand `rctrl`, `rshift`, `ralt` (`altgr`), `rgui`.

The keyboard and mouse use the boot protocol, so they work in firmware menus. The tablet needs a system with a HID driver.

**The indicator's lines** (issues-human section 8) are taken too, and get no answer. A machine running MIND Core can send them to show its boot on its own board's LED:
- `@STATE KERNEL`, `@STATE SERVICES <n>/<all>`, `@STATE READY`, `@STATE REBOOT`, `@STATE OFF`;
- `@HEALTH OK|WARN|FAIL <reason>`;
- `@BEAT <ms>`.

## The LED

| Colour | Meaning |
|---|---|
| Dim blue | Powered; its machine has not set it up |
| Dim cyan | Its machine has, and the other board does not answer |
| Dim green | Both boards answer; brighter while data moves |
| Red, blinking | A damaged frame, a lost byte or an overrun within the last second |

Once its machine sends `@STATE`, the board shows that machine's boot instead:
- yellow, brighter as services start;
- green when `READY`;
- orange on `HEALTH WARN`, red on `FAIL`;
- magenta blinking when the beats stop for 3 s;
- white flashes on `REBOOT` or `OFF`.

## Between the boards

- **Frames:** COBS-encoded and ended by a zero byte. Each carries:
  - a type (hello, data, credit) and a channel (serial, data, control);
  - the stream offset of its first byte, and up to 256 bytes;
  - CRC-16/CCITT.
- **Credits:** a board sends at most 4 KiB beyond what the other board's machine has taken. Credits carry absolute offsets, so a lost credit is made good by the next.
- **A damaged frame** is dropped and counted. Its bytes are lost, and the receiver skips past them rather than waiting. The protocol on the data port (`linkproto`) sends its request again; the serial port's bytes are simply lost, and the LED shows it.
- **The cores:** core 1 serves the line's two PIO state machines. Core 0 serves USB, the frames and the HID interfaces.

## Expectations, not yet measured

- **The line:** 15 Mbit/s carries about 1.4 MB/s of frames each way, more than USB full speed's roughly 1 MB/s.
- **`mindlink` through the pair** will move less than the ports can carry. Each 1 KiB request waits for its answer over two USB hops. A protocol that keeps several requests in flight is a later step.
- **The acceptance's 0.9 MB/s** is measured on the data port itself.

## Tests

`devices/rp2-link/tests/host_test.c`, run by `build.sh` and `scripts/host_tests.sh`:
- COBS and the CRC;
- every damaged byte of a frame caught;
- noise and an overlong run;
- credits across the offsets' wrap;
- a stream through a line that loses 3 % of frames, which never overflows the window;
- every control command;
- the LED's table.

These are evidence for the logic only, not for a board (MC-12.1).
