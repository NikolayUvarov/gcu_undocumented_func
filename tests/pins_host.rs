//! Host tests of `pins` (pins/src/tool.rs, issue u015) and `pinmap` (pins/src/view.rs, issue u017) against a gpio service built here as `gpio` builds it: the
//! register models of the BCM2711 and the PL061 (libmind/src/gpio.rs), who may change a pin, and the hwdocs tables of
//! this repository (the Raspberry Pi 4's header).
#![allow(dead_code)]
extern crate alloc;
use std::cell::RefCell;
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
#[path = "../libmind/src/util.rs"]
mod util;
#[path = "../libmind/src/tui/mod.rs"]
mod tui;
#[path = "../libmind/src/gpio.rs"]
mod gpio;
#[path = "../pins/src/tool.rs"]
mod tool;
#[path = "../pins/src/view.rs"]
mod view;
use tool::*;

// The BCM2711 GPIO block: GPSETn and GPCLRn drive the levels (GPLEVn) of output pins; the rest is plain storage.
struct Bcm(RefCell<[u32; 64]>);
impl gpio::Registers for Bcm {
    fn read32(&self, offset: usize) -> u32 { self.0.borrow()[offset / 4] }
    fn write32(&self, offset: usize, value: u32) {
        let mut r = self.0.borrow_mut();
        match offset {
            0x1C | 0x20 => r[(0x34 + offset - 0x1C) / 4] |= value,
            0x28 | 0x2C => r[(0x34 + offset - 0x28) / 4] &= !value,
            _ => r[offset / 4] = value,
        }
    }
}
// The PL061: DATA writes change only the pins the address bits 9:2 select, and only outputs; reads are masked too.
struct Pl061(RefCell<(u32, u32)>);
impl gpio::Registers for Pl061 {
    fn read32(&self, offset: usize) -> u32 { let (data, dir) = *self.0.borrow(); if offset == 0x400 { dir } else { data & (offset as u32 >> 2) } }
    fn write32(&self, offset: usize, value: u32) {
        let mut s = self.0.borrow_mut();
        if offset == 0x400 { s.1 = value & 0xFF; } else { let mask = (offset as u32 >> 2) & s.1; s.0 = s.0 & !mask | value & mask; }
    }
}

enum Unit { Bcm(gpio::Controller<Bcm>), Pl061(gpio::Controller<Pl061>) }

// The service as gpio/src/main.rs answers: the controller, its SoC table and board file, and the client's badge.
struct Service { unit: Unit, pins: Option<gpio::Pins>, board: Option<gpio::Board>, badge: u16 }

fn refusal(r: gpio::Refusal) -> Refusal { match r { gpio::Refusal::NoPin => Refusal::NoPin, gpio::Refusal::Reserved => Refusal::Reserved, gpio::Refusal::Denied => Refusal::Denied, gpio::Refusal::Unsupported => Refusal::Unsupported } }
fn pull_to(p: gpio::Pull) -> Pull { match p { gpio::Pull::None => Pull::None, gpio::Pull::Up => Pull::Up, gpio::Pull::Down => Pull::Down, gpio::Pull::Unknown => Pull::Unknown } }
fn pull_of(p: Pull) -> gpio::Pull { match p { Pull::None => gpio::Pull::None, Pull::Up => gpio::Pull::Up, Pull::Down => gpio::Pull::Down, Pull::Unknown => gpio::Pull::Unknown } }

macro_rules! with { ($s:expr, $c:ident => $body:expr) => { match &$s.unit { Unit::Bcm($c) => $body, Unit::Pl061($c) => $body } } }

impl Service {
    fn kind(&self) -> gpio::Kind { with!(self, c => c.kind) }
    fn reserved(&self, pin: u8) -> bool { self.board.as_ref().is_some_and(|b| b.reserved(pin)) }
    fn change(&self, controller: u8, pin: u8, apply: impl Fn(&Self) -> Result<(), gpio::Refusal>) -> Result<(), Refusal> {
        if controller != 0 { return Err(Refusal::NoController); }
        if pin >= self.kind().pins() { return Err(Refusal::NoPin); }
        gpio::may_change(self.badge, self.reserved(pin)).map_err(refusal)?;
        apply(self).map_err(refusal)
    }
}

