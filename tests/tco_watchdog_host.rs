// Intel's TCO watchdog (000-KRN-0076, 801-KRN-0258) against a model of its registers: the N100's Alder Lake-N PCH, its
// TCO behind the SMBus controller, and q35's ICH9 behind the LPC bridge; a strap or firmware that holds NO_REBOOT, a
// decode left off, TCO_EN locked, chipsets the code does not know, and the feed and the stop. Then the PCH's
// over-clocking watchdog (801-KRN-0259): off, after its reset, locked, silent, and its feed, longest count and stop.
#[allow(dead_code)]
#[path = "../kernel/src/arch/x86_64/tco.rs"]
mod tco;

use tco::*;

const ADL_N_ESPI: u32 = 0x5481 << 16 | INTEL;
const ADL_N_SMBUS: u32 = 0x54A3 << 16 | INTEL;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Write { Config(u8, u8, u8, u32), Io(u16, u32), Gcs(u32) }

struct Model {
    lpc: u32,
    class: u32,
    pmbase: u32,
    acpi_cntl: u32,
    rcba: u32,
    smbus: u32,
    tcobase: u32,
    tcoctl: u32,
    decode_sticks: bool,
    answers: bool,
    cnt1: u32,
    sts1: u32,
    sts2: u32,
    tmr: u32,
    count: u32,
    strap: bool,
    gcs: u32,
    gcs_sticks: bool,
    pm: Option<u16>,
    smi_en: u32,
    smi_locked: bool,
    log: Vec<Write>,
}

impl Model {
    // The DEXP as its hardware report showed it (log:/hw0004.txt, 2026-11-18): Intel's eSPI bridge 8086:5481 without
    // the ACPI I/O of the older chipsets, the SMBus controller 8086:54A3 with TCOBASE 0x401 and TCOCTL 0x101 (decoded,
    // locked), the FADT's PM1a events at 0x1800. The TCO's own registers are not in the report: here halted, NO_REBOOT
    // set (its strap low), TCO_EN on in SMI_EN.
    fn n100() -> Self {
        Model { lpc: ADL_N_ESPI, class: ISA_BRIDGE, pmbase: 0, acpi_cntl: 0, rcba: 0, smbus: ADL_N_SMBUS, tcobase: 0x401,
                tcoctl: TCO_BASE_EN | 1, decode_sticks: true, answers: true, cnt1: TMR_HLT | NR_MSUS | NMI_NOW, sts1: 0,
                sts2: 0, tmr: 4, count: 4, strap: false, gcs: 0, gcs_sticks: true, pm: Some(0x1800), smi_en: TCO_EN | 1,
                smi_locked: false, log: Vec::new() }
    }

    // QEMU's q35: ICH9's LPC bridge, its ACPI I/O at 0x600 and on, the chipset configuration unmapped by OVMF.
    fn q35() -> Self {
        Model { lpc: QEMU_ICH9 << 16 | INTEL, class: ISA_BRIDGE, pmbase: 0x601, acpi_cntl: ACPI_EN, rcba: 0,
                smbus: 0xFFFF_FFFF, gcs: GCS_NO_REBOOT, pm: None, ..Self::n100() }
    }

    // The TCO behind the SMBus controller, as the PCHs the model plays have it.
    fn pch(&self) -> bool { self.smbus == ADL_N_SMBUS || self.smbus == 0x7AA3 << 16 | INTEL }

    fn tco(&self) -> Option<u16> {
        if self.pch() {
            (self.tcoctl & TCO_BASE_EN != 0 && self.answers).then_some((self.tcobase & TCOBA) as u16)
        } else {
            (self.acpi_cntl & ACPI_EN != 0).then_some((self.pmbase & 0xFF80) as u16 + 0x60)
        }
    }

    fn smi_port(&self) -> Option<u16> {
        if self.pch() { self.pm.map(|p| p + SMI_EN) } else { Some((self.pmbase & 0xFF80) as u16 + SMI_EN) }
    }

    fn io_writes(&self, offset: u16) -> Vec<u32> {
        let tco = self.tco().unwrap_or((self.tcobase & TCOBA) as u16);
        self.log.iter().filter_map(|w| match w { Write::Io(port, value) if *port == tco + offset => Some(*value), _ => None }).collect()
    }

    fn position(&self, wanted: impl Fn(&Write) -> bool) -> usize { self.log.iter().position(wanted).expect("written") }
}

