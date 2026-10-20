#[path = "../rtw89_wifi/src/io.rs"]
mod io;
#[path = "../rtw89_wifi/src/registers.rs"]
mod registers;
#[path = "../rtw89_wifi/src/power.rs"]
mod power;
#[path = "../rtw89_wifi/src/efuse.rs"]
mod efuse;
#[path = "../rtw89_wifi/src/board.rs"]
mod board;

use io::{Fault, FaultKind, RegisterIo, Result};
use registers::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Write { width: usize, at: usize, value: u32 }

struct Device {
    registers: Vec<u8>, xtal: [u8; 256], otp: [u8; 0x800], dav: [u8; 96],
    writes: Vec<Write>, attempted: usize, fail_writes: Vec<usize>,
    now: u64, freeze: bool, oversleep: u64, stuck_xtal: Option<usize>, xtal_count: usize,
    stuck_efuse: bool, minimum_delay: u32,
    late_ready: bool, backwards: bool,
    off_state: u32,
}

impl Device {
    fn new() -> Self {
        let mut value = Self {
            registers: vec![0; 0x10000], xtal: [0; 256], otp: [0xff; 0x800], dav: [0xff; 96],
            writes: Vec::new(), attempted: 0, fail_writes: Vec::new(), now: 0, freeze: false,
            oversleep: 0, stuck_xtal: None, xtal_count: 0, stuck_efuse: false, minimum_delay: 0,
            late_ready: false, backwards: false,
            off_state: 0,
        };
        value.put(SYS_CFG1, 4, 1 << 12);
        value.put(R_AX_SYS_PW_CTRL, 4, B_AX_RDY_SYSPWR);
        value.put(R_AX_SYS_WL_EFUSE_CTRL, 2, B_AX_AUTOLOAD_SUS);
        let mut logical = [0xff; efuse::LOGICAL_SIZE];
        logical[efuse::MAC_AT..efuse::MAC_AT + 6].copy_from_slice(&[2, 3, 4, 5, 6, 7]);
        logical[efuse::RFE_AT] = 1;
        logical[efuse::XTAL_AT] = 0x20;
        logical[efuse::COUNTRY_AT..efuse::COUNTRY_AT + 2].copy_from_slice(b"US");
        logical[efuse::CHANNEL_PLAN_AT] = 0x7f;
        let mut at = 4;
        for (block, bytes) in logical.chunks_exact(8).enumerate() {
            if bytes == [0xff; 8] { continue; }
            value.otp[at] = 0x30 | ((block >> 4) as u8);
            value.otp[at + 1] = (block as u8) << 4;
            value.otp[at + 2..at + 10].copy_from_slice(bytes);
            at += 10;
        }
        value.otp[efuse::PHYCAP_AT..efuse::PHYCAP_AT + efuse::PHYCAP_SIZE].fill(0x5a);
        value
    }
    fn put(&mut self, at: usize, width: usize, value: u32) { self.registers[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]); }
    fn get(&self, at: usize, width: usize) -> u32 {
        let mut raw = [0; 4]; raw[..width].copy_from_slice(&self.registers[at..at + width]); u32::from_le_bytes(raw)
    }
    fn write(&mut self, width: usize, at: usize, value: u32) -> Result<()> {
        let count = self.attempted; self.attempted += 1;
        if self.fail_writes.contains(&count) { return Err(Fault::new(FaultKind::Access, at, value)); }
        io::check_offset(at, width, self.registers.len())?;
        self.writes.push(Write { width, at, value });
        self.put(at, width, value);
        if at == R_AX_SYS_PW_CTRL && width == 4 {
            if value & B_AX_APFN_ONMAC != 0 { self.put(R_AX_IC_PWR_STATE, 4, 1 << 8); }
            if value & B_AX_APFM_OFFMAC != 0 { self.put(R_AX_IC_PWR_STATE, 4, self.off_state); }
            self.put(at, 4, value & !(B_AX_APFN_ONMAC | B_AX_APFM_OFFMAC));
        }
        if at == R_AX_EFUSE_CTRL && width == 4 && !self.stuck_efuse {
            let address = ((value & B_AX_EF_ADDR_MASK) >> 16) as usize;
            self.put(at, 4, B_AX_EF_RDY | self.otp[address] as u32);
        }
        if at == R_AX_WLAN_XTAL_SI_CTRL && width == 4 {
            self.xtal_count += 1;
            if self.stuck_xtal == Some(self.xtal_count) { return Ok(()); }
            let address = (value & 0xff) as usize;
            if value & 0x01000000 != 0 {
                self.put(at, 4, (self.xtal[address] as u32) << 8 | address as u32);
            } else {
                let mask = (value >> 16) as u8;
                self.xtal[address] = (self.xtal[address] & !mask) | (((value >> 8) as u8) & mask);
                if address == XTAL_SI_CTRL as usize && mask == XTAL_SI_MODE_SEL_MASK as u8 {
                    let index = self.xtal[XTAL_SI_LOW_ADDR as usize] as usize | ((self.xtal[address] as usize & 7) << 8);
                    self.xtal[XTAL_SI_READ_VAL as usize] = self.dav[index];
                    self.xtal[address] |= XTAL_SI_RDY as u8;
                }
                self.put(at, 4, value & !B_AX_WL_XTAL_SI_CMD_POLL);
            }
        }
        Ok(())
    }
}

impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> Result<u8> { io::check_offset(at, 1, self.registers.len())?; Ok(self.get(at, 1) as u8) }
    fn read16(&mut self, at: usize) -> Result<u16> { io::check_offset(at, 2, self.registers.len())?; Ok(self.get(at, 2) as u16) }
    fn read32(&mut self, at: usize) -> Result<u32> { io::check_offset(at, 4, self.registers.len())?; Ok(self.get(at, 4)) }
    fn write8(&mut self, at: usize, value: u8) -> Result<()> { self.write(1, at, value as u32) }
    fn write16(&mut self, at: usize, value: u16) -> Result<()> { self.write(2, at, value as u32) }
    fn write32(&mut self, at: usize, value: u32) -> Result<()> { self.write(4, at, value) }
    fn now_us(&mut self) -> u64 { self.now }
    fn delay_us(&mut self, us: u32) -> Result<()> {
        if !self.freeze { self.now += us.max(self.minimum_delay) as u64 + self.oversleep; }
        if self.backwards { self.now = 0; }
        if self.late_ready { self.put(R_AX_WLAN_XTAL_SI_CTRL, 4, 1); }
        Ok(())
    }
}

#[test]
fn grant_refuses_wrong_function_dma_io_and_short_bar() {
    assert!(io::validate_grant(0xb85210ec, 2, 0x1004, 1, 0x10000).is_ok());
    for (id, command, low, high, len) in [
        (0xb85310ec, 2, 0x1004, 1, 0x10000), (0xb85210ec, 6, 0x1004, 1, 0x10000),
        (0xb85210ec, 3, 0x1004, 1, 0x10000), (0xb85210ec, 0, 0x1004, 1, 0x10000),
        (0xb85210ec, 2, 0x1001, 0, 0x10000), (0xb85210ec, 2, 0x1002, 0, 0x10000),
        (0xb85210ec, 2, 0, 0, 0x10000), (0xb85210ec, 2, 0x1000, 0, 0xc003),
        (0xb85210ec, 2, 0xfffffff4, u32::MAX, 0x10000),
    ] { assert!(io::validate_grant(id, command, low, high, len).is_err()); }
    assert!(io::check_offset(0xffff, 1, 0x10000).is_ok());
    for (at, width) in [(0x10000, 1), (0xffff, 2), (3, 4), (usize::MAX, 4), (0, 0), (0, 8)] {
        assert!(io::check_offset(at, width, 0x10000).is_err());
    }
}

#[test]
fn unknown_cut_unavailable_or_already_active_mac_get_no_writes() {
    for value in [0, u32::MAX, 2 << 12] {
        let mut d = Device::new(); d.put(SYS_CFG1, 4, value);
        assert!(board::read(&mut d).is_err()); assert!(d.writes.is_empty());
    }
    for state in 1..=3 {
        let mut d = Device::new(); d.put(R_AX_IC_PWR_STATE, 4, state << 8);
        assert_eq!(board::read(&mut d).err().unwrap().cause.kind, FaultKind::PowerState);
        assert!(d.writes.is_empty());
    }
}

