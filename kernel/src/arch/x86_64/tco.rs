// Intel's TCO timer (000-KRN-0076, 801-KRN-0258), kept apart from the hardware so tests/tco_watchdog_host.rs runs it
// against a model of the registers. Two kinds of chipset hold it:
// - ICH9 to the 7-series PCH (and QEMU's q35): the LPC bridge 00:1F.0 decodes the ACPI I/O at PMBASE (0x40), on while
//   ACPI_CNTL (0x44) bit 7 is set; the TCO registers are at PMBASE + 0x60, NO_REBOOT is bit 5 of GCS in the chipset
//   configuration (RCBA, 0xF0, + 0x3410);
// - the PCHs from Cannon Lake's on (the 300 series and later, the N100's Alder Lake-N among them): the SMBus controller
//   00:1F.4 holds TCOBASE (0x50, the base in bits 15:5) and TCOCTL (0x54, TCO_BASE_EN bit 8), and NO_REBOOT is
//   TCO1_CNT's bit 0 (NR_MSUS), which software may clear unless the board's strap holds it. Names, offsets and bits as
//   Intel's register database for the 400 Series Chipset On-Package PCH (615146) gives them: SMBus PCI configuration,
//   SMBus TCO I/O.
// The same PCHs have an over-clocking watchdog, OC_WDT_CTL at the PM I/O base + 0x54 (the same database, PMC I/O): its
// expiry resets the platform with no SMI in the way, where the firmware keeps the TCO's first expiry (801-KRN-0259).
// The timer itself is the same in both: TCO_RLD 0x00 (a write reloads it), TCO1_STS 0x04 (TIMEOUT bit 3), TCO2_STS 0x06
// (SECOND_TO_STS bit 1), TCO1_CNT 0x08 (TCO_TMR_HALT bit 11), TCO_TMR 0x12 (ticks of about 0.6 s); with NO_REBOOT clear,
// the second expiry without a reload resets the machine (PLTRST#).

pub const INTEL: u32 = 0x8086;
pub const ISA_BRIDGE: u32 = 0x06_0100; // 00:1F.0's class code
pub const LPC: (u8, u8) = (31, 0);
pub const SMBUS: (u8, u8) = (31, 4);
pub const PMBASE: u8 = 0x40;
pub const ACPI_CNTL: u8 = 0x44;
pub const ACPI_EN: u32 = 1 << 7;
pub const RCBA: u8 = 0xF0;
pub const GCS: usize = 0x3410;
pub const GCS_NO_REBOOT: u32 = 1 << 5;
pub const QEMU_ICH9: u32 = 0x2918; // q35's LPC bridge: OVMF leaves the chipset configuration unmapped
pub const RCBA_DEFAULT: u32 = 0xFED1_C000;
pub const TCOBASE: u8 = 0x50;
pub const TCOBA: u32 = 0xFFE0; // bits 15:5
pub const TCOCTL: u8 = 0x54;
pub const TCO_BASE_EN: u32 = 1 << 8;
pub const RLD: u16 = 0x00;
pub const STS1: u16 = 0x04;
pub const STS2: u16 = 0x06;
pub const CNT1: u16 = 0x08;
pub const TMR: u16 = 0x12;
pub const TIMEOUT: u32 = 1 << 3;
pub const SECOND_TO: u32 = 1 << 1;
pub const BOOT_STS: u32 = 1 << 2; // ICH9's; reserved on the later PCHs
pub const NR_MSUS: u32 = 1 << 0; // NO_REBOOT in TCO1_CNT, the later PCHs'
pub const NMI_NOW: u32 = 1 << 8; // never written back as read
pub const TMR_HLT: u32 = 1 << 11;
pub const SMI_EN: u16 = 0x30; // in the ACPI PM I/O space
pub const TCO_EN: u32 = 1 << 13;