impl Registers for Model {
    fn config(&mut self, device: u8, function: u8, offset: u8) -> u32 {
        match (device, function, offset) {
            (31, 0, 0) => self.lpc,
            (31, 0, 8) => self.class << 8 | 0x10,
            (31, 0, PMBASE) => self.pmbase,
            (31, 0, ACPI_CNTL) => self.acpi_cntl,
            (31, 0, RCBA) => self.rcba,
            (31, 4, 0) => self.smbus,
            (31, 4, TCOBASE) => self.tcobase,
            (31, 4, TCOCTL) => self.tcoctl,
            _ => 0xFFFF_FFFF,
        }
    }
    fn set_config(&mut self, device: u8, function: u8, offset: u8, value: u32) {
        self.log.push(Write::Config(device, function, offset, value));
        match (device, function, offset) {
            (31, 0, RCBA) => self.rcba = value,
            (31, 4, TCOCTL) if self.decode_sticks => self.tcoctl = value,
            (31, 4, TCOCTL) => {}
            other => panic!("a write to {other:x?}"),
        }
    }
    fn io(&mut self, port: u16, width: u8) -> u32 {
        if Some(port) == self.smi_port() { assert_eq!(width, 4); return self.smi_en; }
        let Some(tco) = self.tco() else { return if width == 2 { 0xFFFF } else { 0xFFFF_FFFF } };
        assert_eq!(width, 2, "{port:#x}");
        match port.wrapping_sub(tco) {
            RLD => self.count,
            STS1 => self.sts1,
            STS2 => self.sts2,
            CNT1 => self.cnt1 | if self.strap { NR_MSUS } else { 0 },
            TMR => self.tmr,
            _ => 0xFFFF,
        }
    }
    fn set_io(&mut self, port: u16, width: u8, value: u32) {
        self.log.push(Write::Io(port, value));
        if Some(port) == self.smi_port() {
            self.smi_en = if self.smi_locked { value & !TCO_EN | self.smi_en & TCO_EN } else { value };
            return;
        }
        let tco = self.tco().expect("a write to TCO registers that do not decode");
        assert_eq!(width, 2);
        match port - tco {
            RLD => self.count = self.tmr,
            STS1 => self.sts1 &= !value,
            STS2 => self.sts2 &= !value,
            CNT1 => {
                assert_eq!(value & NMI_NOW, 0, "NMI_NOW written back: {value:#x}");
                self.cnt1 = value | self.cnt1 & NMI_NOW;
            }
            TMR => self.tmr = value,
            other => panic!("a write to TCO + {other:#x}"),
        }
    }
    fn gcs(&mut self, address: usize) -> u32 {
        assert_eq!(address, (RCBA_DEFAULT as usize) + GCS);
        self.gcs
    }
    fn set_gcs(&mut self, address: usize, value: u32) {
        assert_eq!(address, (RCBA_DEFAULT as usize) + GCS);
        self.log.push(Write::Gcs(value));
        if self.gcs_sticks { self.gcs = value; }
    }
    fn pm_base(&mut self) -> Option<u16> { self.pm }
}

#[test]
fn the_n100s_tco_arms_behind_its_smbus_controller() {
    let mut m = Model::n100();
    let armed = arm(&mut m, 25).expect("armed");
    assert_eq!(armed, Armed { base: 0x400, kind: Kind::Smbus(0x54A3, "ALDER LAKE-M AND -N"), restarted: false, fresh: true,
                              decode_on: false, smi_kept: false });
    assert_eq!(m.cnt1 & (TMR_HLT | NR_MSUS), 0, "counting, a reset at the second expiry: {:#x}", m.cnt1);
    assert_eq!((m.tmr, m.count), (25, 25), "25 ticks of 0.6 s, reloaded");
    assert_eq!(m.smi_en, 1, "TCO_EN off, the rest of SMI_EN as it was");
    // TCO2_STS on these PCHs has no BOOT_STS: only SECOND_TO_STS is written.
    assert_eq!(m.io_writes(STS2), vec![SECOND_TO]);
    // The count reloaded before NO_REBOOT clears, the timer started last.
    let reload = m.position(|w| *w == Write::Io(0x400 + RLD, 1));
    let no_reboot = m.position(|w| matches!(w, Write::Io(0x408, v) if v & NR_MSUS == 0));
    let start = m.position(|w| matches!(w, Write::Io(0x408, v) if v & TMR_HLT == 0));
    assert!(reload < no_reboot && no_reboot <= start, "{:x?}", m.log);
    assert!(!m.log.iter().any(|w| matches!(w, Write::Config(..))), "nothing written to configuration space: {:x?}", m.log);
}