#[test]
fn full_cycle_reads_real_map_offsets_then_powers_down_without_releasing_baseband() {
    let mut d = Device::new(); d.minimum_delay = 1_000;
    let board = board::read(&mut d).ok().unwrap();
    assert_eq!(board.identity.mac, [2, 3, 4, 5, 6, 7]);
    assert_eq!((board.identity.cut, board.identity.rfe, board.identity.xtal), (1, 1, 0x20));
    assert_eq!(board.identity.country, *b"US");
    assert_eq!(board.maps.phycap, [0x5a; 128]);
    assert_eq!(board.maps.dav_logical, [0xff; 16]);
    assert_eq!(d.get(R_AX_IC_PWR_STATE, 4) & B_AX_WLMAC_PWR_STE_MASK, 0);
    assert_eq!(d.get(R_AX_SYS_FUNC_EN, 1) & 3, 0);
    assert_eq!(d.get(R_AX_SCOREBOARD + 3, 1), 0x80);
    assert!(d.now < 3_000_000);
    assert!(d.writes.iter().filter(|w| w.at == R_AX_SYS_FUNC_EN).all(|w| w.width == 1 && w.value & 3 == 0));
    assert!(d.writes.iter().filter(|w| w.at == R_AX_EFUSE_CTRL).all(|w| w.width == 4 && w.value & !B_AX_EF_ADDR_MASK == 0));
    assert_eq!(d.get(R_AX_SYS_ISO_CTRL, 2) & (B_AX_PWC_EV2EF_B14 | B_AX_PWC_EV2EF_B15 | B_AX_ISO_EB2CORE), B_AX_ISO_EB2CORE);
}

#[test]
fn every_failed_power_on_write_still_attempts_independent_shutdown() {
    let mut baseline = Device::new(); power::on_for_board_read(&mut baseline).unwrap();
    for failed in 1..=baseline.writes.len() {
        let mut d = Device::new(); d.fail_writes = vec![failed];
        let error = board::read(&mut d).err().unwrap();
        assert_eq!(error.stage, board::Stage::PowerOn, "write {failed}");
        assert!(error.shutdown.is_none(), "write {failed}: {error:?}");
        assert_eq!(d.get(R_AX_SCOREBOARD + 3, 1), 0x80, "write {failed}");
    }
}

#[test]
fn pending_xtal_timeout_gets_an_independent_cleanup_budget_without_command_replacement() {
    let mut d = Device::new(); d.stuck_xtal = Some(1);
    let error = board::read(&mut d).err().unwrap();
    assert_eq!(error.cause.kind, FaultKind::Timeout);
    assert_eq!(error.shutdown.unwrap().kind, FaultKind::Timeout);
    assert!(d.now >= 100_000 && d.now < 110_000);
    assert_eq!(d.xtal_count, 1);
    assert_ne!(d.get(R_AX_WLAN_XTAL_SI_CTRL, 4) & B_AX_WL_XTAL_SI_CMD_POLL, 0);
    assert_ne!(d.get(R_AX_SCOREBOARD + 3, 1), 0x80);
}

#[test]
fn new_xtal_reads_and_writes_never_replace_pending_or_unavailable_commands() {
    for read in [false, true] {
        for (value, frozen, kind) in [(B_AX_WL_XTAL_SI_CMD_POLL | 0x123456, false, FaultKind::Timeout),
            (B_AX_WL_XTAL_SI_CMD_POLL | 0x123456, true, FaultKind::Timeout), (u32::MAX, false, FaultKind::Unavailable)] {
            let mut d = Device::new(); d.freeze = frozen;
            d.put(R_AX_WLAN_XTAL_SI_CTRL, 4, value);
            let result = if read { power::xtal_read(&mut d, 0x80).map(|_| ()) } else { power::xtal_write(&mut d, 0x80, 3, 3) };
            assert_eq!(result.unwrap_err().kind, kind);
            assert!(d.writes.is_empty());
            assert_eq!(d.get(R_AX_WLAN_XTAL_SI_CTRL, 4), value);
            assert_eq!(d.now, if !frozen && kind == FaultKind::Timeout { 50_000 } else { 0 });
        }
    }
}

#[test]
fn completed_pending_xtal_allows_a_fresh_command_within_the_callers_budget() {
    for read in [false, true] {
        let mut d = Device::new(); d.late_ready = true;
        d.put(R_AX_WLAN_XTAL_SI_CTRL, 4, B_AX_WL_XTAL_SI_CMD_POLL | 0x123456);
        let mut timed = io::TimedIo::new(&mut d, 100);
        if read { power::xtal_read(&mut timed, 0x80).unwrap(); } else { power::xtal_write(&mut timed, 0x80, 3, 3).unwrap(); }
        assert_eq!(d.xtal_count, 1);
        assert_eq!(d.now, 50);
        assert_eq!(d.writes.len(), 1);
    }
    let mut d = Device::new();
    d.put(R_AX_WLAN_XTAL_SI_CTRL, 4, B_AX_WL_XTAL_SI_CMD_POLL);
    let mut timed = io::TimedIo::new(&mut d, 25);
    assert_eq!(power::xtal_write(&mut timed, 0x80, 3, 3).unwrap_err().kind, FaultKind::Timeout);
    assert!(d.writes.is_empty());
    assert!(d.now <= 25);
}