impl Gpio for Service {
    fn controllers(&mut self) -> Result<Vec<Controller>, Refusal> {
        Ok(vec![Controller { kind: self.kind().name().into(), pins: self.kind().pins(), soc: self.pins.as_ref().map_or(String::new(), |p| p.soc.as_str().into()),
                             board: self.board.as_ref().map_or(String::new(), |b| b.board.as_str().into()) }])
    }
    fn pins(&mut self, controller: u8) -> Result<Vec<Pin>, Refusal> {
        if controller != 0 { return Err(Refusal::NoController); }
        Ok((0..self.kind().pins()).map(|pin| with!(self, c => Pin {
            pin, function: c.function(pin).unwrap(), functions: c.kind.functions(), level: c.level(pin).unwrap(), pull: pull_to(c.pull(pin).unwrap()),
            reserved: self.reserved(pin), position: self.board.as_ref().map_or(0, |b| b.positions[pin as usize]),
        })).collect())
    }
    fn functions(&mut self, controller: u8, pin: u8) -> Result<Vec<String>, Refusal> {
        if controller != 0 { return Err(Refusal::NoController); }
        if pin >= self.kind().pins() { return Err(Refusal::NoPin); }
        Ok((0..self.kind().functions()).map(|f| self.pins.as_ref().and_then(|t| t.function_name(pin, f)).unwrap_or("").to_string()).collect())
    }
    fn set_function(&mut self, controller: u8, pin: u8, function: u8) -> Result<(), Refusal> { self.change(controller, pin, |s| with!(s, c => c.set_function(pin, function))) }
    fn write(&mut self, controller: u8, pin: u8, high: bool) -> Result<(), Refusal> { self.change(controller, pin, |s| with!(s, c => c.write(pin, high))) }
    fn set_pull(&mut self, controller: u8, pin: u8, pull: Pull) -> Result<(), Refusal> { self.change(controller, pin, |s| with!(s, c => c.set_pull(pin, pull_of(pull)))) }
}

// The repository's tables.
const BCM2711: &str = include_str!("../hwdocs/socs/bcm2711.pins");
const PL061: &str = include_str!("../hwdocs/socs/pl061.pins");
const RPI4B: &str = include_str!("../hwdocs/boards/rpi4b.board");

// A Raspberry Pi 4 as its firmware leaves it: UART0 on 14 and 15, the reset pulls of the table.
fn raspberry_pi(badge: u16, tables: bool) -> Service {
    let pins = gpio::Pins::parse(BCM2711).unwrap();
    let board = gpio::Board::parse(RPI4B).unwrap();
    let c = gpio::Controller { kind: gpio::Kind::Bcm2711, registers: Bcm(RefCell::new([0; 64])) };
    for pin in 0..58 { c.set_pull(pin, pins.pulls[pin as usize]).unwrap(); }
    c.set_function(14, gpio::ALT0).unwrap();
    c.set_function(15, gpio::ALT0).unwrap();
    Service { unit: Unit::Bcm(c), pins: tables.then_some(pins), board: tables.then_some(board), badge }
}

fn run_ok(service: &mut Service, args: &str) -> String {
    let mut out = String::new();
    tool::run(&parse(args).unwrap(), service, &mut out).unwrap_or_else(|e| panic!("{args}: {e:?}"));
    out
}
fn run_err(service: &mut Service, args: &str) -> (String, i32) {
    let mut out = String::new();
    tool::run(&parse(args).unwrap(), service, &mut out).unwrap_err()
}

#[test]
fn the_list_of_a_raspberry_pi() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, true);
    let out = run_ok(&mut pi, "");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "bcm2711: 58 pins, board rpi4b (POS: its header)");
    assert_eq!(lines.len(), 2 + 58, "{out}");
    let row = |pin: u8| lines.iter().find(|l| l.split_whitespace().next() == Some(&pin.to_string())).unwrap().to_string();
    assert_eq!(row(14), " 14    8  ALT0 TXD0           0      down  reserved");
    assert_eq!(row(17), " 17   11  input               0      down");
    assert_eq!(row(2), "  2    3  input               0      up");
    assert_eq!(row(40), " 40    -  input               0      down  reserved", "a pin not on the header");
}