/// The SMBus controllers (00:1F.4) of the PCHs that keep NO_REBOOT in TCO1_CNT, by device ID, as Linux's i2c-i801
/// lists them with Cannon Lake's TCO.
pub const PCH_SMBUS: [(u16, &str); 24] = [
    (0xA323, "CANNON LAKE-H"), (0x9DA3, "CANNON LAKE-LP"), (0x18DF, "CEDAR FORK"), (0x1BC9, "EMMITSBURG"),
    (0x34A3, "ICE LAKE-LP"), (0x38A3, "ICE LAKE-N"), (0x02A3, "COMET LAKE"), (0x06A3, "COMET LAKE-H"),
    (0x4B23, "ELKHART LAKE"), (0xA0A3, "TIGER LAKE-LP"), (0x43A3, "TIGER LAKE-H"), (0x4DA3, "JASPER LAKE"),
    (0x7AA3, "ALDER LAKE-S"), (0x51A3, "ALDER LAKE-P"), (0x54A3, "ALDER LAKE-M AND -N"), (0x7A23, "RAPTOR LAKE-S"),
    (0x7E22, "METEOR LAKE-P"), (0xAE22, "METEOR LAKE SOC-S"), (0x7F23, "METEOR LAKE PCH-S"), (0x7722, "ARROW LAKE-H"),
    (0xE322, "PANTHER LAKE-H"), (0xE422, "PANTHER LAKE-P"), (0x4D22, "WILDCAT LAKE-U"), (0x6E23, "NOVA LAKE-S"),
];
/// Sunrise Point's kind (the 100 and 200 series, Lewisburg, Denverton, Comet Lake-V): NO_REBOOT is in the SMBus
/// controller's private registers behind the P2SB, not done here.
pub const P2SB_SMBUS: [u16; 7] = [0xA123, 0x9D23, 0xA2A3, 0xA1A3, 0xA223, 0x19DF, 0xA3A3];

/// The chipset as the watchdog reaches it: bus 0's configuration space, I/O ports, the chipset configuration's GCS and
/// the ACPI PM I/O base the FADT names.
pub trait Registers {
    fn config(&mut self, device: u8, function: u8, offset: u8) -> u32;
    fn set_config(&mut self, device: u8, function: u8, offset: u8, value: u32);
    /// A word (`width` 2) or a dword (4).
    fn io(&mut self, port: u16, width: u8) -> u32;
    fn set_io(&mut self, port: u16, width: u8, value: u32);
    fn gcs(&mut self, address: usize) -> u32;
    fn set_gcs(&mut self, address: usize, value: u32);
    /// The FADT's PM1a event block, where Intel's PCHs start their ACPI PM I/O space.
    fn pm_base(&mut self) -> Option<u16>;
}

