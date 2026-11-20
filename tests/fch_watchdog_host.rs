// The AMD FCH's watchdog (213-KRN-0125) against a model of its registers: an FCH that arms, its WDT_TMREN set as Linux's
// sp5100_tco sets it, one the firmware disabled, a revision the code does not know, a timer that does not answer, and the
// feed and the stop before a reboot.
#[allow(dead_code)]
#[path = "../kernel/src/arch/x86_64/fch.rs"]
mod fch;

use fch::*;

// The timer answers at TIMER_DECODED while WDT_TMREN is set and `decoded` holds, and at TIMER_IN_ACPI_MMIO while MMIOEN is.
struct Model {
    smbus: (u32, u8),
    pm: [u8; 8],
    pm_sticks: bool,
    tmren_sticks: bool,
    decoded: bool,
    control: u32,
    count: u32,
    pm_reads: usize,
    log: Vec<(u32, u32)>,
}

impl Model {
    // As the Ryzen's firmware may leave it: WDT_TMREN clear, the ACPI MMIO block decoded, the watchdog disabled in PM.
    fn zen() -> Self {
        Model { smbus: (SMBUS, 0x51), pm: [0, 0, 0, WDT_DISABLE, MMIOEN, 0, 0, 0], pm_sticks: true, tmren_sticks: true, decoded: true,
                control: POWER_OFF, count: 0xFFFF, pm_reads: 0, log: Vec::new() }
    }
    fn answers(&self, address: u32) -> bool {
        let base = address & !0xF;
        base == TIMER_DECODED && self.decoded && self.pm[DECODEEN as usize] & WDT_TMREN != 0
            || base == TIMER_IN_ACPI_MMIO && self.pm[ISACONTROL as usize] & MMIOEN != 0
    }
}

impl Registers for Model {
    fn smbus(&mut self) -> (u32, u8) { self.smbus }
    fn pm(&mut self, offset: u8) -> u8 { self.pm_reads += 1; self.pm[offset as usize] }
    fn set_pm(&mut self, offset: u8, value: u8) {
        self.log.push((PM + offset as u32, value as u32));
        let sticks = if offset == DECODEEN { self.tmren_sticks } else { self.pm_sticks };
        if sticks { self.pm[offset as usize] = value; }
    }
    fn timer(&mut self, address: u32) -> u32 {
        assert!(address & 0xF == CONTROL || address & 0xF == COUNT, "{address:#x}");
        if !self.answers(address) { return u32::MAX; }
        if address & 0xF == COUNT { self.count } else { self.control }
    }
    fn set_timer(&mut self, address: u32, value: u32) {
        assert!(self.answers(address), "a write where the timer does not answer: {address:#x}");
        self.log.push((address, value));
        if address & 0xF == COUNT { self.count = value; return; }
        // FIRED clears where a 1 is written, TRIGGER reads 0 and DISABLED is the hardware's.
        let fired = if value & FIRED != 0 { 0 } else { self.control & FIRED };
        self.control = value & !(FIRED | TRIGGER | DISABLED) | fired | self.control & DISABLED;
    }
}

fn timer_writes(model: &Model) -> Vec<(u32, u32)> { model.log.iter().copied().filter(|(a, _)| !(PM..PM + 8).contains(a)).collect() }

#[test]
fn a_zen_fch_arms_with_a_reset_after_thirty_seconds() {
    let mut m = Model::zen();
    assert_eq!(arm(&mut m, 30), Ok(Armed { base: TIMER_DECODED, restarted: false }));
    assert_eq!(m.pm[DECODEEN as usize], WDT_TMREN, "the timer decoded and its operation started, as Linux does");
    assert_eq!(m.pm[DECODEEN3 as usize], WDT_SECONDS, "enabled, counting seconds");
    assert_eq!(m.count, 30);
    assert_eq!(m.control & (RUN | POWER_OFF), RUN, "running, a reset at expiry");
    let writes = timer_writes(&m);
    assert_eq!(writes.last(), Some(&(TIMER_DECODED, RUN | TRIGGER)), "the trigger apart from RUN: {writes:x?}");
    assert_eq!(writes[writes.len() - 2], (TIMER_DECODED, RUN));
    assert_eq!(writes[1], (TIMER_DECODED + COUNT, 30), "the count set while stopped: {writes:x?}");
    // WDT_TMREN first, then DECODEEN3, then the timer: Linux's order.
    assert_eq!(&m.log[..2], &[(PM + DECODEEN as u32, WDT_TMREN as u32), (PM + DECODEEN3 as u32, WDT_SECONDS as u32)]);
}

