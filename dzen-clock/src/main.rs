#![no_std]
#![no_main]
use mind::{abi, font};
mod cycle;
mod face;
mod view;
use abi::BootInfo;
use cycle::{point_at, Cycle, OrbitMode};
use face::{time_text, Face};
use view::View;

fn print(text: &[u8]) { mind::process::log(text) }

mind::entry!(main);
fn main(info: &'static BootInfo) {
    print(
        b"\r\n[DZEN-CLOCK] STARTED. D: DIGITS, C: ORBIT, P: 10S TICKS, H: TEXT, CTRL+Z: SHELL, ESC: EXIT.\r\n",
    );
    let view = View::new(info);
    view.clear();
    view.hints(true);
    view.face(Face::DARK);
    view.digital(b"--:--:--", true);
    let mut previous_face = Face::DARK;
    let mut previous_time = None;
    let mut show_digits = true;
    let mut show_hints = true;
    let mut orbit_mode = OrbitMode::Off;
    let mut previous_mode = OrbitMode::Off;
    let mut cycle = Cycle::new();
    let mut previous_dot = None;
    let mut waiting = false;
    loop {
        let key = mind::input::read_key().unwrap_or(0) as usize;
        if key == 0x01 || key == 0x1B {
            print(b"[DZEN-CLOCK] RETURNING TO KERNEL.\r\n");
            return;
        }
        // PS/2 D make code, or the UART's ASCII D/d (the shared input ABI).
        if key == 0x20 || key == b'd' as usize || key == b'D' as usize {
            show_digits = !show_digits;
            let text = previous_time.map(time_text).unwrap_or(*b"--:--:--");
            view.digital(&text, show_digits);
            print(
                if show_digits {
                    b"[DZEN-CLOCK] DIGITS ON\r\n"
                } else {
                    b"[DZEN-CLOCK] DIGITS OFF\r\n"
                },
            );
        }
        // PS/2 H make code, or UART ASCII H/h.
        if key == 0x23 || key == b'h' as usize || key == b'H' as usize {
            show_hints = !show_hints;
            view.hints(show_hints);
            print(
                if show_hints {
                    b"[DZEN-CLOCK] TEXT ON\r\n"
                } else {
                    b"[DZEN-CLOCK] TEXT OFF\r\n"
                },
            );
        }
        // PS/2 C/P make codes, or UART ASCII C/c/P/p.
        let requested = match key {
            0x2E | 0x63 | 0x43 => Some(OrbitMode::Simple),
            0x19 | 0x70 | 0x50 => Some(OrbitMode::Ticks),
            _ => None,
        };
        if let Some(requested) = requested {
            orbit_mode = orbit_mode.toggle(requested);
            print(
                match orbit_mode {
                    OrbitMode::Off => b"[DZEN-CLOCK] ORBIT OFF\r\n",
                    OrbitMode::Simple => b"[DZEN-CLOCK] ORBIT SIMPLE\r\n",
                    OrbitMode::Ticks => b"[DZEN-CLOCK] ORBIT 10S TICKS\r\n",
                },
            );
        }
        // Time now comes from the rtc driver over IPC rather than a kernel syscall.
        let seconds = mind::rtc::seconds_since_midnight().unwrap_or(abi::RTC_UNAVAILABLE);
        let now = mind::time::uptime_ms();
        cycle.observe(seconds, now);
        if let Some(current) = Face::at(seconds) {
            if current != previous_face {
                view.face(current);
                previous_face = current;
                print(b"[DZEN-CLOCK] ");
                print(&time_text(seconds));
                print(b"\r\n");
            }
            if previous_time != Some(seconds) && show_digits {
                view.digital(&time_text(seconds), true);
            }
            previous_time = Some(seconds);
            waiting = false;
        } else if previous_time.is_none() && !waiting {
            // An RTC update in progress is transient: retain the last valid face.
            print(b"[DZEN-CLOCK] WAITING FOR RTC...\r\n");
            waiting = true;
        }
        let dot = if orbit_mode != OrbitMode::Off {
            cycle
                .phase(now)
                .map(|phase| point_at(phase, view.half() * 3 / 4))
        } else {
            None
        };
        if dot != previous_dot || orbit_mode != previous_mode {
            view.cycle(previous_dot, dot, previous_mode, orbit_mode);
            previous_dot = dot;
            previous_mode = orbit_mode;
        }
        // Sleep instead of polling continuously, including when digits are hidden.
        mind::time::sleep(100);
    }
}
