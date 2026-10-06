// What `pins` says and does (issue u015), apart from the IPC: the commands, the list of pins, one pin's functions,
// changes and the levels `watch` reports. The program wraps idl/gpio.wit in `Gpio`; the host test wraps the register
// models of mind::gpio and the hwdocs tables.
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;

/// Function numbers of idl/gpio.wit: input, output, then ALT0, ALT1, ...
pub const INPUT: u8 = 0;
pub const OUTPUT: u8 = 1;
pub const ALT0: u8 = 2;

pub const USAGE: &str = "Usage: pins [-c controller] [pin | set <pin> in|out|alt<k> | write <pin> 0|1 | pull <pin> up|down|none | watch <pin>... [-t seconds]]";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Controller { pub kind: String, pub pins: u8, pub soc: String, pub board: String }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pull { None, Up, Down, Unknown }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin { pub pin: u8, pub function: u8, pub functions: u8, pub level: bool, pub pull: Pull, pub reserved: bool, pub position: u8 }

/// Why the service refused, or that it could not be asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal { NoController, NoPin, Reserved, Denied, Unsupported, Lost(String) }

/// The pin controller service as `pins` uses it (idl/gpio.wit 1.0).
pub trait Gpio {
    fn controllers(&mut self) -> Result<Vec<Controller>, Refusal>;
    fn pins(&mut self, controller: u8) -> Result<Vec<Pin>, Refusal>;
    /// The names of a pin's functions in function order; an empty name: none in the tables.
    fn functions(&mut self, controller: u8, pin: u8) -> Result<Vec<String>, Refusal>;
    fn set_function(&mut self, controller: u8, pin: u8, function: u8) -> Result<(), Refusal>;
    fn write(&mut self, controller: u8, pin: u8, high: bool) -> Result<(), Refusal>;
    fn set_pull(&mut self, controller: u8, pin: u8, pull: Pull) -> Result<(), Refusal>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command { List, Show(u8), Set(u8, u8), Write(u8, bool), Pull(u8, Pull), Watch(Vec<u8>, Option<u32>) }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options { pub controller: u8, pub command: Command }

/// The arguments: `-c n` picks a controller (0 is the first); a command follows.
pub fn parse(args: &str) -> Result<Options, String> {
    let mut words: Vec<&str> = args.split_whitespace().collect();
    let mut controller = 0;
    if let Some(at) = words.iter().position(|&w| w == "-c") {
        controller = words.get(at + 1).and_then(|w| w.parse().ok()).ok_or_else(|| String::from(USAGE))?;
        words.drain(at..at + 2);
    }
    let pin = |word: Option<&&str>| word.and_then(|w| w.parse::<u8>().ok()).ok_or_else(|| String::from(USAGE));
    let command = match words.as_slice() {
        [] => Command::List,
        [n] if n.parse::<u8>().is_ok() => Command::Show(n.parse().unwrap()),
        ["set", n, function] => Command::Set(pin(Some(n))?, match *function {
            "in" | "input" => INPUT,
            "out" | "output" => OUTPUT,
            alt => alt.strip_prefix("alt").and_then(|k| k.parse::<u8>().ok()).filter(|&k| k < 6).map(|k| ALT0 + k).ok_or_else(|| String::from(USAGE))?,
        }),
        ["write", n, level] => Command::Write(pin(Some(n))?, match *level { "1" | "high" => true, "0" | "low" => false, _ => return Err(String::from(USAGE)) }),
        ["pull", n, pull] => Command::Pull(pin(Some(n))?, match *pull { "up" => Pull::Up, "down" => Pull::Down, "none" => Pull::None, _ => return Err(String::from(USAGE)) }),
        ["watch", rest @ ..] => {
            let (mut list, mut seconds, mut at) = (Vec::new(), None, 0);
            while at < rest.len() {
                if rest[at] == "-t" { seconds = Some(rest.get(at + 1).and_then(|w| w.parse().ok()).ok_or_else(|| String::from(USAGE))?); at += 2; continue; }
                list.push(pin(rest.get(at))?);
                at += 1;
            }
            if list.is_empty() { return Err(String::from(USAGE)); }
            Command::Watch(list, seconds)
        }
        _ => return Err(String::from(USAGE)),
    };
    Ok(Options { controller, command })
}

/// What a refusal of `command` means to the user.
pub fn explain(refusal: &Refusal, controller: u8, pin: u8, command: &Command) -> String {
    match refusal {
        Refusal::NoController => format!("no pin controller {}", controller),
        Refusal::NoPin => format!("controller {} has no pin {}", controller, pin),
        Refusal::Reserved => format!("pin {} is reserved by the board: only the platform may change it", pin),
        Refusal::Denied => String::from("changing a pin needs the gpio service's control client: start pins from the shell"),
        Refusal::Unsupported => match command {
            Command::Write(..) => format!("pin {} is not an output: pins set {} out first", pin, pin),
            Command::Pull(..) => String::from("this controller has no pulls (a PL061's pads are the SoC's)"),
            _ => format!("pin {} has no such function", pin),
        },
        Refusal::Lost(why) => format!("the gpio service does not answer: {}", why),
    }
}

/// A function's name: input, output, or ALTk with its signal from the tables.
pub fn label(function: u8, names: &[String]) -> String {
    match function {
        INPUT => String::from("input"),
        OUTPUT => String::from("output"),
        f => match names.get(f as usize).filter(|n| !n.is_empty()) {
            Some(name) => format!("ALT{} {}", f - ALT0, name),
            None => format!("ALT{}", f - ALT0),
        },
    }
}

fn pull_name(pull: Pull) -> &'static str { match pull { Pull::None => "none", Pull::Up => "up", Pull::Down => "down", Pull::Unknown => "?" } }

fn describe(c: &Controller) -> String {
    let mut text = format!("{}: {} pins", c.kind, c.pins);
    if !c.board.is_empty() { let _ = write!(text, ", board {} (POS: its header)", c.board); }
    if c.soc.is_empty() { text.push_str("; no hwdocs table: function numbers only (make_usb_image.py --hwdocs puts the tables on the disk)"); }
    text
}

/// Runs a command other than `watch`; the text goes to `out`. An error is the message and the exit code (1: no
/// controller or the service lost, 2: refused or a bad pin).
pub fn run(options: &Options, gpio: &mut dyn Gpio, out: &mut String) -> Result<(), (String, i32)> {
    let c = options.controller;
    let failed = |refusal: Refusal, pin: u8| { let code = if matches!(refusal, Refusal::NoController | Refusal::Lost(_)) { 1 } else { 2 }; (explain(&refusal, c, pin, &options.command), code) };
    let controllers = gpio.controllers().map_err(|r| failed(r, 0))?;
    let Some(controller) = controllers.get(c as usize) else { return Err(failed(Refusal::NoController, 0)) };
    match options.command {
        Command::List => {
            let _ = writeln!(out, "{}", describe(controller));
            let _ = writeln!(out, "PIN  POS  FUNCTION            LEVEL  PULL");
            for p in gpio.pins(c).map_err(|r| failed(r, 0))? {
                // Names only for a pin on an alternate: input and output need none.
                let names = if p.function >= ALT0 { gpio.functions(c, p.pin).map_err(|r| failed(r, p.pin))? } else { Vec::new() };
                let position = if p.position == 0 { String::from("-") } else { p.position.to_string() };
                let row = format!("{:>3}  {:>3}  {:<18}  {:<5}  {:<4}{}", p.pin, position, label(p.function, &names), p.level as u8, pull_name(p.pull), if p.reserved { "  reserved" } else { "" });
                let _ = writeln!(out, "{}", row.trim_end());
            }
        }
        Command::Show(pin) => show(gpio, c, pin, out).map_err(|r| failed(r, pin))?,
        Command::Set(pin, function) => {
            gpio.set_function(c, pin, function).map_err(|r| failed(r, pin))?;
            show(gpio, c, pin, out).map_err(|r| failed(r, pin))?;
        }
        Command::Write(pin, high) => {
            gpio.write(c, pin, high).map_err(|r| failed(r, pin))?;
            let p = state(gpio, c, pin).map_err(|r| failed(r, pin))?;
            let _ = writeln!(out, "pin {}: level {}", pin, p.level as u8);
        }
        Command::Pull(pin, pull) => {
            gpio.set_pull(c, pin, pull).map_err(|r| failed(r, pin))?;
            let p = state(gpio, c, pin).map_err(|r| failed(r, pin))?;
            let _ = writeln!(out, "pin {}: pull {}", pin, pull_name(p.pull));
        }
        Command::Watch(..) => {}
    }
    Ok(())
}

fn state(gpio: &mut dyn Gpio, c: u8, pin: u8) -> Result<Pin, Refusal> {
    gpio.pins(c)?.into_iter().find(|p| p.pin == pin).ok_or(Refusal::NoPin)
}

/// One pin: where it is, its level and pull, and every function with the active one marked `*`.
fn show(gpio: &mut dyn Gpio, c: u8, pin: u8, out: &mut String) -> Result<(), Refusal> {
    let p = state(gpio, c, pin)?;
    let names = gpio.functions(c, pin)?;
    let _ = write!(out, "pin {}", pin);
    if p.position != 0 { let _ = write!(out, " (header position {})", p.position); }
    let _ = writeln!(out, ": level {}, pull {}{}", p.level as u8, pull_name(p.pull), if p.reserved { ", reserved by the board" } else { "" });
    for function in 0..p.functions { let _ = writeln!(out, "{} {}", if function == p.function { '*' } else { ' ' }, label(function, &names)); }
    Ok(())
}

/// The levels of `pins` on controller `c`, for `watch`.
pub fn levels(gpio: &mut dyn Gpio, c: u8, pins: &[u8]) -> Result<Vec<(u8, bool)>, Refusal> {
    let all = gpio.pins(c)?;
    pins.iter().map(|&pin| all.iter().find(|p| p.pin == pin).map(|p| (pin, p.level)).ok_or(Refusal::NoPin)).collect()
}

/// `watch`'s lines: the first reading whole, then each change.
pub fn changes(before: Option<&[(u8, bool)]>, now: &[(u8, bool)]) -> Vec<String> {
    match before {
        None => now.iter().map(|&(pin, level)| format!("pin {}: {}", pin, level as u8)).collect(),
        Some(before) => now.iter().zip(before).filter(|(a, b)| a.1 != b.1).map(|(&(pin, level), _)| format!("pin {}: {} -> {}", pin, !level as u8, level as u8)).collect(),
    }
}
