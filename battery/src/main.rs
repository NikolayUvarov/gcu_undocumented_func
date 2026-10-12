#![no_std]
#![no_main]
// Ring 3 battery service (211-PRT-0008): the Smart Battery System behind the ACPI embedded controller (mind::battery),
// behind idl/battery.wit. init gives it the controller's command/status port (SLOT_DEV0) and data port (SLOT_DEV1) from
// the ECDT, and the SMBus host's base from ACPI0001's `_EC` as `smbus=0xNN`. It only reads: every 5 s, and at once for a
// client when the last reading is older. Changes of the power source, of the charge by 5 %, and errors go to the log.
use core::fmt::Write;
use mind::abi::{BootInfo, SLOT_DEV0, SLOT_DEV1};
use mind::battery::{self as sbs, Ec, Smbus, SmbusError, State};
use mind::dev::Ports;
use mind::idl::{battery as api, wire};
use mind::ipc::Endpoint;
use mind::util::FixedBuf;

const RECEIVED: usize = 9;
const PERIOD_MS: u64 = 5000;
const STEP_PERCENT: u8 = 5; // the log's step of the charge

// The controller's two ports, each its own capability.
struct EcPorts { command: Ports, command_port: u16, data: Ports, data_port: u16 }

impl sbs::Ports for EcPorts {
    fn status(&mut self) -> u8 { self.command.in8(self.command_port) }
    fn command(&mut self, value: u8) { self.command.out8(self.command_port, value) }
    fn read_data(&mut self) -> u8 { self.data.in8(self.data_port) }
    fn write_data(&mut self, value: u8) { self.data.out8(self.data_port, value) }
    fn pause(&mut self) { mind::time::sleep(1); }
}

// `smbus=0x20` among the arguments.
fn smbus_base() -> Option<u8> {
    let value = mind::process::args_str().split_whitespace().find_map(|word| word.strip_prefix("smbus="))?;
    u8::from_str_radix(value.trim_start_matches("0x"), 16).ok()
}

// A Smart Battery Data string (a block read), or "?".
fn text(bus: &mut Smbus<EcPorts>, command: u8, out: &mut [u8; 32]) -> usize {
    match bus.read_block(sbs::BATTERY, command, out) {
        Ok(n) => out[..n].iter().position(|&b| b == 0).unwrap_or(n),
        Err(_) => { out[0] = b'?'; 1 }
    }
}

// The reading as one line: "87 %, ON BATTERY, 2:41 TO EMPTY, 1200 MA, 12150 MV, 312 CYCLES".
fn describe(state: &State) -> FixedBuf<128> {
    let mut line = FixedBuf::<128>::new();
    let source = if state.on_ac { if state.charging { "ON AC, CHARGING" } else if state.full { "ON AC, FULL" } else { "ON AC" } } else { "ON BATTERY" };
    let _ = write!(line, "{} %, {}, ", state.percent, source);
    let (minutes, what) = if state.charging { (state.to_full, "TO FULL") } else { (state.to_empty, "TO EMPTY") };
    let _ = sbs::write_minutes(&mut line, minutes);
    let _ = write!(line, " {}, {} MA, {} MV, {} CYCLES", what, state.milliamps, state.millivolts, state.cycles);
    line
}

