#![no_std]
#![no_main]
// Ring 3 pin controller driver (issue 207): the BCM2711's GPIO (SLOT_DEV0) and a PL061 (SLOT_DEV1), as init found them
// in the firmware's tables (PLATFORM_PINS_*), behind idl/gpio.wit. Names of functions and the board's header come from
// hwdocs/ on the boot disk when it is there (mind::gpio parses them). Reading is open; a change needs the control
// badge, a reserved pin the platform badge (MC-3.3, MC-8.3), and every change goes to the system log with who asked
// (MC-10.2). The driver never drives a pin on its own, and after a restart it leaves the pins as they are (MC-8.4).
use mind::abi::{BootInfo, CAP_KIND_MMIO, SLOT_DEV0, SLOT_DEV1};
use mind::dev::{cap_info, Mmio};
use mind::gpio::{may_change, Board, Controller, Kind, Pins, Pull, Refusal, Registers, MAX_PINS};
use mind::idl::codec::Text;
use mind::idl::{gpio as idl, wire};
use mind::ipc::Endpoint;

const RECEIVED_CAP: usize = 9;
const TABLE_BYTES: usize = 8192;

struct Window(Mmio);
impl Registers for Window {
    fn read32(&self, offset: usize) -> u32 { self.0.read32(offset) }
    fn write32(&self, offset: usize, value: u32) { self.0.write32(offset, value) }
}

struct Unit { controller: Controller<Window>, pins: Option<Pins>, board: Option<Board> }

// A hwdocs file read whole (they are small), or None.
fn read(path: &str, buffer: &mut [u8; TABLE_BYTES]) -> Option<usize> {
    let file = mind::fs::File::open(path).ok()?;
    let n = file.read_at(0, buffer).ok()?;
    (n < TABLE_BYTES).then_some(n)
}

// The SoC's table, and the first board file in hwdocs/boards/ for that SoC.
fn tables(kind: Kind) -> (Option<Pins>, Option<Board>) {
    let mut buffer = [0u8; TABLE_BYTES];
    let mut path = [0u8; 64];
    let pins = format(&mut path, &["hwdocs/socs/", kind.name(), ".pins"]).and_then(|p| read(p, &mut buffer))
        .and_then(|n| Pins::parse(core::str::from_utf8(&buffer[..n]).ok()?)).filter(|t| t.soc.as_str() == kind.name());
    let mut names = [[0u8; 32]; 8]; let mut count = 0;
    if let Ok(dir) = mind::fs::Dir::open("hwdocs/boards") {
        let _ = dir.list(|entry| { if !entry.is_dir && count < names.len() && entry.name.len() <= 32 { names[count][..entry.name.len()].copy_from_slice(entry.name); count += 1; } });
    }
    let board = names[..count].iter().find_map(|name| {
        let name = core::str::from_utf8(name).ok()?.trim_end_matches('\0');
        let bytes = name.as_bytes();
        if bytes.len() < 6 || !bytes[bytes.len() - 6..].eq_ignore_ascii_case(b".board") { return None; }
        let path = format(&mut path, &["hwdocs/boards/", name])?;
        let n = read(path, &mut buffer)?;
        Board::parse(core::str::from_utf8(&buffer[..n]).ok()?).filter(|b| b.soc.as_str() == kind.name())
    });
    (pins, board)
}

fn format<'a>(out: &'a mut [u8; 64], parts: &[&str]) -> Option<&'a str> {
    let mut at = 0;
    for part in parts { out.get_mut(at..at + part.len())?.copy_from_slice(part.as_bytes()); at += part.len(); }
    core::str::from_utf8(&out[..at]).ok()
}

fn refusal(r: Refusal) -> idl::Error { match r { Refusal::NoPin => idl::Error::NoPin, Refusal::Reserved => idl::Error::Reserved, Refusal::Denied => idl::Error::Denied, Refusal::Unsupported => idl::Error::Unsupported } }
fn pull_of(p: idl::Pull) -> Pull { match p { idl::Pull::None => Pull::None, idl::Pull::Up => Pull::Up, idl::Pull::Down => Pull::Down, idl::Pull::Unknown => Pull::Unknown } }
fn pull_to(p: Pull) -> idl::Pull { match p { Pull::None => idl::Pull::None, Pull::Up => idl::Pull::Up, Pull::Down => idl::Pull::Down, Pull::Unknown => idl::Pull::Unknown } }

impl Unit {
    fn reserved(&self, pin: u8) -> bool { self.board.as_ref().is_some_and(|b| b.reserved(pin)) }
    // A change by `badge`, checked and logged.
    fn change(&self, badge: u16, sender: u64, pin: u8, what: &str, apply: impl FnOnce(&Controller<Window>) -> Result<(), Refusal>) -> Result<(), idl::Error> {
        if pin >= self.controller.kind.pins() { return Err(idl::Error::NoPin); }
        may_change(badge, self.reserved(pin)).map_err(refusal)?;
        let before = self.controller.function(pin).ok();
        apply(&self.controller).map_err(refusal)?;
        mind::println!("[GPIO] PID {} {} PIN {}: {} (FUNCTION {:?} -> {:?}, LEVEL {:?})", sender, self.controller.kind.name(), pin, what, before, self.controller.function(pin).ok(), self.controller.level(pin).ok());
        Ok(())
    }
}