#[test]
fn expired_or_frozen_poll_has_finite_reads_and_rejects_late_ready() {
    let mut d = Device::new(); d.freeze = true;
    assert_eq!(io::poll32(&mut d, R_AX_WLAN_XTAL_SI_CTRL, 1, 1, 50, 500).unwrap_err().kind, FaultKind::Timeout);
    d.freeze = false; d.oversleep = 1_000; d.late_ready = true;
    assert_eq!(io::poll32(&mut d, R_AX_WLAN_XTAL_SI_CTRL, 1, 1, 50, 500).unwrap_err().kind, FaultKind::Timeout);
    let mut timed = io::TimedIo::new(&mut d, 50);
    assert_eq!(timed.delay_us(51).unwrap_err().kind, FaultKind::Timeout);
    let mut d = Device::new(); d.now = 10_000; d.backwards = true;
    assert_eq!(io::poll32(&mut d, R_AX_WLAN_XTAL_SI_CTRL, 1, 1, 50, 500).unwrap_err().kind, FaultKind::Clock);
}

#[test]
fn short_device_delays_do_not_round_up_to_the_ten_ms_sleep_tick() {
    assert!(io::validate_clock(10_000_000, 0).is_err());
    assert!(io::validate_clock(0, 3_600_000_000).is_err());
    io::validate_clock(1, 3_600_000_000).unwrap();
    let mut now = 0;
    for _ in 0..1216 {
        io::short_delay(|| { now += 100; now }, 1).unwrap();
    }
    assert!(now >= 1_216_000 && now < 2_000_000);
    assert_eq!(io::short_delay(|| 0, 1).unwrap_err().kind, FaultKind::Clock);
    let mut now = 10;
    assert_eq!(io::short_delay(|| { now -= 1; now }, 1).unwrap_err().kind, FaultKind::Clock);
    assert!(io::short_delay(|| 0, 1_001).is_err());
}

#[test]
fn ddv_read_failure_and_restore_failure_are_both_preserved() {
    let mut baseline = Device::new(); efuse::read_ddv(&mut baseline, 0, &mut [0; 1]).unwrap();
    let command = baseline.writes.iter().position(|w| w.at == R_AX_EFUSE_CTRL).unwrap();
    let restore = command + 1;
    let mut d = Device::new(); d.fail_writes = vec![command, restore];
    let error = efuse::read_ddv(&mut d, 0, &mut [0; 1]).unwrap_err();
    assert_eq!(error.cause.register, R_AX_EFUSE_CTRL);
    assert_eq!(error.restore.unwrap().register, R_AX_SYS_ISO_CTRL);
}

#[test]
fn ddv_hung_hardware_times_out_then_restores_otp_supply() {
    let mut d = Device::new(); d.stuck_efuse = true;
    let error = efuse::read_ddv(&mut d, 0x580, &mut [0; 1]).unwrap_err();
    assert_eq!(error.cause.kind, FaultKind::Timeout); assert!(error.restore.is_none());
    assert!(d.now >= 1_000_000 && d.now < 1_010_000);
    assert_eq!(d.get(R_AX_SYS_ISO_CTRL, 2) & (B_AX_PWC_EV2EF_B14 | B_AX_PWC_EV2EF_B15), 0);
    assert_eq!(d.get(R_AX_PMC_DBG_CTRL2, 1) & B_AX_SYSON_DIS_PMCR_AX_WRMSK, 0);
}

#[test]
fn ddv_rejects_empty_overflow_and_out_of_range_without_writes() {
    let mut d = Device::new();
    assert!(efuse::read_ddv(&mut d, 0, &mut []).is_err());
    assert!(efuse::read_ddv(&mut d, usize::MAX, &mut [0; 1]).is_err());
    assert!(efuse::read_ddv(&mut d, 0x7ff, &mut [0; 2]).is_err());
    assert!(d.writes.is_empty());
}

