// The AMD FCH's watchdog (213-KRN-0125) on Zen's FCH (SMBus 1022:790B, revision 0x51 and later): its PM registers and
// its timer in the FCH's ACPI MMIO block, as AMD's PPR for families 17h and 19h names them (FCH::PM, FCH::WDT). Kept
// apart from the hardware, so tests/fch_watchdog_host.rs runs it against a model of the registers.

pub const SMBUS: u32 = 0x790B_1022; // 00:14.0's device << 16 | vendor
pub const FIRST_REVISION: u8 = 0x51; // Zen's FCH, with its PM registers in MMIO
pub const PM: u32 = 0xFED8_0300;
pub const DECODEEN: u8 = 0x00;
pub const WDT_TMREN: u8 = 1 << 7; // the timer decoded at TIMER_DECODED
pub const DECODEEN3: u8 = 0x03;
pub const WDT_DISABLE: u8 = 0b11 << 2;
pub const WDT_SECONDS: u8 = 0b11; // the count in seconds
pub const ISACONTROL: u8 = 0x04;
pub const MMIOEN: u8 = 1 << 1; // the ACPI MMIO block decoded, the timer at TIMER_IN_ACPI_MMIO
pub const TIMER_DECODED: u32 = 0xFEB0_0000;
pub const TIMER_IN_ACPI_MMIO: u32 = 0xFED8_0B00;
pub const CONTROL: u32 = 0x00;
pub const COUNT: u32 = 0x04;
pub const RUN: u32 = 1 << 0;
pub const FIRED: u32 = 1 << 1; // set by an expiry's reset, kept across it; a 1 written clears it
pub const POWER_OFF: u32 = 1 << 2; // the action at expiry; clear, a reset
pub const DISABLED: u32 = 1 << 3;
pub const TRIGGER: u32 = 1 << 7; // a 1 written starts the count again

/// The FCH as the watchdog reaches it: the SMBus function's identity, the PM registers' bytes and the timer's words.
pub trait Registers {
    fn smbus(&mut self) -> (u32, u8);
    fn pm(&mut self, offset: u8) -> u8;
    fn set_pm(&mut self, offset: u8, value: u8);
    fn timer(&mut self, address: u32) -> u32;
    fn set_timer(&mut self, address: u32, value: u32);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Armed {
    pub base: u32,
    pub restarted: bool,
}

/// Finds the timer, sets it to reset the machine `seconds` after the last feed and starts it; else why not. Nothing is
/// written to an FCH this code does not know, and a refusal after the PM registers changed puts them back.
pub fn arm(r: &mut impl Registers, seconds: u32) -> Result<Armed, &'static str> {
    let (id, revision) = r.smbus();
    if id != SMBUS { return Err("NO AMD FCH AT 00:14.0"); }
    if revision < FIRST_REVISION { return Err("AMD FCH: A REVISION BEFORE ZEN'S, NOT KNOWN HERE"); }
    let (decode, isa) = (r.pm(DECODEEN), r.pm(ISACONTROL));
    if decode == 0xFF && isa == 0xFF { return Err("AMD FCH: ITS PM REGISTERS DO NOT ANSWER"); }
    let base = if decode & WDT_TMREN != 0 { TIMER_DECODED } else if isa & MMIOEN != 0 { TIMER_IN_ACPI_MMIO } else {
        return Err("AMD FCH: ITS TIMER IS NOT DECODED");
    };
    let before = r.pm(DECODEEN3);
    let wanted = before & !WDT_DISABLE | WDT_SECONDS;
    if wanted != before { r.set_pm(DECODEEN3, wanted); }
    let control = r.timer(base + CONTROL);
    let failure = if control == u32::MAX { Some("AMD FCH: ITS TIMER DOES NOT ANSWER") }
        else if r.pm(DECODEEN3) & WDT_DISABLE != 0 || control & DISABLED != 0 { Some("AMD FCH: DISABLED BY THE FIRMWARE") } else { None };
    if let Some(why) = failure {
        if wanted != before { r.set_pm(DECODEEN3, before); }
        return Err(why);
    }
    let restarted = control & FIRED != 0;
    // Stopped while it is set; FIRED written back as read clears it.
    r.set_timer(base + CONTROL, control & !(RUN | POWER_OFF | TRIGGER));
    r.set_timer(base + COUNT, seconds);
    let control = r.timer(base + CONTROL) & !(FIRED | TRIGGER);
    r.set_timer(base + CONTROL, control | RUN);
    // The count starts at a trigger written apart from RUN.
    r.set_timer(base + CONTROL, control | RUN | TRIGGER);
    if r.timer(base + CONTROL) & RUN == 0 { return Err("AMD FCH: ITS TIMER DOES NOT START"); }
    Ok(Armed { base, restarted })
}

/// The count started again.
pub fn feed(r: &mut impl Registers, base: u32) {
    let control = r.timer(base + CONTROL) & !(FIRED | TRIGGER);
    r.set_timer(base + CONTROL, control | TRIGGER);
}

/// The timer stopped.
pub fn stop(r: &mut impl Registers, base: u32) {
    let control = r.timer(base + CONTROL) & !(FIRED | TRIGGER | RUN);
    r.set_timer(base + CONTROL, control);
}