/// Where the timer was found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Lpc,
    /// The SMBus controller's device ID and the PCH it names.
    Smbus(u16, &'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Armed {
    pub base: u16,
    pub kind: Kind,
    /// The last run ended at the second expiry: its status outlives the reset.
    pub restarted: bool,
    /// No run armed it since the machine was powered on (QEMU keeps the count across resets).
    pub fresh: bool,
    /// TCOCTL's decode was off with a base set, and turned on here.
    pub decode_on: bool,
    /// The first expiry still raises the firmware's SMI (TCO_EN locked, or no PM base known).
    pub smi_kept: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refused {
    /// No Intel ISA bridge at 00:1F.0: the AMD FCH may have a timer.
    NotIntel,
    /// An Intel SMBus controller at 00:1F.4 this code does not know.
    Unknown(u16),
    /// A Sunrise Point kind of PCH (the 100 and 200 series): NO_REBOOT is behind the P2SB.
    BehindP2sb(u16),
    Why(&'static str),
}

/// The PCH listed by its SMBus controller at 00:1F.4, whose TCO needs only configuration space and I/O ports, so it can
/// be armed before the kernel's page tables (801-KRN-0258).
pub fn listed(r: &mut impl Registers) -> Option<(u16, &'static str)> {
    let lpc = r.config(LPC.0, LPC.1, 0);
    if lpc & 0xFFFF != INTEL || r.config(LPC.0, LPC.1, 8) >> 8 != ISA_BRIDGE { return None; }
    let smbus = r.config(SMBUS.0, SMBUS.1, 0);
    if smbus & 0xFFFF != INTEL { return None; }
    PCH_SMBUS.iter().find(|(known, _)| *known as u32 == smbus >> 16).copied()
}

/// Finds the timer, sets it to reset the machine at its second expiry, `ticks` of 0.6 s after the last reload each, and
/// starts it; else why not. A refusal after a register changed puts it back, or leaves the timer halted.
pub fn arm(r: &mut impl Registers, ticks: u16) -> Result<Armed, Refused> {
    let lpc = r.config(LPC.0, LPC.1, 0);
    if lpc & 0xFFFF != INTEL || r.config(LPC.0, LPC.1, 8) >> 8 != ISA_BRIDGE { return Err(Refused::NotIntel); }
    let smbus = r.config(SMBUS.0, SMBUS.1, 0);
    let id = (smbus >> 16) as u16;
    if smbus & 0xFFFF == INTEL {
        if let Some((_, name)) = PCH_SMBUS.iter().find(|(known, _)| *known == id) { return pch(r, ticks, id, name); }
        if P2SB_SMBUS.contains(&id) { return Err(Refused::BehindP2sb(id)); }
    }
    let pmbase = (r.config(LPC.0, LPC.1, PMBASE) & 0xFF80) as u16;
    if pmbase == 0 || r.config(LPC.0, LPC.1, ACPI_CNTL) & ACPI_EN == 0 {
        return Err(if smbus & 0xFFFF == INTEL { Refused::Unknown(id) } else { Refused::Why("THE LPC BRIDGE'S ACPI I/O IS OFF") });
    }
    ich(r, ticks, lpc >> 16, pmbase)
}

fn ich(r: &mut impl Registers, ticks: u16, lpc: u32, pmbase: u16) -> Result<Armed, Refused> {
    let tco = pmbase + 0x60;
    let mut rcba = r.config(LPC.0, LPC.1, RCBA);
    if rcba & 1 == 0 && lpc == QEMU_ICH9 { r.set_config(LPC.0, LPC.1, RCBA, RCBA_DEFAULT | 1); rcba = r.config(LPC.0, LPC.1, RCBA); }
    if rcba & 1 == 0 { return Err(Refused::Why("NO CHIPSET CONFIGURATION: THIS PCH KEEPS NO_REBOOT ELSEWHERE")); }
    let (restarted, fresh) = load(r, tco, ticks, SECOND_TO | BOOT_STS);
    let gcs = (rcba & 0xFFFF_C000) as usize + GCS;
    let value = r.gcs(gcs);
    if value & GCS_NO_REBOOT != 0 { r.set_gcs(gcs, value & !GCS_NO_REBOOT); }
    if r.gcs(gcs) & GCS_NO_REBOOT != 0 { return Err(Refused::Why("NO_REBOOT HELD BY THE BOARD'S STRAP OR ITS FIRMWARE")); }
    let smi_kept = no_smi(r, Some(pmbase));
    run(r, tco)?;
    Ok(Armed { base: tco, kind: Kind::Lpc, restarted, fresh, decode_on: false, smi_kept })
}

fn pch(r: &mut impl Registers, ticks: u16, id: u16, name: &'static str) -> Result<Armed, Refused> {
    let tco = (r.config(SMBUS.0, SMBUS.1, TCOBASE) & TCOBA) as u16;
    if tco == 0 { return Err(Refused::Why("INTEL PCH: THE FIRMWARE SET NO TCO BASE")); }
    let control = r.config(SMBUS.0, SMBUS.1, TCOCTL);
    let decode_on = control & TCO_BASE_EN == 0;
    if decode_on { r.set_config(SMBUS.0, SMBUS.1, TCOCTL, control | TCO_BASE_EN); }
    // A refusal turns off again a decode it turned on.
    let refuse = |r: &mut dyn Registers, why| { if decode_on { r.set_config(SMBUS.0, SMBUS.1, TCOCTL, control); } Err(Refused::Why(why)) };
    if r.config(SMBUS.0, SMBUS.1, TCOCTL) & TCO_BASE_EN == 0 { return Err(Refused::Why("INTEL PCH: ITS TCO DECODE STAYS OFF")); }
    if r.io(tco + CNT1, 2) == 0xFFFF { return refuse(r, "INTEL PCH: ITS TCO REGISTERS DO NOT ANSWER"); }
    let (restarted, fresh) = load(r, tco, ticks, SECOND_TO);
    let control1 = r.io(tco + CNT1, 2) & !NMI_NOW;
    if control1 & NR_MSUS != 0 { r.set_io(tco + CNT1, 2, control1 & !NR_MSUS); }
    if r.io(tco + CNT1, 2) & NR_MSUS != 0 { return refuse(r, "INTEL PCH: NO_REBOOT HELD BY THE BOARD'S STRAP"); }
    let pm = r.pm_base();
    let smi_kept = no_smi(r, pm);
    run(r, tco)?;
    Ok(Armed { base: tco, kind: Kind::Smbus(id, name), restarted, fresh, decode_on, smi_kept })
}

// The count set and reloaded before NO_REBOOT clears, so a timer the firmware left running cannot expire on an old count;
// then the last run's status read and cleared (`clear`: the TCO2_STS bits this chipset has).
fn load(r: &mut impl Registers, tco: u16, ticks: u16, clear: u32) -> (bool, bool) {
    let fresh = r.io(tco + TMR, 2) != ticks as u32;
    r.set_io(tco + TMR, 2, ticks as u32);
    r.set_io(tco + RLD, 2, 1);
    let restarted = r.io(tco + STS2, 2) & SECOND_TO != 0;
    r.set_io(tco + STS2, 2, clear);
    r.set_io(tco + STS1, 2, TIMEOUT);
    (restarted, fresh)
}

/// No SMI at the first expiry: no handler of the kernel's would take it, and the firmware's might reload the timer. True
/// when it stays on: TCO1_CNT's TCO_LOCK (bit 12) keeps TCO_EN as the firmware set it, or no PM base is known.
pub fn no_smi(r: &mut impl Registers, pm: Option<u16>) -> bool {
    let Some(pm) = pm else { return true };
    let smi = r.io(pm + SMI_EN, 4);
    if smi & TCO_EN != 0 { r.set_io(pm + SMI_EN, 4, smi & !TCO_EN); }
    r.io(pm + SMI_EN, 4) & TCO_EN != 0
}

fn run(r: &mut impl Registers, tco: u16) -> Result<(), Refused> {
    let control = r.io(tco + CNT1, 2) & !NMI_NOW;
    r.set_io(tco + CNT1, 2, control & !TMR_HLT);
    if r.io(tco + CNT1, 2) & TMR_HLT != 0 { return Err(Refused::Why("ITS TIMER STAYS HALTED")); }
    Ok(())
}

/// The count started again.
pub fn feed(r: &mut impl Registers, tco: u16) { r.set_io(tco + RLD, 2, 1); }

/// The timer halted.
pub fn stop(r: &mut impl Registers, tco: u16) {
    let control = r.io(tco + CNT1, 2) & !NMI_NOW;
    r.set_io(tco + CNT1, 2, control | TMR_HLT);
}

pub const OC_WDT_CTL: u16 = 0x54; // in the PM I/O space
pub const OC_RLD: u32 = 1 << 31; // a 1 written reloads the count
pub const OC_STS: u32 = 0b11 << 24; // ICCSURV_STS and NO_ICCSURV_STS: set by an expiry's reset, a 1 written clears them
pub const OC_MARK: u32 = 1 << 23; // a scratch bit: the DEXP's firmware sets it after an expiry and clears the status
pub const OC_FORCE_ALL: u32 = 1 << 15; // set: any global reset counts as an expiry
pub const OC_EN: u32 = 1 << 14;
pub const OC_LCK: u32 = 1 << 12;
pub const OC_TOV: u32 = 0x3FF; // seconds, less one

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OcArmed {
    pub port: u16,
    /// The register as the firmware left it.
    pub found: u32,
    pub seconds: u32,
    /// An expiry's reset ended the last run: its status, or the firmware's mark where it cleared that.
    pub restarted: bool,
    /// Locked by the firmware while running: fed at its own timeout, never stopped.
    pub locked: bool,
}

/// The over-clocking watchdog of a listed PCH whose PM I/O base is `pm`, set to reset the platform `seconds` after the
/// last feed and started; else why not. A locked one the firmware left running is fed as it is.
pub fn arm_oc(r: &mut impl Registers, pm: u16, seconds: u32) -> Result<OcArmed, &'static str> {
    let port = pm + OC_WDT_CTL;
    let found = r.io(port, 4);
    if found == u32::MAX { return Err("ITS OVER-CLOCKING WATCHDOG DOES NOT ANSWER"); }
    let mut armed = OcArmed { port, found, seconds, restarted: found & (OC_STS | OC_MARK) != 0, locked: found & OC_LCK != 0 };
    if armed.locked {
        if found & OC_EN == 0 { return Err("ITS OVER-CLOCKING WATCHDOG IS OFF AND LOCKED BY THE FIRMWARE"); }
        armed.seconds = (found & OC_TOV) + 1;
        feed_oc(r, port);
        return Ok(armed);
    }
    let kept = found & !(OC_RLD | OC_STS | OC_MARK | OC_FORCE_ALL | OC_EN | OC_TOV);
    r.set_io(port, 4, kept | found & OC_STS);
    r.set_io(port, 4, kept | OC_EN | (seconds - 1));
    r.set_io(port, 4, kept | OC_EN | (seconds - 1) | OC_RLD);
    if r.io(port, 4) & (OC_EN | OC_TOV) != OC_EN | (seconds - 1) { return Err("ITS OVER-CLOCKING WATCHDOG DOES NOT START"); }
    Ok(armed)
}

/// The over-clocking watchdog's count started again.
pub fn feed_oc(r: &mut impl Registers, port: u16) {
    let value = r.io(port, 4) & !OC_STS;
    r.set_io(port, 4, value | OC_RLD);
}

/// Its longest count, 1024 s, started: before a reset the kernel chose, so that one the firmware does not stop leaves
/// it and the loader time, and a reset that does not come still ends in one.
pub fn long_oc(r: &mut impl Registers, port: u16) {
    let value = r.io(port, 4) & !(OC_STS | OC_TOV);
    r.set_io(port, 4, value | OC_TOV | OC_RLD);
}

/// The over-clocking watchdog stopped, unless the firmware locked it.
pub fn stop_oc(r: &mut impl Registers, port: u16) {
    let value = r.io(port, 4) & !(OC_STS | OC_RLD);
    r.set_io(port, 4, value & !OC_EN);
}
