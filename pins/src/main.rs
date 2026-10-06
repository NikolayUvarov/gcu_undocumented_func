#![no_std]
#![no_main]
// pins: the pins of an ARM board through the gpio service (issue u015), a console program. The shell lends its
// control client of `gpio` in SLOT_GPIO for REQUEST_GPIO where a pin controller runs; without it there is nothing to
// show. Reading is open, a change needs the control badge, a pin the board reserves the platform's (idl/gpio.wit).
extern crate alloc;
mod tool;

use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_GPIO};
use mind::idl::gpio as idl;
use mind::ipc::Endpoint;
use tool::{Command, Controller, Gpio, Pin, Pull, Refusal};

mind::request!(REQUEST_CONSOLE | REQUEST_GPIO);

struct Service(Endpoint);

fn lost(error: mind::Error) -> Refusal { Refusal::Lost(alloc::format!("{:?}", error)) }
fn refused(error: idl::Error) -> Refusal {
    match error { idl::Error::NoController => Refusal::NoController, idl::Error::NoPin => Refusal::NoPin, idl::Error::Reserved => Refusal::Reserved, idl::Error::Denied => Refusal::Denied, idl::Error::Unsupported => Refusal::Unsupported }
}
fn pull_of(pull: idl::Pull) -> Pull { match pull { idl::Pull::None => Pull::None, idl::Pull::Up => Pull::Up, idl::Pull::Down => Pull::Down, idl::Pull::Unknown => Pull::Unknown } }
fn pull_to(pull: Pull) -> idl::Pull { match pull { Pull::None => idl::Pull::None, Pull::Up => idl::Pull::Up, Pull::Down => idl::Pull::Down, Pull::Unknown => idl::Pull::Unknown } }

impl Gpio for Service {
    fn controllers(&mut self) -> Result<Vec<Controller>, Refusal> {
        let list = idl::controllers(self.0).map_err(lost)?;
        Ok(list.as_slice().iter().map(|c| Controller {
            kind: String::from(match c.kind { idl::Kind::Bcm2711 => "bcm2711", idl::Kind::Pl061 => "pl061" }), pins: c.pins,
            soc: String::from(c.soc.as_str()), board: String::from(c.board.as_str()),
        }).collect())
    }
    fn pins(&mut self, controller: u8) -> Result<Vec<Pin>, Refusal> {
        let list = idl::pins(self.0, controller).map_err(lost)?.map_err(refused)?;
        Ok(list.as_slice().iter().map(|p| Pin { pin: p.pin, function: p.function, functions: p.functions, level: p.level, pull: pull_of(p.pull), reserved: p.reserved, position: p.position }).collect())
    }
    fn functions(&mut self, controller: u8, pin: u8) -> Result<Vec<String>, Refusal> {
        let list = idl::functions(self.0, controller, pin).map_err(lost)?.map_err(refused)?;
        Ok(list.as_slice().iter().map(|name| String::from(name.as_str())).collect())
    }
    fn set_function(&mut self, controller: u8, pin: u8, function: u8) -> Result<(), Refusal> { idl::set_function(self.0, controller, pin, function).map_err(lost)?.map_err(refused) }
    fn write(&mut self, controller: u8, pin: u8, high: bool) -> Result<(), Refusal> { idl::write(self.0, controller, pin, high).map_err(lost)?.map_err(refused) }
    fn set_pull(&mut self, controller: u8, pin: u8, pull: Pull) -> Result<(), Refusal> { idl::set_pull(self.0, controller, pin, pull_to(pull)).map_err(lost)?.map_err(refused) }
}

mind::entry!(main);
fn main(_info: &'static mind::BootInfo) {
    mind::about!("pins — the pins of an ARM board through the gpio service: functions, levels, pulls; changes them.\nUsage: pins [-c controller] [pin | set <pin> in|out|alt<k> | write <pin> 0|1 | pull <pin> up|down|none | watch <pin>... [-t seconds]]\nWithout arguments: every pin with its header position, active function, level and pull. A pin number: all its functions,\nthe active one marked *. Names of functions come from hwdocs/ on the boot disk. Changing a pin needs the shell's grant.");
    let options = match tool::parse(mind::process::args_str()) { Ok(options) => options, Err(usage) => { mind::println!("{}", usage); mind::process::exit_with(2) } };
    if mind::dev::cap_info(SLOT_GPIO).0 != CAP_KIND_ENDPOINT {
        mind::println!("pins: no client of the gpio service: it runs only where the firmware names a known pin controller (none on x86 and QEMU), and the shell lends it (start pins there)");
        mind::process::exit_with(1)
    }
    let mut gpio = Service(Endpoint(SLOT_GPIO));
    if let Command::Watch(list, seconds) = &options.command { watch(&mut gpio, options.controller, list, *seconds) }
    let mut out = String::new();
    let result = tool::run(&options, &mut gpio, &mut out);
    for line in out.lines() { mind::println!("{}", line); }
    if let Err((message, code)) = result { mind::println!("pins: {}", message); mind::process::exit_with(code as u32) }
}

// Levels every 100 ms, a line for each change, for `seconds` or until stopped (Esc in the shell).
fn watch(gpio: &mut Service, controller: u8, list: &[u8], seconds: Option<u32>) -> ! {
    let end = seconds.map(|s| mind::time::uptime_ms() as u64 + s as u64 * 1000);
    let mut before: Option<Vec<(u8, bool)>> = None;
    loop {
        match tool::levels(gpio, controller, list) {
            Ok(now) => { for line in tool::changes(before.as_deref(), &now) { mind::println!("{}", line); } before = Some(now); }
            Err(refusal) => { mind::println!("pins: {}", tool::explain(&refusal, controller, list[0], &Command::Watch(Vec::new(), None))); mind::process::exit_with(1) }
        }
        if end.is_some_and(|end| mind::time::uptime_ms() as u64 >= end) { mind::process::exit_with(0) }
        let _ = mind::time::sleep(100);
    }
}
