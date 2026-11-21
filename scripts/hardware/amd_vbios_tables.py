#!/usr/bin/env python3
"""Read-only: an AMD ATOM BIOS image's tables, as a display driver needs them (213-DRV-0085).

Usage: python3 -I scripts/hardware/amd_vbios_tables.py VBIOS.bin

Prints the ROM header, the master command and data tables (named), the display object paths with their records,
the GPIO pin table, the firmware's VRAM reservation and the integrated system information's external display paths
(v1.11/v1.12). It reads the image only; the image is the vendor's and stays out of the repository.

The layouts are those of Linux v7.0 `drivers/gpu/drm/amd/include/atomfirmware.h` (AMD, MIT licence):
`atom_rom_header_v2_2`, `atom_master_list_of_command_functions_v2_1`, `atom_master_list_of_data_tables_v2_1`,
`display_object_info_table_v1_4`, `atom_display_object_path_v2`, the record types of `atom_object_record_type_id`,
`atom_gpio_pin_lut_v2_1`, `vram_usagebyfirmware_v2_1`, `atom_integrated_system_info_v1_12` and
`atom_external_display_connection_info`. Only names and offsets are used here.
"""
import struct
import sys

COMMANDS = {0: "asic_init", 4: "digxencodercontrol", 10: "setengineclock", 11: "setmemoryclock", 12: "setpixelclock",
            13: "enabledisppowergating", 34: "blankcrtc", 35: "enablecrtc", 41: "getsmuclockinfo",
            42: "selectcrtc_source", 46: "setdceclock", 47: "getmemoryclock", 48: "getengineclock",
            49: "setcrtc_usingdtdtiming", 50: "externalencodercontrol", 54: "processi2cchanneltransaction",
            60: "computegpuclockparam", 63: "dynamicmemorysettings", 64: "memorytraining", 67: "setvoltage",
            69: "readefusevalue", 76: "dig1transmittercontrol", 78: "processauxchanneltransaction", 80: "getvoltageinfo"}
DATA = {0: "utilitypipeline", 1: "multimedia_info", 2: "smc_dpm_info", 4: "firmwareinfo", 6: "lcd_info", 8: "smu_info",
        11: "vram_usagebyfirmware", 12: "gpio_pin_lut", 14: "gfx_info", 15: "powerplayinfo", 22: "displayobjectinfo",
        23: "indirectioaccess", 24: "umc_info", 27: "dce_info", 28: "vram_info", 30: "integratedsysteminfo",
        31: "asic_profiling_info", 32: "voltageobject_info"}
OBJECT_TYPES = {1: "GPU", 2: "ENCODER", 3: "CONNECTOR", 4: "ROUTER", 5: "GENERIC"}
ENCODERS = {0x1E: "UNIPHY", 0x20: "UNIPHY1", 0x21: "UNIPHY2", 0x25: "UNIPHY3"}  # objectid.h's internal encoders
CONNECTORS = {0x01: "DVI_I", 0x03: "DVI_D", 0x0C: "HDMI_TYPE_A", 0x0D: "HDMI_TYPE_B", 0x13: "DISPLAYPORT",
              0x14: "eDP", 0x18: "USBC"}
RECORDS = {1: "I2C", 2: "HPD_INT", 3: "CONNECTOR_CAP", 4: "CONNECTOR_SPEED", 9: "GPIO_CNTL", 16: "HPDPIN_LUT",
           17: "AUXDDC_LUT", 20: "ENCODER_CAP", 21: "BRACKET_LAYOUT", 22: "FORCED_TMDS_CAP", 23: "DISP_CONNECTOR_CAPS",
           25: "BRACKET_LAYOUT_V2"}


def u16(b, o): return struct.unpack_from("<H", b, o)[0]
def u32(b, o): return struct.unpack_from("<I", b, o)[0]


def object_name(oid):
    kind, enum, num = (oid >> 12) & 7, (oid >> 8) & 0xF, oid & 0xFF
    sub = CONNECTORS.get(num) if kind == 3 else ENCODERS.get(num) if kind == 2 else None
    return "%04x %s %s enum %d" % (oid, OBJECT_TYPES.get(kind, kind), sub or hex(num), enum)


def phy_letter(oid):
    # UNIPHY enum 1/2 = PHY A/B, UNIPHY1 = C/D, UNIPHY2 = E/F (Display Core's transmitter for an encoder object)
    base = {0x1E: 0, 0x20: 2, 0x21: 4, 0x25: 6}.get(oid & 0xFF)
    enum = (oid >> 8) & 0xF
    return "PHY " + "ABCDEFGH"[base + enum - 1] if base is not None and enum in (1, 2) else "-"