#[test]
fn every_function_of_a_pin_with_the_active_one_marked() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, true);
    assert_eq!(run_ok(&mut pi, "14"), "pin 14 (header position 8): level 0, pull down, reserved by the board\n  input\n  output\n* ALT0 TXD0\n  ALT1 SD6\n  ALT2 DPI_D10\n  ALT3 SPI5_MOSI\n  ALT4 CTS5\n  ALT5 TXD1\n");
}

#[test]
fn a_pin_set_as_output_is_driven_and_read_back() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, true);
    assert!(run_ok(&mut pi, "set 17 out").contains("\n* output\n"));
    assert_eq!(run_ok(&mut pi, "write 17 1"), "pin 17: level 1\n");
    assert!(run_ok(&mut pi, "").contains("\n 17   11  output              1      down\n"));
    assert_eq!(run_ok(&mut pi, "write 17 low"), "pin 17: level 0\n");
    assert_eq!(run_ok(&mut pi, "pull 17 up"), "pin 17: pull up\n");
    assert!(run_ok(&mut pi, "set 18 alt5").ends_with("\n* ALT5 PWM0_0\n"), "pin 18: ALT5 is PWM0_0 in table 94");
}

#[test]
fn refusals_say_why() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, true);
    assert_eq!(run_err(&mut pi, "write 14 1"), (String::from("pin 14 is reserved by the board: only the platform may change it"), 2));
    assert_eq!(run_err(&mut pi, "write 17 1"), (String::from("pin 17 is not an output: pins set 17 out first"), 2));
    assert_eq!(run_err(&mut pi, "60"), (String::from("controller 0 has no pin 60"), 2));
    assert_eq!(run_err(&mut pi, "-c 1"), (String::from("no pin controller 1"), 1));
    let mut reader = raspberry_pi(0, true);
    assert_eq!(run_err(&mut reader, "set 17 out").0, "changing a pin needs the gpio service's control client: start pins from the shell");
    assert!(run_ok(&mut reader, "").contains("ALT0 TXD0"), "reading needs no badge");
    let mut platform = raspberry_pi(gpio::BADGE_PLATFORM, true);
    assert!(run_ok(&mut platform, "set 14 in").contains("\n* input\n"), "the platform may change a reserved pin");
}

#[test]
fn without_hwdocs_function_numbers() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, false);
    let out = run_ok(&mut pi, "");
    assert!(out.starts_with("bcm2711: 58 pins; no hwdocs table: function numbers only"), "{out}");
    assert!(out.contains("\n 14    -  ALT0                0      down\n"), "{out}");
    assert!(run_ok(&mut pi, "14").ends_with("* ALT0\n  ALT1\n  ALT2\n  ALT3\n  ALT4\n  ALT5\n"));
}

#[test]
fn a_pl061_has_inputs_outputs_and_no_pulls() {
    let mut pl = Service { unit: Unit::Pl061(gpio::Controller { kind: gpio::Kind::Pl061, registers: Pl061(RefCell::new((0, 0))) }), pins: gpio::Pins::parse(PL061), board: None, badge: gpio::BADGE_CONTROL };
    let out = run_ok(&mut pl, "");
    assert_eq!(out.lines().count(), 2 + 8, "{out}");
    assert!(out.contains("\n  3    -  input               0      ?\n"), "{out}");
    assert_eq!(run_ok(&mut pl, "3"), "pin 3: level 0, pull ?\n* input\n  output\n");
    run_ok(&mut pl, "set 3 out");
    assert_eq!(run_ok(&mut pl, "write 3 1"), "pin 3: level 1\n");
    assert_eq!(run_err(&mut pl, "pull 3 up").0, "this controller has no pulls (a PL061's pads are the SoC's)");
    assert_eq!(run_err(&mut pl, "set 3 alt0").0, "pin 3 has no such function");
}

