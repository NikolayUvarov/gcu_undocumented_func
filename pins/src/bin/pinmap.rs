#![no_std]
#![no_main]
// pinmap: the board's header on a screen (issue u017), its pins with their functions and levels refreshed every 100 ms;
// Enter lists a pin's functions, w toggles an output, p cycles the pull. The first change of a session asks first.
extern crate alloc;

use mind::abi::{CAP_KIND_ENDPOINT, SLOT_GPIO};
use mind::tui::{Terminal, DARK};
use pins::service::Service;
use pins::tool::{Gpio, Refusal};
use pins::view::{Flow, Pinmap};

mind::request!(REQUEST_GPIO);

// Without a client every call fails alike: the view says why.
struct Absent;
impl Gpio for Absent {
    fn controllers(&mut self) -> Result<alloc::vec::Vec<pins::tool::Controller>, Refusal> { Err(Refusal::Lost(alloc::string::String::from("no client of the gpio service was lent"))) }
    fn pins(&mut self, _: u8) -> Result<alloc::vec::Vec<pins::tool::Pin>, Refusal> { Err(Refusal::NoController) }
    fn functions(&mut self, _: u8, _: u8) -> Result<alloc::vec::Vec<alloc::string::String>, Refusal> { Err(Refusal::NoController) }
    fn set_function(&mut self, _: u8, _: u8, _: u8) -> Result<(), Refusal> { Err(Refusal::NoController) }
    fn write(&mut self, _: u8, _: u8, _: bool) -> Result<(), Refusal> { Err(Refusal::NoController) }
    fn set_pull(&mut self, _: u8, _: u8, _: pins::tool::Pull) -> Result<(), Refusal> { Err(Refusal::NoController) }
}

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("pinmap — the board's header on a screen: each pin with its function and level, as the gpio service sees them.\nUsage: pinmap [controller]\n←↑↓→ move, Enter: a pin's functions (Enter selects one), w: toggle an output, p: next pull, q or Esc: quit.\nThe first change of a session asks before it touches the hardware.");
    let controller = mind::process::args_str().trim().parse().unwrap_or(0);
    let Some(mut term) = Terminal::open(info, "pinmap") else { return };
    let mut service = Service::new();
    let mut absent = Absent;
    let gpio: &mut dyn Gpio = if mind::dev::cap_info(SLOT_GPIO).0 == CAP_KIND_ENDPOINT { &mut service } else { &mut absent };
    let mut view = Pinmap::new(controller);
    view.refresh(gpio);
    mind::println!("[PINMAP] READY {}x{}{}", term.cols(), term.rows(), if view.problem.is_some() { " NO PIN CONTROLLER" } else { "" });
    loop {
        { let mut grid = term.grid(); view.draw(&mut grid, &DARK, gpio); }
        term.set_cursor(None);
        term.present();
        let Some(key) = mind::input::wait_key(100) else { if view.problem.is_none() { view.refresh(gpio); } continue };
        if view.problem.is_some() { if key.is_escape() || key.code() == mind::keys::Code::Enter || key.char() == Some('q') { break; } continue; }
        if let Flow::Quit = view.key(key, gpio) { break; }
        mind::println!("[PINMAP] {}", view.status());
    }
    mind::println!("[PINMAP] DONE");
}