fn to_idl(state: &State, age_ms: u64) -> api::Status {
    api::Status {
        on_ac: state.on_ac, ac_from_charger: state.ac_from_charger, charging: state.charging, discharging: state.discharging, full: state.full,
        percent: state.percent, to_empty: state.to_empty, to_full: state.to_full, millivolts: state.millivolts, milliamps: state.milliamps,
        remaining: state.remaining, full_capacity: state.full_capacity, design: state.design, tens_of_mwh: state.tens_of_mwh, cycles: state.cycles,
        age_ms: age_ms.min(u32::MAX as u64) as u32,
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (command, data) = (Ports(SLOT_DEV0), Ports(SLOT_DEV1));
    let (Some((command_port, _)), Some((data_port, _)), Some(base)) = (command.range(), data.range(), smbus_base()) else {
        mind::println!("[BATTERY] NO EMBEDDED CONTROLLER OR SMBUS HOST GIVEN");
        return;
    };
    let mut bus = Smbus { ec: Ec { ports: EcPorts { command, command_port, data, data_port } }, base };
    mind::println!("[BATTERY] EMBEDDED CONTROLLER AT PORTS {:02X}/{:02X}, SMBUS HOST AT {:02X}", command_port, data_port, base);
    // Fitted: the manager says so, or, without a manager, the battery answers.
    let fitted = sbs::fitted(&mut bus);
    let with_charger = sbs::charger(&mut bus).is_some();
    let (mut name, mut maker, mut chemistry) = ([0u8; 32], [0u8; 32], [0u8; 32]);
    let first = sbs::read(&mut bus, with_charger);
    let present = fitted.unwrap_or(first.is_ok()) && first.is_ok();
    if present {
        let n = text(&mut bus, sbs::DEVICE_NAME, &mut name);
        let m = text(&mut bus, sbs::MANUFACTURER, &mut maker);
        let c = text(&mut bus, sbs::CHEMISTRY, &mut chemistry);
        let design_mv = bus.read_word(sbs::BATTERY, sbs::DESIGN_VOLTAGE).unwrap_or(0);
        let state = first.map(|r| r.state()).unwrap_or_default();
        mind::println!("[BATTERY] {} {} ({}): DESIGN {} {}, {} MV; MANAGER {}, CHARGER {}", core::str::from_utf8(&maker[..m]).unwrap_or("?"),
                       core::str::from_utf8(&name[..n]).unwrap_or("?"), core::str::from_utf8(&chemistry[..c]).unwrap_or("?"), state.design,
                       if state.tens_of_mwh { "X 10 MWH" } else { "MAH" }, design_mv, if fitted.is_some() { "ANSWERS" } else { "NONE" },
                       if with_charger { "ANSWERS" } else { "NONE: THE SOURCE FROM THE BATTERY'S STATUS" });
    } else {
        mind::println!("[BATTERY] NO BATTERY: MANAGER {:?}, BATTERY {:?}", fitted, first.err());
    }
    let mut last: Result<State, SmbusError> = first.map(|r| r.state());
    let mut read_at = mind::time::uptime_ms() as u64;
    let mut shown: Option<State> = None;
    loop {
        // What changed since it was last logged.
        match &last {
            Ok(state) => {
                let step = |s: &State| s.percent / STEP_PERCENT;
                if shown.is_none_or(|s| s.on_ac != state.on_ac || s.charging != state.charging || s.full != state.full || step(&s) != step(state)) {
                    mind::println!("[BATTERY] {}", describe(state).as_str());
                    shown = Some(*state);
                }
            }
            Err(error) if present && shown.is_some() => { mind::println!("[BATTERY] NOT READ: {:?}", error); shown = None; }
            Err(_) => {}
        }
        let now = mind::time::uptime_ms() as u64;
        let wait = (read_at + PERIOD_MS).saturating_sub(now).max(1) as u32;
        let request = Endpoint::SERVICE.recv_timeout(RECEIVED, wait);
        // Due, or a client asked after the period: read again first.
        if present && mind::time::uptime_ms() as u64 >= read_at + PERIOD_MS {
            last = sbs::read(&mut bus, with_charger).map(|r| r.state());
            read_at = mind::time::uptime_ms() as u64;
        }
        let Ok(request) = request else { continue };
        if !request.is_call { continue; }
        let age = mind::time::uptime_ms() as u64 - read_at;
        let _ = match api::decode(&request, RECEIVED) {
            Err(reason) => wire::reject(reason),
            Ok((api::Request::Status, call)) => match (&last, present) {
                (_, false) => api::reply_status(call, Err(api::Error::NoBattery)),
                (Ok(state), true) => api::reply_status(call, Ok(&to_idl(state, age))),
                (Err(_), true) => api::reply_status(call, Err(api::Error::Unavailable)),
            },
        };
    }
}
