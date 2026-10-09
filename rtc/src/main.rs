#![no_std]
#![no_main]
// LEGACY: the CMOS RTC on ISA ports (docs/legacy.md); the PL031 of aarch64 `virt` is the other clock (issue 202).
// Ring 3 RTC driver: answers CALL with the time since midnight and the date, and sets them for the client with the
// setting badge (idl/rtc.wit).
mod cmos;
use cmos::{decode_time, encode_date, encode_time};
use mind::abi::{BootInfo, CAP_KIND_MMIO, SLOT_DEV0};
use mind::dev::{cap_info, Mmio, Ports};
use mind::idl::{rtc, wire};
use mind::ipc::Endpoint;

const SECONDS: u8 = 0x00; const MINUTES: u8 = 0x02; const HOURS: u8 = 0x04; const DAY: u8 = 0x07; const MONTH: u8 = 0x08; const YEAR: u8 = 0x09;
const STATUS_A: u8 = 0x0A; const STATUS_B: u8 = 0x0B; const UPDATE_IN_PROGRESS: u8 = 0x80; const SET: u8 = 0x80;
const WEEKDAY: u8 = 0x06;

// The update window (UIP, up to about 2 ms once a second) can cover several quick tries: after QUICK of them the
// reads wait a tick, so a later try lands after the update.
const ATTEMPTS: usize = 8; const QUICK: usize = 4;
fn settle(attempt: usize) { if attempt >= QUICK { mind::time::sleep(10); } }

fn read_time(cmos: Ports) -> Option<usize> {
    let read = |reg: u8| -> u8 { cmos.out8(0x70, reg); cmos.in8(0x71) };
    for attempt in 0..ATTEMPTS {
        settle(attempt);
        if read(STATUS_A) & UPDATE_IN_PROGRESS != 0 { continue; }
        let first = [read(SECONDS), read(MINUTES), read(HOURS), read(STATUS_B)];
        if read(STATUS_A) & UPDATE_IN_PROGRESS != 0 { continue; }
        let second = [read(SECONDS), read(MINUTES), read(HOURS), read(STATUS_B)];
        if read(STATUS_A) & UPDATE_IN_PROGRESS == 0 && first == second {
            return decode_time(second);
        }
    }
    None
}

// Days since 2000-01-01 from the CMOS date registers (two-digit year: 2000-2099).
fn read_date(cmos: Ports) -> Option<u32> {
    let read = |reg: u8| -> u8 { cmos.out8(0x70, reg); cmos.in8(0x71) };
    for attempt in 0..ATTEMPTS {
        settle(attempt);
        if read(STATUS_A) & UPDATE_IN_PROGRESS != 0 { continue; }
        let first = [read(DAY), read(MONTH), read(YEAR), read(STATUS_B)];
        if read(STATUS_A) & UPDATE_IN_PROGRESS != 0 { continue; }
        let second = [read(DAY), read(MONTH), read(YEAR), read(STATUS_B)];
        if read(STATUS_A) & UPDATE_IN_PROGRESS == 0 && first == second {
            let [day, month, year, mode] = second;
            let decode = |value: u8| -> Option<u32> { if mode & 0x04 != 0 { Some(value as u32) } else if value & 0x0F <= 9 && value >> 4 <= 9 { Some(((value >> 4) * 10 + (value & 0x0F)) as u32) } else { None } };
            return mind::rtc::days_from_civil(2000 + decode(year)?, decode(month)?, decode(day)?);
        }
    }
    None
}

// The device in SLOT_DEV0: CMOS ports, or the PL031's registers (RTCDR: seconds since 1970, UTC).
enum Clock { Cmos(Ports), Pl031(Mmio) }
const DAYS_1970_TO_2000: u32 = 10957;
impl Clock {
    fn time(&self) -> Option<usize> { match self { Self::Cmos(cmos) => read_time(*cmos), Self::Pl031(rtc) => Some(rtc.read32(0) as usize % 86400) } }
    fn date(&self) -> Option<u32> { match self { Self::Cmos(cmos) => read_date(*cmos), Self::Pl031(rtc) => (rtc.read32(0) / 86400).checked_sub(DAYS_1970_TO_2000) } }
    // The clock set to `date` (days since 2000-01-01) and `seconds` since midnight (211-KRN-0051).
    fn set(&self, date: u32, seconds: u32) -> Result<(), rtc::Error> {
        let civil = mind::rtc::civil_from_days(date);
        match self {
            Self::Cmos(cmos) => {
                let read = |reg: u8| -> u8 { cmos.out8(0x70, reg); cmos.in8(0x71) };
                let write = |reg: u8, value: u8| { cmos.out8(0x70, reg); cmos.out8(0x71, value) };
                let mode = read(STATUS_B) & !SET;
                let (Some(time), Some(day)) = (encode_time(mode, seconds), encode_date(mode, date, civil)) else { return Err(rtc::Error::Invalid) };
                // SET holds the clock still while its registers change.
                write(STATUS_B, mode | SET);
                for (reg, value) in [(SECONDS, time[0]), (MINUTES, time[1]), (HOURS, time[2]), (WEEKDAY, day[0]), (DAY, day[1]), (MONTH, day[2]), (YEAR, day[3])] { write(reg, value); }
                write(STATUS_B, mode);
            }
            Self::Pl031(rtc) => {
                if seconds >= 86_400 { return Err(rtc::Error::Invalid); }
                let since_1970 = (date as u64 + DAYS_1970_TO_2000 as u64) * 86_400 + seconds as u64;
                rtc.write32(8, u32::try_from(since_1970).map_err(|_| rtc::Error::Invalid)?); // RTCLR
            }
        }
        if self.date() == Some(date) { Ok(()) } else { Err(rtc::Error::Unavailable) }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let clock = match cap_info(SLOT_DEV0).0 { CAP_KIND_MMIO => Mmio::map(SLOT_DEV0).map(Clock::Pl031).unwrap_or(Clock::Cmos(Ports(SLOT_DEV0))), _ => Clock::Cmos(Ports(SLOT_DEV0)) };
    loop {
        // Requests are checked against idl/rtc.wit before they are served (MC-2.4).
        let Ok(request) = Endpoint::SERVICE.recv(0) else { continue };
        let _ = match rtc::decode(&request, 0) {
            Ok((rtc::Request::Now, call)) => rtc::reply_now(call, clock.time().map(|seconds| seconds as u32)),
            Ok((rtc::Request::Date, call)) => rtc::reply_date(call, clock.date()),
            Ok((rtc::Request::Set { date, seconds }, call)) => {
                let result = if request.badge & mind::rtc::BADGE_SET == 0 { Err(rtc::Error::Rights) } else { clock.set(date, seconds) };
                if result.is_ok() { mind::println!("[RTC] SET BY PID {}", request.sender); }
                rtc::reply_set(call, result)
            }
            Err(reason) if request.is_call => wire::reject(reason),
            Err(_) => Ok(()),
        };
    }
}
