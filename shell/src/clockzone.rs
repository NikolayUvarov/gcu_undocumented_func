//! The time zone the clocks show the time in (000-APP-0099): `date zone`, kept in `data/clock.conf` and given to the rtc
//! service at the shell's start, since the service keeps it only while it runs (rtc.wit 1.3). The file's line is read
//! by `mind::rtc::configured_zone`, tested on the host (tests/rtc_host.rs).
use alloc::format;
use alloc::string::String;
use mind::rtc::Zone;

pub const FILE: &str = "data/clock.conf";
const MAX: usize = 4096;

/// The file's text for `zone`.
pub fn text(zone: Zone) -> String { format!("# The time zone the clocks show the time in; the RTC keeps UTC (date zone).\nzone {}\n", zone) }

/// Whether the zone was loaded at start, or should be tried again.
pub enum Load { Later, Done }

/// At start: the file's zone given to the rtc service. `Later` while the volume does not answer, unless `last`.
pub fn load(last: bool) -> Load {
    let text = match mind::idl::wire::with_timeout(200, || read()) {
        Ok(text) => text,
        Err("NOT FOUND") => return Load::Done,
        Err("VFS NOT AVAILABLE") if !last => return Load::Later,
        Err(why) => { mind::println!("[SHELL] TIME ZONE NOT READ FROM {}: {}", FILE, why); return Load::Done; }
    };
    match mind::rtc::configured_zone(&text) {
        Some(zone) => match mind::rtc::set_zone(zone) {
            Ok(()) => mind::println!("[SHELL] TIME ZONE {} FROM {}", zone, FILE),
            Err(error) => mind::println!("[SHELL] TIME ZONE {} FROM {} NOT SET: {:?}", zone, FILE, error),
        },
        None => mind::println!("[SHELL] {} HAS NO LINE ZONE <OFFSET> THE CLOCK TAKES", FILE),
    }
    Load::Done
}

/// Sets the zone and keeps it in the file: the line that says so.
pub fn set(zone: Zone) -> Result<String, &'static str> {
    apply(zone)?;
    Ok(keep(zone))
}

/// The zone given to the rtc service, which every clock reads: at once, before the file is written.
pub fn apply(zone: Zone) -> Result<(), &'static str> {
    match mind::rtc::set_zone(zone) {
        Ok(()) => { mind::println!("[SHELL] TIME ZONE {} SET", zone); Ok(()) }
        Err(mind::idl::rtc::Error::Rights) => Err("ONLY THE SHELL'S RTC CLIENT MAY SET THE ZONE"),
        Err(mind::idl::rtc::Error::Invalid) => Err("THE CLOCK REFUSED THAT ZONE"),
        Err(mind::idl::rtc::Error::Unavailable) => Err("RTC NOT AVAILABLE (A SERVICE OLDER THAN RTC.WIT 1.3?)"),
    }
}

/// The zone kept in the file for the next start, which a slow disk may take seconds over: the line that says so.
pub fn keep(zone: Zone) -> String {
    match write(&text(zone)) {
        Ok(()) => format!("ZONE {}, KEPT IN {}", zone, FILE),
        Err(why) => format!("ZONE {} UNTIL THE NEXT START: NOT KEPT IN {} ({})", zone, FILE, why),
    }
}

fn read() -> Result<String, &'static str> {
    let mut file = mind::fs::File::open(FILE).map_err(crate::files::text)?;
    let mut bytes = alloc::vec![0u8; file.size().min(MAX)];
    let mut at = 0;
    while at < bytes.len() {
        match file.read(&mut bytes[at..]) { Ok(0) => break, Ok(n) => at += n, Err(e) => return Err(crate::files::text(e)) }
    }
    bytes.truncate(at);
    String::from_utf8(bytes).map_err(|_| "NOT UTF-8")
}

fn write(text: &str) -> Result<(), &'static str> {
    let mut file = mind::fs::File::create(FILE).map_err(crate::files::text)?;
    file.write(text.as_bytes()).map_err(crate::files::text)?;
    Ok(())
}