#[test]
fn a_reset_by_the_timer_is_read_and_cleared() {
    let mut m = Model { sts2: SECOND_TO, sts1: TIMEOUT, tmr: 25, ..Model::n100() };
    let armed = arm(&mut m, 25).expect("armed");
    assert!(armed.restarted && !armed.fresh, "{armed:?}");
    assert_eq!((m.sts1, m.sts2), (0, 0));
}

#[test]
fn a_no_reboot_strap_leaves_the_timer_halted() {
    let mut m = Model { strap: true, ..Model::n100() };
    assert_eq!(arm(&mut m, 25), Err(Refused::Why("INTEL PCH: NO_REBOOT HELD BY THE BOARD'S STRAP")));
    assert_ne!(m.cnt1 & TMR_HLT, 0, "still halted");
    assert_eq!(m.tcoctl, TCO_BASE_EN | 1, "the decode as the firmware left it");
}

#[test]
fn a_decode_left_off_is_turned_on_where_the_firmware_set_a_base() {
    let mut m = Model { tcoctl: 0, ..Model::n100() };
    let armed = arm(&mut m, 25).expect("armed");
    assert!(armed.decode_on, "{armed:?}");
    assert_eq!(m.tcoctl, TCO_BASE_EN);

    let mut m = Model { tcoctl: 0, tcobase: 1, ..Model::n100() };
    assert_eq!(arm(&mut m, 25), Err(Refused::Why("INTEL PCH: THE FIRMWARE SET NO TCO BASE")));
    assert!(m.log.is_empty(), "nothing written: {:x?}", m.log);

    let mut m = Model { tcoctl: 0, decode_sticks: false, ..Model::n100() };
    assert_eq!(arm(&mut m, 25), Err(Refused::Why("INTEL PCH: ITS TCO DECODE STAYS OFF")));
}

#[test]
fn registers_that_do_not_answer_are_refused_and_the_decode_put_back() {
    let mut m = Model { tcoctl: 0, answers: false, ..Model::n100() };
    assert_eq!(arm(&mut m, 25), Err(Refused::Why("INTEL PCH: ITS TCO REGISTERS DO NOT ANSWER")));
    assert_eq!(m.tcoctl, 0, "the decode turned off again: {:x?}", m.log);
}

#[test]
fn tco_en_locked_or_no_pm_base_keeps_the_firmwares_smi() {
    let mut m = Model { smi_locked: true, ..Model::n100() };
    assert!(arm(&mut m, 25).expect("armed").smi_kept);
    let mut m = Model { pm: None, ..Model::n100() };
    let armed = arm(&mut m, 25).expect("armed");
    assert!(armed.smi_kept && m.cnt1 & TMR_HLT == 0, "armed all the same: {armed:?}");
}

#[test]
fn chipsets_not_known_here_are_named_and_left_alone() {
    let mut m = Model { smbus: 0x1234 << 16 | INTEL, ..Model::n100() };
    assert_eq!(arm(&mut m, 25), Err(Refused::Unknown(0x1234)));
    let mut m = Model { smbus: 0xA123 << 16 | INTEL, ..Model::n100() };
    assert_eq!(arm(&mut m, 25), Err(Refused::BehindP2sb(0xA123)));
    let mut m = Model { lpc: 0x790E << 16 | 0x1022, ..Model::n100() };
    assert_eq!(arm(&mut m, 25), Err(Refused::NotIntel));
    let mut m = Model { smbus: 0xFFFF_FFFF, ..Model::n100() };
    assert_eq!(arm(&mut m, 25), Err(Refused::Why("THE LPC BRIDGE'S ACPI I/O IS OFF")));
    // A later PCH known by its SMBus ID: the same path.
    let mut m = Model { smbus: 0x7AA3 << 16 | INTEL, ..Model::n100() };
    assert_eq!(arm(&mut m, 25).expect("armed").kind, Kind::Smbus(0x7AA3, "ALDER LAKE-S"));
}

#[test]
fn q35s_ich9_arms_behind_its_lpc_bridge() {
    let mut m = Model::q35();
    let armed = arm(&mut m, 25).expect("armed");
    assert_eq!(armed, Armed { base: 0x660, kind: Kind::Lpc, restarted: false, fresh: true, decode_on: false, smi_kept: false });
    assert_eq!(m.rcba, RCBA_DEFAULT | 1, "OVMF's unmapped chipset configuration mapped at its default");
    assert_eq!(m.gcs & GCS_NO_REBOOT, 0);
    assert_eq!(m.cnt1 & TMR_HLT, 0);
    assert_eq!(m.smi_en, 1);
    assert_eq!(m.io_writes(STS2), vec![SECOND_TO | BOOT_STS], "ICH9's TCO2_STS has BOOT_STS");
}