#[test]
fn a_timer_the_firmware_decoded_already_is_armed_with_nothing_more_set() {
    let mut m = Model::zen();
    m.pm[DECODEEN as usize] = WDT_TMREN | 0x77;
    assert_eq!(arm(&mut m, 30).map(|a| a.base), Ok(TIMER_DECODED));
    assert!(!m.log.contains(&(PM + DECODEEN as u32, (WDT_TMREN | 0x77) as u32)), "DECODEEN not written again: {:x?}", m.log);
}

#[test]
fn the_acpi_mmio_window_where_the_timers_own_does_not_answer() {
    let mut m = Model::zen();
    m.decoded = false;
    assert_eq!(arm(&mut m, 30).map(|a| a.base), Ok(TIMER_IN_ACPI_MMIO));
    let mut m = Model::zen();
    m.decoded = false;
    m.pm[ISACONTROL as usize] = 0;
    assert_eq!(arm(&mut m, 30), Err("AMD FCH: ITS TIMER DOES NOT ANSWER"));
    assert_eq!((m.pm[DECODEEN as usize], m.pm[DECODEEN3 as usize]), (0, WDT_DISABLE), "the PM registers put back");
}

#[test]
fn a_wdt_tmren_that_stays_clear_is_refused_with_nothing_else_written() {
    let mut m = Model::zen();
    m.tmren_sticks = false;
    assert_eq!(arm(&mut m, 30), Err("AMD FCH: ITS TIMER COULD NOT BE ENABLED (PM DECODEEN'S WDT_TMREN STAYS CLEAR)"));
    assert_eq!(m.log, vec![(PM + DECODEEN as u32, WDT_TMREN as u32)]);
}

#[test]
fn a_reset_by_the_timer_is_reported_once_and_cleared() {
    let mut m = Model::zen();
    m.control |= FIRED;
    assert_eq!(arm(&mut m, 30).map(|a| a.restarted), Ok(true));
    assert_eq!(m.control & FIRED, 0);
    assert_eq!(arm(&mut m, 30).map(|a| a.restarted), Ok(false));
}

#[test]
fn a_timer_the_firmware_disabled_is_left_as_it_was() {
    let mut m = Model::zen();
    m.control |= DISABLED;
    assert_eq!(arm(&mut m, 30), Err("AMD FCH: DISABLED BY THE FIRMWARE (THE TIMER'S WATCHDOGDISABLE BIT)"));
    assert_eq!((m.pm[DECODEEN as usize], m.pm[DECODEEN3 as usize]), (0, WDT_DISABLE), "the PM registers put back");
    assert!(timer_writes(&m).is_empty());
    let mut m = Model::zen();
    m.pm_sticks = false;
    assert_eq!(arm(&mut m, 30), Err("AMD FCH: DISABLED BY THE FIRMWARE (PM DECODEEN3'S WATCHDOG DISABLE KEPT)"));
    assert!(timer_writes(&m).is_empty());
}

#[test]
fn an_fch_this_code_does_not_know_is_not_touched() {
    for (smbus, why) in [((SMBUS, 0x41), "AMD FCH: A REVISION BEFORE ZEN'S, NOT KNOWN HERE"), ((0xFFFF_FFFF, 0xFF), "NO AMD FCH AT 00:14.0"),
                         ((0x2918_8086, 0x02), "NO AMD FCH AT 00:14.0")] {
        let mut m = Model::zen();
        m.smbus = smbus;
        assert_eq!(arm(&mut m, 30), Err(why));
        assert_eq!((m.pm_reads, m.log.len()), (0, 0));
    }
}

#[test]
fn registers_that_do_not_answer_are_refused() {
    let mut m = Model::zen();
    m.pm = [0xFF; 8];
    assert_eq!(arm(&mut m, 30), Err("AMD FCH: ITS PM REGISTERS DO NOT ANSWER"));
    assert!(m.log.is_empty());
    let mut m = Model::zen();
    m.control = u32::MAX;
    assert_eq!(arm(&mut m, 30), Err("AMD FCH: ITS TIMER DOES NOT ANSWER"));
    assert_eq!((m.pm[DECODEEN as usize], m.pm[DECODEEN3 as usize]), (0, WDT_DISABLE));
}

#[test]
fn fed_every_second_and_stopped_before_a_reboot() {
    let mut m = Model::zen();
    let base = arm(&mut m, 30).unwrap().base;
    m.log.clear();
    feed(&mut m, base);
    assert_eq!(m.log, vec![(base, RUN | TRIGGER)]);
    stop(&mut m, base);
    assert_eq!(m.control & RUN, 0);
    assert_eq!(m.log.last(), Some(&(base, 0)));
}