#[test]
fn watch_reports_the_first_levels_then_changes() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, true);
    run_ok(&mut pi, "set 17 out");
    let first = levels(&mut pi, 0, &[17, 27]).unwrap();
    assert_eq!(changes(None, &first), ["pin 17: 0", "pin 27: 0"]);
    run_ok(&mut pi, "write 17 1");
    let now = levels(&mut pi, 0, &[17, 27]).unwrap();
    assert_eq!(changes(Some(&first), &now), ["pin 17: 0 -> 1"]);
    assert!(changes(Some(&now), &now).is_empty());
    assert_eq!(levels(&mut pi, 0, &[70]), Err(Refusal::NoPin));
}

#[test]
fn the_commands() {
    assert_eq!(parse("").unwrap(), Options { controller: 0, command: Command::List });
    assert_eq!(parse("-c 1 5").unwrap(), Options { controller: 1, command: Command::Show(5) });
    assert_eq!(parse("set 4 alt3").unwrap().command, Command::Set(4, ALT0 + 3));
    assert_eq!(parse("set 4 input").unwrap().command, Command::Set(4, INPUT));
    assert_eq!(parse("write 4 high").unwrap().command, Command::Write(4, true));
    assert_eq!(parse("pull 4 none").unwrap().command, Command::Pull(4, Pull::None));
    assert_eq!(parse("watch 4 5 -t 3").unwrap().command, Command::Watch(vec![4, 5], Some(3)));
    for bad in ["x", "set 4", "set 4 alt6", "write 4 2", "pull 4 sideways", "watch", "watch -t 2", "-c", "-c x 3", "300"] {
        assert_eq!(parse(bad), Err(String::from(USAGE)), "{bad}");
    }
}

// pinmap (issue u017): the header of the Raspberry Pi 4 on a screen, keys, the session's confirmation.
use keys::Key;
use view::{layout, Change, Flow, Pinmap, Slot};

fn key(code: u16) -> Key { Key(keys::event(code, 0, 0)) }
fn letter(ch: char) -> Key { Key(keys::event(0, ch as u32, 0)) }

fn screen(view: &Pinmap, gpio: &mut Service, cols: usize, rows: usize) -> Vec<String> {
    let mut cells = vec![tui::Cell::BLANK; cols * rows];
    let mut grid = tui::Grid::new(&mut cells, cols, rows);
    view.draw(&mut grid, &tui::DARK, gpio);
    (0..rows).map(|y| (0..cols).map(|x| grid.get(x, y).ch).collect::<String>().trim_end().to_string()).collect()
}

#[test]
fn the_header_as_on_the_board() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, true);
    let pins = pi.pins(0).unwrap();
    let slots = layout(&pins);
    assert_eq!(slots.len(), 40);
    assert_eq!(slots[0], Slot { position: 1, pin: None }, "position 1: 3.3 V");
    assert_eq!(slots[2], Slot { position: 3, pin: Some(2) });
    assert_eq!(slots[7], Slot { position: 8, pin: Some(14) });
    assert_eq!(slots[39], Slot { position: 40, pin: Some(21) });
    let mut view = Pinmap::new(0);
    view.refresh(&mut pi);
    let rows = screen(&view, &mut pi, 100, 30);
    assert!(rows[0].contains("pinmap — bcm2711 on rpi4b: 58 pins"), "{}", rows[0]);
    let row = rows.iter().find(|r| r.contains("GPIO14")).unwrap();
    assert!(row.contains("GPIO4  input            0  7") && row.contains(" 8 GPIO14 ALT0 TXD0        0"), "{row}");
    assert!(rows.iter().any(|r| r.contains("power or ground  1") && r.contains(" 2 power or ground")), "{rows:?}");
}