#[test]
fn firmware_that_holds_no_reboot_in_gcs_leaves_the_timer_halted() {
    // The MacBook Pro's 7-series PCH (801-KRN-0258).
    let mut m = Model { gcs_sticks: false, lpc: 0x1E57 << 16 | INTEL, rcba: 0xFED1_C001, ..Model::q35() };
    assert_eq!(arm(&mut m, 25), Err(Refused::Why("NO_REBOOT HELD BY THE BOARD'S STRAP OR ITS FIRMWARE")));
    assert_ne!(m.cnt1 & TMR_HLT, 0);
    let mut m = Model { lpc: 0x1E57 << 16 | INTEL, ..Model::q35() };
    assert_eq!(arm(&mut m, 25), Err(Refused::Why("NO CHIPSET CONFIGURATION: THIS PCH KEEPS NO_REBOOT ELSEWHERE")));
}

#[test]
fn the_feed_reloads_and_the_stop_halts() {
    let mut m = Model::n100();
    let armed = arm(&mut m, 25).expect("armed");
    m.count = 3;
    feed(&mut m, armed.base);
    assert_eq!(m.count, 25);
    stop(&mut m, armed.base);
    assert_ne!(m.cnt1 & TMR_HLT, 0);
    assert_eq!(m.cnt1 & NR_MSUS, 0, "NO_REBOOT left clear");
}

#[test]
fn the_n100_is_armed_before_the_page_tables_and_its_smi_quieted_after() {
    // As the kernel starts (801-KRN-0258): listed, armed without the FADT; then, the FADT read, TCO_EN cleared.
    let mut m = Model { pm: None, ..Model::n100() };
    assert_eq!(listed(&mut m), Some((0x54A3, "ALDER LAKE-M AND -N")));
    let armed = arm(&mut m, 25).expect("armed");
    assert!(armed.smi_kept && m.cnt1 & TMR_HLT == 0, "{armed:?}");
    assert!(!m.log.iter().any(|w| matches!(w, Write::Gcs(_))), "no MMIO before the page tables: {:x?}", m.log);
    m.pm = Some(0x1800);
    assert!(!no_smi(&mut m, Some(0x1800)));
    assert_eq!(m.smi_en, 1);
    // The older chipsets and AMD's wait for the page tables.
    assert_eq!(listed(&mut Model::q35()), None);
    assert_eq!(listed(&mut Model { lpc: 0x790E << 16 | 0x1022, ..Model::n100() }), None);
    assert_eq!(listed(&mut Model { smbus: 0xA123 << 16 | INTEL, ..Model::n100() }), None);
}

// OC_WDT_CTL as Intel's SoC I/O registers describe it: the status bits cleared by a 1, the RW/L fields frozen by LCK,
// RLD reloading only while EN is set; the reserved bits (30:26, 13, 11:10) read 0.
struct Oc { ctl: u32, count: u32, sticks: bool, answers: bool, log: Vec<u32> }

const OC_PORT: u16 = 0x1854; // the DEXP's: PM base 0x1800 in its FADT, CWDT's _CRS in its DSDT
const OC_RW: u32 = 0xFF << 16 | OC_FORCE_ALL | OC_EN | OC_LCK | OC_TOV;

impl Oc {
    fn new(ctl: u32) -> Self { Oc { ctl, count: 0, sticks: true, answers: true, log: Vec::new() } }
}

impl Registers for Oc {
    fn config(&mut self, _: u8, _: u8, _: u8) -> u32 { panic!("no configuration space for the OC watchdog") }
    fn set_config(&mut self, _: u8, _: u8, _: u8, _: u32) { panic!("no configuration space for the OC watchdog") }
    fn io(&mut self, port: u16, width: u8) -> u32 {
        assert_eq!((port, width), (OC_PORT, 4));
        if self.answers { self.ctl } else { u32::MAX }
    }
    fn set_io(&mut self, port: u16, width: u8, value: u32) {
        assert_eq!((port, width), (OC_PORT, 4));
        self.log.push(value);
        self.ctl &= !(value & OC_STS);
        if self.sticks && self.ctl & OC_LCK == 0 { self.ctl = self.ctl & !OC_RW | value & OC_RW; }
        if value & OC_RLD != 0 && self.ctl & OC_EN != 0 { self.count = (self.ctl & OC_TOV) + 1; }
    }
    fn gcs(&mut self, _: usize) -> u32 { panic!("no GCS") }
    fn set_gcs(&mut self, _: usize, _: u32) { panic!("no GCS") }
    fn pm_base(&mut self) -> Option<u16> { Some(0x1800) }
}