#[test]
fn nested_xtal_timeout_obeys_the_shorter_dav_byte_budget() {
    let mut d = Device::new(); d.stuck_xtal = Some(5);
    let error = efuse::read_maps(&mut d).err().unwrap();
    assert_eq!(error.cause.kind, FaultKind::Timeout);
    assert!(d.now >= 10_000 && d.now < 20_000);
}

#[test]
fn complete_board_data_is_not_success_when_shutdown_fails() {
    let mut baseline = Device::new(); assert!(board::read(&mut baseline).is_ok());
    let off_command = B_AX_WL_XTAL_SI_CMD_POLL | XTAL_SI_RFC2RF << 16 | XTAL_SI_RFC2RF << 8 | XTAL_SI_ANAPAR_WL;
    let off_start = baseline.writes.iter().position(|w| w.at == R_AX_WLAN_XTAL_SI_CTRL && w.value == off_command).unwrap();
    let mut d = Device::new(); d.fail_writes = vec![off_start];
    let error = board::read(&mut d).err().unwrap();
    assert_eq!(error.stage, board::Stage::PowerOff);
    assert_eq!(error.shutdown.unwrap().kind, FaultKind::Access);
    for state in 1..=3 {
        let mut d = Device::new(); d.off_state = state << 8;
        let error = board::read(&mut d).err().unwrap();
        assert_eq!(error.stage, board::Stage::PowerOff);
        assert_eq!(error.shutdown.unwrap().kind, FaultKind::PowerState);
    }
}

#[test]
fn invalid_board_and_failed_autoload_still_power_off() {
    let mut d = Device::new(); d.put(R_AX_SYS_WL_EFUSE_CTRL, 2, 0);
    let error = board::read(&mut d).err().unwrap();
    assert_eq!(error.cause.kind, FaultKind::Autoload); assert!(error.shutdown.is_none());
    assert_eq!(d.get(R_AX_SCOREBOARD + 3, 1), 0x80);
    let mut d = Device::new(); d.otp.fill(0xff);
    let error = board::read(&mut d).err().unwrap();
    assert_eq!(error.stage, board::Stage::Board); assert_eq!(error.cause.kind, FaultKind::Mac);
    assert!(error.shutdown.is_none());
}

#[test]
fn board_validation_rejects_unsupported_rfe_missing_trim_and_invalid_mac() {
    let mut maps = efuse::Maps { logical: [0xff; 2048], dav_logical: [0xff; 16], phycap: [0xff; 128] };
    maps.logical[efuse::MAC_AT..efuse::MAC_AT + 6].copy_from_slice(&[2, 3, 4, 5, 6, 7]);
    maps.logical[efuse::RFE_AT] = 5;
    assert_eq!(board::identity(1, &maps).unwrap_err().kind, FaultKind::Rfe);
    maps.logical[efuse::RFE_AT] = 1;
    assert_eq!(board::identity(1, &maps).unwrap_err().kind, FaultKind::Xtal);
    maps.logical[efuse::XTAL_AT] = 0x20;
    for mac in [[0; 6], [0xff; 6], [1, 3, 4, 5, 6, 7]] {
        maps.logical[efuse::MAC_AT..efuse::MAC_AT + 6].copy_from_slice(&mac);
        assert_eq!(board::identity(1, &maps).unwrap_err().kind, FaultKind::Mac);
    }
}

#[test]
fn logical_map_uses_own_dav_bounds_and_rejects_truncated_records() {
    let mut physical = [0xff; 16];
    physical[4..8].copy_from_slice(&[0x30, 0x1e, 0xaa, 0xbb]);
    let mut logical = [0; 16]; efuse::decode(&physical, &mut logical).unwrap();
    assert_eq!(&logical[8..10], &[0xaa, 0xbb]);
    physical[5] = 0x2e;
    assert!(efuse::decode(&physical, &mut logical).is_err());
    for len in 0..8 { assert!(efuse::decode(&physical[..len], &mut logical).is_err()); }
    physical[5] = 0x10;
    assert!(efuse::decode(&physical[..13], &mut logical).is_err());
}

#[test]
fn later_otp_records_replace_words_without_erasing_other_words() {
    let mut physical = [0xff; 24];
    physical[4..10].copy_from_slice(&[0x30, 0x0c, 1, 2, 3, 4]);
    physical[10..14].copy_from_slice(&[0x30, 0x0e, 9, 8]);
    let mut logical = [0; 8]; efuse::decode(&physical, &mut logical).unwrap();
    assert_eq!(logical, [9, 8, 3, 4, 0xff, 0xff, 0xff, 0xff]);
}