def table_list(b, base, names):
    out = []
    for i in range((u16(b, base) - 4) // 2):
        o = u16(b, base + 4 + 2 * i)
        if o:
            out.append((i, names.get(i, "index %d" % i), o, u16(b, o), b[o + 2], b[o + 3]))
    return out


def records(b, start):
    out, o = [], start
    while o + 2 <= len(b) and b[o] not in (0, 0xFF):
        kind, size = b[o], b[o + 1]
        if size < 2:
            break
        out.append((kind, b[o:o + size]))
        o += size
    return out


def describe_record(kind, r):
    name = RECORDS.get(kind, "type %d" % kind)
    if kind == 1:
        i2c = r[2]
        return "%s id %02x (line %d, engine %d, hw %d) slave %02x" % (name, i2c, i2c & 0xF, (i2c >> 4) & 7, i2c >> 7, r[3])
    if kind == 2:
        return "%s pin %02x plugged-state %02x" % (name, r[2], r[3])
    if kind in (4, 20) and len(r) >= 6:
        return "%s %08x" % (name, u32(r, 2))
    if kind == 3 and len(r) >= 4:
        return "%s %04x" % (name, u16(r, 2))
    return "%s %s" % (name, r[2:].hex())


def main(path):
    b = open(path, "rb").read()
    print("image %d bytes, signature %s" % (len(b), b[0:2].hex()))
    rom = u16(b, 0x48)
    print("ATOM ROM header at %#x: %s, size %d, format %d.%d" % (rom, b[rom + 4:rom + 8], u16(b, rom), b[rom + 2], b[rom + 3]))
    print("  subsystem %04x:%04x" % (u16(b, rom + 0x18), u16(b, rom + 0x1A)))
    cmd, data = u16(b, rom + 0x1E), u16(b, rom + 0x20)
    print("  master command table %#x, master data table %#x" % (cmd, data))
    tables = {name: (o, size, f, c) for _, name, o, size, f, c in table_list(b, data, DATA)}
    print("-- data tables (index, name, offset, size, format.content)")
    for i, name, o, size, f, c in table_list(b, data, DATA):
        print("  %2d %-22s %#06x %5d %d.%d" % (i, name, o, size, f, c))
    print("-- command tables (index, name, offset, size, format.content, workspace/parameter bytes)")
    for i, name, o, size, f, c in table_list(b, cmd, COMMANDS):
        attr = u16(b, o + 4)
        print("  %2d %-28s %#06x %5d %d.%d ws %d ps %d" % (i, name, o, size, f, c, (attr & 0xFF) * 4, ((attr >> 8) & 0x7F) * 4))
    if "displayobjectinfo" in tables:
        o = tables["displayobjectinfo"][0]
        n = b[o + 6]
        print("-- display paths (displayobjectinfo %d.%d, supported devices %04x, %d paths)" % (b[o + 2], b[o + 3], u16(b, o + 4), n))
        for p in range(n):
            a = o + 8 + 16 * p
            disp, drec, enc, ext, erec, xrec, tag, prio = struct.unpack_from("<HHHHHHHB", b, a)
            print("  path %d: %s | encoder %s (%s) | ext %04x | device tag %04x | priority %d" % (
                p, object_name(disp), object_name(enc), phy_letter(enc), ext, tag, prio))
            for kind, r in records(b, o + drec) if drec else []:
                print("     connector record: " + describe_record(kind, r))
            for kind, r in records(b, o + erec) if erec else []:
                print("     encoder record: " + describe_record(kind, r))
    if "gpio_pin_lut" in tables:
        o, size = tables["gpio_pin_lut"][:2]
        print("-- gpio_pin_lut (register dword index, bit shift, mask shift, id)")
        for e in range((size - 4) // 8):
            a = o + 4 + 8 * e
            reg, shift, mshift, gid = u32(b, a), b[a + 4], b[a + 5], b[a + 6]
            kind = "I2C line %d engine %d" % (gid & 0xF, (gid >> 4) & 7) if gid & 0x80 else "GPIO pin %d" % gid
            print("  %2d reg %#06x (BAR byte %#07x) bit %2d mask bit %2d id %02x %s" % (e, reg, reg * 4, shift, mshift, gid, kind))
    if "vram_usagebyfirmware" in tables:
        o, size, f, c = tables["vram_usagebyfirmware"]
        start, fw_kb, drv_kb = u32(b, o + 4), u16(b, o + 8), u16(b, o + 10)
        print("-- vram_usagebyfirmware %d.%d: start %#x KiB (flags in bits 31:30), by firmware %d KiB, by driver %d KiB" % (
            f, c, start, fw_kb, drv_kb))
    if "integratedsysteminfo" in tables:
        o, size, f, c = tables["integratedsysteminfo"]
        print("-- integratedsysteminfo %d.%d: vbios_misc %08x gpucapinfo %08x system_config %08x" % (
            f, c, u32(b, o + 4), u32(b, o + 8), u32(b, o + 12)))
        print("  hdmi spread %d (0.001 %%) at %d x10 Hz, dvi spread %d at %d x10 Hz, dpphy_override %04x" % (
            u16(b, o + 28), u16(b, o + 30), u16(b, o + 32), u16(b, o + 34), u16(b, o + 36)))
        x = o + 56  # extdispconninfo follows the 56 bytes up to reserved2
        print("  extdispconninfo %d.%d, size %d" % (b[x + 2], b[x + 3], u16(b, x)))
        for p in range(7):
            a = x + 4 + 16 + 16 * p
            tag, acpi, conn, ddc, hpd, ext, chmap, inv, caps = struct.unpack_from("<HHHBBHBBH", b, a)
            if conn:
                print("  ext path %d: device tag %04x acpi %04x connector %s aux/ddc lut %d hpd lut %d ext %04x channel map %02x invert %02x caps %04x" % (
                    p, tag, acpi, object_name(conn), ddc, hpd, ext, chmap, inv, caps))
        t = x + 4 + 16 + 16 * 7
        print("  checksum %02x stereo pin %02x" % (b[t], b[t + 1]))
        if (f, c) == (1, 12):
            tune = x + 140  # sizeof(atom_external_display_connection_info)
            for name, a in (("TMDS", tune), ("hdmiCLK5", tune + 12), ("hdmiCLK8", tune + 24)):
                print("  %s tuning: max symclk %d x10 kHz mode %d phy_sel %02x eq main %d pre %d post %d vboost %d" % (
                    name, u32(b, a), b[a + 4], b[a + 5], b[a + 6], b[a + 7], b[a + 8], b[a + 10]))


if __name__ == "__main__":
    main(sys.argv[1])
