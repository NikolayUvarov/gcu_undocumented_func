#![no_std]
#![no_main]
// Ring 3 CMOS RTC driver: answers CALL with the time since midnight.
use mind::abi::{BootInfo, RTC_UNAVAILABLE, SLOT_DEV0};
use mind::dev::Ports;
use mind::ipc::{self, Endpoint, Message};

const SECONDS: u8 = 0x00; const MINUTES: u8 = 0x02; const HOURS: u8 = 0x04;
const STATUS_A: u8 = 0x0A; const STATUS_B: u8 = 0x0B; const UPDATE_IN_PROGRESS: u8 = 0x80;

fn read_time(cmos: Ports) -> Option<usize> {
    let read = |reg: u8| -> u8 { cmos.out8(0x70, reg); cmos.in8(0x71) };
    for _ in 0..8 {
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

fn decode_time([seconds, minutes, hours, mode]: [u8; 4]) -> Option<usize> {
    let decode = |value: u8| -> Option<u8> {
        if mode & 0x04 != 0 { Some(value) } else if value & 0x0F <= 9 && value >> 4 <= 9 { Some((value >> 4) * 10 + (value & 0x0F)) } else { None }
    };
    let second = decode(seconds)?; let minute = decode(minutes)?; let mut hour = decode(hours & 0x7F)?;
    if mode & 0x02 == 0 { if hour == 0 || hour > 12 { return None; } hour = hour % 12 + if hours & 0x80 != 0 { 12 } else { 0 }; } else if hours & 0x80 != 0 { return None; }
    if second >= 60 || minute >= 60 || hour >= 24 { return None; }
    Some(hour as usize * 3600 + minute as usize * 60 + second as usize)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let cmos = Ports(SLOT_DEV0);
    loop {
        // The reply goes via CALL/REPLY: the client no longer needs its own endpoint for the reply.
        let Ok(request) = Endpoint::SERVICE.recv(0) else { continue };
        if !request.is_call { continue; }
        let _ = ipc::reply(&Message::new(read_time(cmos).unwrap_or(RTC_UNAVAILABLE), 0));
    }
}
