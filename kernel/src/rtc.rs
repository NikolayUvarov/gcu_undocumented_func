// MC146818-compatible CMOS RTC. Register meanings are documented in QEMU:
// https://github.com/qemu/qemu/blob/master/include/hw/rtc/mc146818rtc_regs.h
const SECONDS: u8 = 0x00;
const MINUTES: u8 = 0x02;
const HOURS: u8 = 0x04;
const STATUS_A: u8 = 0x0A;
const STATUS_B: u8 = 0x0B;
const UPDATE_IN_PROGRESS: u8 = 0x80;

#[cfg(not(test))]
pub fn read_time() -> Option<usize> {
    read_consistent(|register| unsafe {
        // Select a CMOS register with NMI enabled; time/mode registers are read-only here.
        super::outb(0x70, register);
        super::inb(0x71)
    })
}

fn read_consistent(mut read: impl FnMut(u8) -> u8) -> Option<usize> {
    // Bounded retries keep a missing or stuck RTC from freezing the shell.
    for _ in 0..8 {
        if read(STATUS_A) & UPDATE_IN_PROGRESS != 0 {
            continue;
        }
        let first = [read(SECONDS), read(MINUTES), read(HOURS), read(STATUS_B)];
        if read(STATUS_A) & UPDATE_IN_PROGRESS != 0 {
            continue;
        }
        let second = [read(SECONDS), read(MINUTES), read(HOURS), read(STATUS_B)];
        if read(STATUS_A) & UPDATE_IN_PROGRESS == 0 && first == second {
            return decode_time(second);
        }
    }
    None
}

fn decode_time([seconds, minutes, hours, mode]: [u8; 4]) -> Option<usize> {
    let decode = |value: u8| -> Option<u8> {
        if mode & 0x04 != 0 {
            Some(value) // Binary mode.
        } else if value & 0x0F <= 9 && value >> 4 <= 9 {
            Some((value >> 4) * 10 + (value & 0x0F))
        } else {
            None
        }
    };
    let second = decode(seconds)?;
    let minute = decode(minutes)?;
    let mut hour = decode(hours & 0x7F)?;
    if mode & 0x02 == 0 {
        if hour == 0 || hour > 12 {
            return None;
        }
        hour = hour % 12 + if hours & 0x80 != 0 { 12 } else { 0 };
    } else if hours & 0x80 != 0 {
        return None;
    }
    if second >= 60 || minute >= 60 || hour >= 24 {
        return None;
    }
    Some(hour as usize * 3600 + minute as usize * 60 + second as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_bcd_and_binary_24_hour_time() {
        assert_eq!(decode_time([0x59, 0x58, 0x23, 0x02]), Some(86339));
        assert_eq!(decode_time([59, 58, 23, 0x06]), Some(86339));
        assert_eq!(decode_time([0, 0, 0, 0x02]), Some(0));
    }

    #[test]
    fn handles_noon_midnight_and_pm_in_12_hour_modes() {
        for (mode, twelve, eleven) in [(0x00, 0x12, 0x11), (0x04, 12, 11)] {
            assert_eq!(decode_time([0, 0, twelve, mode]), Some(0));
            assert_eq!(decode_time([0, 0, twelve | 0x80, mode]), Some(43200));
            assert_eq!(decode_time([0, 0, eleven | 0x80, mode]), Some(82800));
        }
    }

    #[test]
    fn rejects_invalid_rtc_values() {
        for sample in [
            [0x6A, 0, 0, 2],
            [60, 0, 0, 6],
            [0, 60, 0, 6],
            [0, 0, 24, 6],
            [0, 0, 0, 0],
            [0, 0, 0x13, 0],
            [0, 0, 0x81, 2],
        ] {
            assert_eq!(decode_time(sample), None);
        }
    }

    #[test]
    fn retries_a_snapshot_that_crossed_midnight() {
        let mut readings = [
            0, 0x59, 0x59, 0x23, 2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 2, 0,
        ]
        .into_iter();
        assert_eq!(read_consistent(|_| readings.next().unwrap()), Some(0));
        assert_eq!(readings.next(), None);
    }

    #[test]
    fn skips_in_progress_updates_and_bounds_retries() {
        let mut reads = 0;
        assert_eq!(
            read_consistent(|register| {
                assert_eq!(register, STATUS_A);
                reads += 1;
                UPDATE_IN_PROGRESS
            }),
            None
        );
        assert_eq!(reads, 8);

        let mut readings = [UPDATE_IN_PROGRESS, 0, 1, 2, 3, 6, 0, 1, 2, 3, 6, 0].into_iter();
        assert_eq!(read_consistent(|_| readings.next().unwrap()), Some(10921));
    }
}