fn serve(units: &[Unit], badge: u16, sender: u64, request: idl::Request, call: wire::Call) {
    let unit = |c: u8| units.get(c as usize).ok_or(idl::Error::NoController);
    let _ = match request {
        idl::Request::Controllers => {
            let list: [idl::Controller; 2] = core::array::from_fn(|i| units.get(i).map_or_else(Default::default, |u| idl::Controller {
                kind: if u.controller.kind == Kind::Bcm2711 { idl::Kind::Bcm2711 } else { idl::Kind::Pl061 }, pins: u.controller.kind.pins(),
                soc: u.pins.as_ref().and_then(|p| Text::new(p.soc.as_str())).unwrap_or_default(),
                board: u.board.as_ref().and_then(|b| Text::new(b.board.as_str())).unwrap_or_default(),
            }));
            idl::reply_controllers(call, &list[..units.len()])
        }
        idl::Request::Pins { controller } => {
            let result = unit(controller).map(|u| {
                let c = &u.controller;
                core::array::from_fn::<idl::Pin, MAX_PINS, _>(|i| { let pin = i as u8; idl::Pin {
                    pin, function: c.function(pin).unwrap_or(0), functions: c.kind.functions(), level: c.level(pin).unwrap_or(false),
                    pull: pull_to(c.pull(pin).unwrap_or(Pull::Unknown)), reserved: u.reserved(pin), position: u.board.as_ref().map_or(0, |b| b.positions[i]),
                } })
            });
            let count = unit(controller).map_or(0, |u| u.controller.kind.pins() as usize);
            idl::reply_pins(call, result.as_ref().map(|list| &list[..count]).map_err(|e| *e))
        }
        idl::Request::Functions { controller, pin } => {
            let result = unit(controller).and_then(|u| {
                if pin >= u.controller.kind.pins() { return Err(idl::Error::NoPin); }
                Ok(core::array::from_fn::<Text<24>, 8, _>(|f| u.pins.as_ref().and_then(|t| t.function_name(pin, f as u8)).and_then(|n| Text::new(n)).unwrap_or_default()))
            });
            let count = unit(controller).map_or(0, |u| u.controller.kind.functions() as usize);
            idl::reply_functions(call, result.as_ref().map(|list| &list[..count]).map_err(|e| *e))
        }
        idl::Request::SetFunction { controller, pin, function } => {
            let r = unit(controller).and_then(|u| u.change(badge, sender, pin, "FUNCTION", |c| c.set_function(pin, function)));
            idl::reply_set_function(call, r)
        }
        idl::Request::Write { controller, pin, high } => {
            let r = unit(controller).and_then(|u| u.change(badge, sender, pin, if high { "HIGH" } else { "LOW" }, |c| c.write(pin, high)));
            idl::reply_write(call, r)
        }
        idl::Request::SetPull { controller, pin, pull } => {
            let r = unit(controller).and_then(|u| u.change(badge, sender, pin, "PULL", |c| c.set_pull(pin, pull_of(pull))));
            idl::reply_set_pull(call, r)
        }
    };
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut units: [Option<Unit>; 2] = [None, None];
    for (n, (slot, kind)) in [(SLOT_DEV0, Kind::Bcm2711), (SLOT_DEV1, Kind::Pl061)].into_iter().enumerate() {
        if cap_info(slot).0 != CAP_KIND_MMIO { continue; }
        let Ok(mmio) = Mmio::map(slot) else { continue };
        let (pins, board) = tables(kind);
        mind::println!("[GPIO] {} WITH {} PINS{}{}", kind.name(), kind.pins(), if pins.is_some() { ", FUNCTION NAMES FROM HWDOCS" } else { ", NO HWDOCS TABLE" },
                       board.as_ref().map_or("", |b| b.board.as_str()));
        units[n] = Some(Unit { controller: Controller { kind, registers: Window(mmio) }, pins, board });
    }
    let units: [Unit; 2] = match units { [Some(a), Some(b)] => [a, b], _ => {
        // One controller (the usual case): it is controller 0.
        let [a, b] = units;
        let Some(one) = a.or(b) else { mind::println!("[GPIO] NO PIN CONTROLLER"); return };
        return run(&[one]);
    } };
    run(&units)
}

fn run(units: &[Unit]) {
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        match idl::decode(&request, RECEIVED_CAP) {
            Ok((call_request, call)) => serve(units, request.badge, request.sender, call_request, call),
            Err(reason) => if request.is_call { let _ = wire::reject(reason); },
        }
    }
}