#[test]
fn a_pin_changed_after_one_confirmation() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, true);
    let mut view = Pinmap::new(0);
    view.refresh(&mut pi);
    for _ in 0..5 { view.key(key(abi::KEY_DOWN), &mut pi); }
    assert_eq!(view.selected_pin(), Some(17), "position 11");
    view.key(key(abi::KEY_ENTER), &mut pi);
    assert!(view.panel.as_ref().is_some_and(|p| p.0 == 17 && p.1 == INPUT));
    let rows = screen(&view, &mut pi, 100, 30);
    assert!(rows.iter().any(|r| r.contains("* input")) && rows.iter().any(|r| r.contains("  ALT4 SPI1_CE1_N")), "{rows:?}");
    view.key(key(abi::KEY_DOWN), &mut pi);
    view.key(key(abi::KEY_ENTER), &mut pi);
    assert_eq!(view.confirm, Some(Change::Function(17, OUTPUT)));
    assert_eq!(pi.pins(0).unwrap()[17].function, INPUT, "nothing changes before the answer");
    assert!(screen(&view, &mut pi, 100, 30).iter().any(|r| r.contains("Set pin 17 to output.")));
    view.key(letter('y'), &mut pi);
    assert_eq!((pi.pins(0).unwrap()[17].function, view.agreed, view.notice.as_str()), (OUTPUT, true, "pin 17: output"));
    assert!(view.panel.is_none());
    view.key(letter('w'), &mut pi);
    assert!(pi.pins(0).unwrap()[17].level && view.notice == "pin 17: level 1", "no second question in the session");
    view.key(letter('p'), &mut pi);
    assert_eq!(view.notice, "pin 17: pull none", "down at reset, then the next pull");
    assert_eq!(view.status(), "SELECTED=17 PANEL=- CONFIRM=NONE AGREED=1 NOTICE=pin 17: pull none");
}

#[test]
fn refused_and_cancelled_changes() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, true);
    let mut view = Pinmap::new(0);
    view.refresh(&mut pi);
    for k in [abi::KEY_DOWN, abi::KEY_DOWN, abi::KEY_DOWN, abi::KEY_RIGHT] { view.key(key(k), &mut pi); }
    assert_eq!(view.selected_pin(), Some(14), "position 8");
    view.key(letter('w'), &mut pi);
    assert_eq!(view.notice, "pin 14 is not an output: Enter, then output");
    view.key(key(abi::KEY_ENTER), &mut pi);
    view.key(key(abi::KEY_UP), &mut pi);
    view.key(key(abi::KEY_UP), &mut pi);
    view.key(key(abi::KEY_ENTER), &mut pi);
    view.key(letter('n'), &mut pi);
    assert_eq!((view.notice.as_str(), view.agreed, view.panel.is_none()), ("nothing changed", false, true));
    view.key(key(abi::KEY_ENTER), &mut pi);
    view.key(key(abi::KEY_UP), &mut pi);
    view.key(key(abi::KEY_ENTER), &mut pi);
    view.key(key(abi::KEY_ENTER), &mut pi);
    assert_eq!(view.notice, "pin 14 is reserved by the board: only the platform may change it");
    assert_eq!(pi.pins(0).unwrap()[14].function, ALT0, "UART0 stays");
    view.key(key(abi::KEY_LEFT), &mut pi);
    view.key(key(abi::KEY_UP), &mut pi);
    assert_eq!(view.selected_pin(), Some(3), "position 5");
    view.key(key(abi::KEY_UP), &mut pi);
    view.key(key(abi::KEY_UP), &mut pi);
    view.key(key(abi::KEY_ENTER), &mut pi);
    assert_eq!(view.notice, "power or ground: no pin to change");
    assert!(matches!(view.key(letter('q'), &mut pi), Flow::Quit));
}

#[test]
fn without_a_board_file_or_a_controller() {
    let mut pi = raspberry_pi(gpio::BADGE_CONTROL, false);
    let pins = pi.pins(0).unwrap();
    assert_eq!(layout(&pins).len(), 58);
    assert!(layout(&pins).iter().enumerate().all(|(i, s)| *s == Slot { position: 0, pin: Some(i as u8) }));
    let mut view = Pinmap::new(1);
    view.refresh(&mut pi);
    assert_eq!(view.problem.as_deref(), Some("no pin controller 1"));
    assert!(screen(&view, &mut pi, 100, 30).iter().any(|r| r.contains("No pin controller")));
}
