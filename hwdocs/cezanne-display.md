# The Ryzen's Radeon display engine: what the firmware leaves and what its VBIOS holds (213-DRV-0078)

A survey for a display driver, read on the Ryzen (un-rizen) on 2026-11-20. Nothing here was written to the hardware.

## The engine

- **The device:** the Cezanne APU's Radeon, PCI 05:00.0, `1002:1638`.
- **Under Linux** (`amdgpu`, kernel 7.0.0-43):
  - `Fetched VBIOS from VFCT` (the ACPI table), `ATOM BIOS: 113-CEZANNE-021`;
  - `Display Core v3.2.369 initialized on DCN 2.1`;
  - `Loading DMUB firmware via PSP: version=0x0101002B`.

## The outputs

**The firmware** sets up one: the loader sees `1 GRAPHICS OUTPUTS`, GOP 0, 2560x1600, its EDID the 2560x1600 monitor's (213-DRV-0074).

**Linux's connectors:**

| Connector | State | Preferred mode | In MIND Core |
|---|---|---|---|
| `card1-DP-2` | connected | 2560x1600 | the firmware's output, lit |
| `card1-HDMI-A-1` | connected | 3440x1440 | dark |
| `card1-DP-1` | not connected | — | — |
| `card1-HDMI-A-2` | not connected | — | — |

## The VBIOS image

The image is 55,296 bytes, read from `amdgpu`'s debugfs file `amdgpu_vbios`. Its SHA-256 is `e04f13be…64c9`. It stays on the machine, as it is the vendor's.

**The header.** The PCI ROM signature is `55 AA`. The ATOM ROM header is at 0x188, signature `ATOM`, 40 bytes. The master command table is at 0x93FA, the master data table at 0x94A0.

**Data tables present:**

| Index | Table | Format | Size |
|---|---|---|---|
| 4 | firmwareinfo | 3.2 | 72 |
| 8 | smu_info | 3.1 | 28 |
| 12 | gpio_pin_lut | 2.1 | 84 |
| 14 | gfx_info | 2.3 | 52 |
| 15 | powerplayinfo | 9.1 | 1024 |
| 22 | displayobjectinfo | 1.4 | 235 |
| 23 | indirectioaccess | 1.1 | 5 |
| 27 | dce_info | 4.3 | 56 |
| 30 | integratedsysteminfo | 1.12 | 1024 |

The image also has tables 3, 5, 6, 7, 11, 13 and 17.

**Command tables:** 34 present. Their indices are 0, 4, 6, 9, 12, 13, 19–21, 23, 33–35, 37, 39, 41–46, 49–51, 53, 54, 56, 60, 61 and 75–79. Which of them DC still calls is the survey's next step.

**The display paths** (`displayobjectinfo` 1.4, six paths, supported devices `0x0688`):

| Path | Connector | Encoder (PHY) | Device tag |
|---|---|---|---|
| 0 | HDMI type A, enum 3 (`0x330C`) | UNIPHY enum 1, PHY A (`0x211E`) | `0x0200` |
| 1 | HDMI type A, enum 4 (`0x340C`) | UNIPHY enum 2, PHY B (`0x221E`) | `0x0400` |
| 2 | none | UNIPHY1 enum 1, PHY C (`0x2120`) | — |
| 3 | DisplayPort, enum 1 (`0x3113`) | UNIPHY1 enum 2, PHY D (`0x2220`) | `0x0008` |
| 4 | DisplayPort, enum 2 (`0x3213`) | UNIPHY2 enum 1, PHY E (`0x2121`) | `0x0080` |
| 5 | object `0x7103` | none | — |

How Linux's connector names map onto these paths is not read yet. The guess is that `DP-2` is path 4 and `HDMI-A-1` is path 0; the DCN registers of the lit pipe will tell.

## Not done yet

- The DCN 2.1 registers of the lit pipe: HUBP, DPP, MPC, OPP and OTG, the DIG, the PHY and DCCG's clocks.
- Which command tables DC uses on DCN 2.1.
- Whether a mode set on a second output needs DMUB.
