// The CMOS RTC's register values, both ways, in the mode status B names (bit 2: binary, else BCD; bit 1: 24 hours, else
// 12 with bit 7 of the hour for PM). Shared with tests/rtc_host.rs.

// Seconds since midnight from the seconds, minutes and hours registers.
pub fn decode_time([seconds, minutes, hours, mode]: [u8; 4]) -> Option<usize> {
    let decode = |value: u8| -> Option<u8> {
        if mode & 0x04 != 0 { Some(value) } else if value & 0x0F <= 9 && value >> 4 <= 9 { Some((value >> 4) * 10 + (value & 0x0F)) } else { None }
    };
    let second = decode(seconds)?; let minute = decode(minutes)?; let mut hour = decode(hours & 0x7F)?;
    if mode & 0x02 == 0 { if hour == 0 || hour > 12 { return None; } hour = hour % 12 + if hours & 0x80 != 0 { 12 } else { 0 }; } else if hours & 0x80 != 0 { return None; }
    if second >= 60 || minute >= 60 || hour >= 24 { return None; }
    Some(hour as usize * 3600 + minute as usize * 60 + second as usize)
}

fn encode(mode: u8, value: u8) -> u8 { if mode & 0x04 != 0 { value } else { (value / 10) << 4 | value % 10 } }

// The seconds, minutes and hours registers for `seconds` since midnight (211-KRN-0051).
pub fn encode_time(mode: u8, seconds: u32) -> Option<[u8; 3]> {
    if seconds >= 86_400 { return None; }
    let hour = (seconds / 3600) as u8;
    let hours = if mode & 0x02 != 0 { encode(mode, hour) } else { encode(mode, if hour % 12 == 0 { 12 } else { hour % 12 }) | if hour >= 12 { 0x80 } else { 0 } };
    Some([encode(mode, (seconds % 60) as u8), encode(mode, (seconds / 60 % 60) as u8), hours])
}

// The weekday (1: Sunday), day, month and two-digit year registers for (year, month, day) in 2000..=2099.
pub fn encode_date(mode: u8, days: u32, (year, month, day): (u32, u32, u32)) -> Option<[u8; 4]> {
    if !(2000..=2099).contains(&year) { return None; }
    let weekday = ((days + 6) % 7 + 1) as u8; // 2000-01-01 was a Saturday
    Some([weekday, encode(mode, day as u8), encode(mode, month as u8), encode(mode, (year - 2000) as u8)])
}