#[test]
fn the_dexps_over_clocking_watchdog_arms_at_30_s() {
    let mut m = Oc::new(0);
    let armed = arm_oc(&mut m, 0x1800, 30).expect("armed");
    assert_eq!(armed, OcArmed { port: OC_PORT, found: 0, seconds: 30, restarted: false, locked: false });
    assert_eq!(m.ctl, OC_EN | 29);
    assert_eq!(m.count, 30);
}

#[test]
fn an_expirys_reset_is_read_and_cleared_and_the_firmwares_scratch_kept() {
    let found = OC_STS & !(1 << 25) | 0x40 << 16 | OC_FORCE_ALL;
    let mut m = Oc::new(found);
    let armed = arm_oc(&mut m, 0x1800, 30).expect("armed");
    assert!(armed.restarted && armed.found == found, "{armed:x?}");
    assert_eq!(m.ctl, 0x40 << 16 | OC_EN | 29, "status cleared, FORCE_ALL off, the other scratch kept");
    assert_eq!(m.log.iter().filter(|w| *w & OC_STS != 0).count(), 1, "the status cleared once: {:x?}", m.log);
}

#[test]
fn the_dexps_firmware_mark_after_an_expiry_is_read_and_cleared() {
    // The DEXP after its over-clocking watchdog reset a stopped trial (801-KRN-0259): status clear, scratch bit 23 set.
    let mut m = Oc::new(0x80_2000);
    let armed = arm_oc(&mut m, 0x1800, 30).expect("armed");
    assert!(armed.restarted, "{armed:x?}");
    assert_eq!(m.ctl & (OC_MARK | OC_EN | OC_TOV), OC_EN | 29, "the mark cleared: the next run is not taken for a restart");
    assert!(!arm_oc(&mut Oc::new(0x2000), 0x1800, 30).expect("armed").restarted, "after an ordinary reset");
}

#[test]
fn a_locked_running_one_is_fed_at_its_own_timeout() {
    let found = OC_LCK | OC_EN | 59;
    let mut m = Oc::new(found);
    let armed = arm_oc(&mut m, 0x1800, 30).expect("armed");
    assert!(armed.locked && armed.seconds == 60, "{armed:x?}");
    assert_eq!((m.ctl, m.count), (found, 60));
}

#[test]
fn a_locked_one_left_off_or_one_that_does_not_answer_is_refused_untouched() {
    let mut m = Oc::new(OC_LCK);
    assert_eq!(arm_oc(&mut m, 0x1800, 30), Err("ITS OVER-CLOCKING WATCHDOG IS OFF AND LOCKED BY THE FIRMWARE"));
    let mut silent = Oc { answers: false, ..Oc::new(0) };
    assert_eq!(arm_oc(&mut silent, 0x1800, 30), Err("ITS OVER-CLOCKING WATCHDOG DOES NOT ANSWER"));
    assert!(m.log.is_empty() && silent.log.is_empty(), "{:x?} {:x?}", m.log, silent.log);
    let mut stuck = Oc { sticks: false, ..Oc::new(0) };
    assert_eq!(arm_oc(&mut stuck, 0x1800, 30), Err("ITS OVER-CLOCKING WATCHDOG DOES NOT START"));
}

#[test]
fn the_oc_feed_reloads_a_reset_gets_the_longest_count_and_the_stop_stops() {
    let mut m = Oc::new(0);
    arm_oc(&mut m, 0x1800, 30).expect("armed");
    m.count = 3;
    m.ctl |= 1 << 24; // a status the feed must not clear unread
    feed_oc(&mut m, OC_PORT);
    assert_eq!(m.count, 30);
    assert_ne!(m.ctl & 1 << 24, 0);
    long_oc(&mut m, OC_PORT);
    assert_eq!((m.ctl & (OC_EN | OC_TOV), m.count), (OC_EN | OC_TOV, 1024));
    m.count = 7;
    stop_oc(&mut m, OC_PORT);
    assert_eq!((m.ctl & OC_EN, m.count), (0, 7), "stopped, not reloaded");
    assert!(m.log[3..].iter().all(|w| w & OC_STS == 0), "{:x?}", m.log);
}
