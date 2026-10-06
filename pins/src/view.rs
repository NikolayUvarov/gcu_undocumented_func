// `pinmap` (issue u017): the board's header on a screen, two columns of positions as on the board, each pin with its
// function and level; Enter lists a pin's functions, and a change asks once a session before it touches the hardware.
// Only the drawing and the keys: the program wraps idl/gpio.wit in `tool::Gpio`, the host test the register models.
use crate::keys::{Code, Key};
use crate::tool::{explain, label, Command, Controller, Gpio, Pin, Pull, OUTPUT};
use crate::tui::{Grid, Line, Style, Theme};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A place on the screen: a header position (0 when the board file gives none) and the pin there, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot { pub position: u8, pub pin: Option<u8> }

/// The places in reading order, two to a row: the header's positions 1..n (odd left, even right) when the board file
/// gives them, else the pins in order.
pub fn layout(pins: &[Pin]) -> Vec<Slot> {
    let last = pins.iter().map(|p| p.position).max().unwrap_or(0);
    if last == 0 { return pins.iter().map(|p| Slot { position: 0, pin: Some(p.pin) }).collect(); }
    let count = (last as usize).div_ceil(2) * 2;
    (1..=count as u8).map(|position| Slot { position, pin: pins.iter().find(|p| p.position == position).map(|p| p.pin) }).collect()
}

/// A change waiting for the session's confirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change { Function(u8, u8), Level(u8, bool), Pull(u8, Pull) }

pub struct Pinmap {
    pub controller: u8,
    pub controllers: Vec<Controller>,
    pub pins: Vec<Pin>,
    pub selected: usize,
    /// The functions list of a pin: (pin, cursor, names).
    pub panel: Option<(u8, u8, Vec<String>)>,
    pub confirm: Option<Change>,
    /// The user agreed to change pins in this session.
    pub agreed: bool,
    pub notice: String,
    pub problem: Option<String>,
}

pub enum Flow { Quit, Continue }

fn pull_name(pull: Pull) -> &'static str { match pull { Pull::None => "none", Pull::Up => "up", Pull::Down => "down", Pull::Unknown => "?" } }

impl Pinmap {
    pub fn new(controller: u8) -> Self {
        Self { controller, controllers: Vec::new(), pins: Vec::new(), selected: 0, panel: None, confirm: None, agreed: false, notice: String::new(), problem: None }
    }

    /// The pins' state again (every 100 ms: levels change).
    pub fn refresh(&mut self, gpio: &mut dyn Gpio) {
        let read = gpio.controllers().and_then(|list| { self.controllers = list; gpio.pins(self.controller) });
        match read {
            Ok(pins) => { self.pins = pins; self.problem = None; }
            Err(refusal) => self.problem = Some(explain(&refusal, self.controller, 0, &Command::List)),
        }
    }

    fn slots(&self) -> Vec<Slot> { layout(&self.pins) }
    fn pin(&self, pin: u8) -> Option<Pin> { self.pins.iter().copied().find(|p| p.pin == pin) }
    pub fn selected_pin(&self) -> Option<u8> { self.slots().get(self.selected).and_then(|s| s.pin) }

    fn function_text(&self, gpio: &mut dyn Gpio, p: &Pin) -> String {
        let names = if p.function >= 2 { gpio.functions(self.controller, p.pin).unwrap_or_default() } else { Vec::new() };
        label(p.function, &names)
    }

    /// One line of state for the log after each key (tests follow it).
    pub fn status(&self) -> String {
        let confirm = match self.confirm { None => String::from("NONE"), Some(c) => format!("{:?}", c).to_uppercase() };
        format!("SELECTED={} PANEL={} CONFIRM={} AGREED={} NOTICE={}", self.selected_pin().map_or(String::from("-"), |p| p.to_string()),
                self.panel.as_ref().map_or(String::from("-"), |p| format!("{}:{}", p.0, p.1)), confirm, self.agreed as u8, self.notice)
    }

    // A change: asked once a session, then made; the notice says what happened.
    fn change(&mut self, change: Change, gpio: &mut dyn Gpio) {
        if !self.agreed { self.confirm = Some(change); return; }
        self.panel = None;
        let (pin, command, result) = match change {
            Change::Function(pin, f) => (pin, Command::Set(pin, f), gpio.set_function(self.controller, pin, f)),
            Change::Level(pin, high) => (pin, Command::Write(pin, high), gpio.write(self.controller, pin, high)),
            Change::Pull(pin, pull) => (pin, Command::Pull(pin, pull), gpio.set_pull(self.controller, pin, pull)),
        };
        self.refresh(gpio);
        self.notice = match result {
            Ok(()) => {
                let p = self.pin(pin);
                match change {
                    Change::Function(..) => format!("pin {}: {}", pin, p.map_or(String::new(), |p| self.function_text(gpio, &p))),
                    Change::Level(..) => format!("pin {}: level {}", pin, p.map_or(0, |p| p.level as u8)),
                    Change::Pull(..) => format!("pin {}: pull {}", pin, p.map_or("?", |p| pull_name(p.pull))),
                }
            }
            Err(refusal) => explain(&refusal, self.controller, pin, &command),
        };
    }

    pub fn key(&mut self, key: Key, gpio: &mut dyn Gpio) -> Flow {
        let ch = key.char().map(|c| c.to_ascii_lowercase());
        if let Some(change) = self.confirm {
            match (key.code(), ch) {
                (Code::Enter, _) | (_, Some('y')) => { self.confirm = None; self.agreed = true; self.change(change, gpio); }
                (Code::Esc, _) | (_, Some('n')) => { self.confirm = None; self.panel = None; self.notice = String::from("nothing changed"); }
                _ => {}
            }
            return Flow::Continue;
        }
        if let Some((pin, cursor, names)) = self.panel.take() {
            let count = self.pin(pin).map_or(0, |p| p.functions);
            match key.code() {
                Code::Up => self.panel = Some((pin, cursor.saturating_sub(1), names)),
                Code::Down => self.panel = Some((pin, (cursor + 1).min(count.saturating_sub(1)), names)),
                Code::Enter => { self.panel = Some((pin, cursor, names)); self.change(Change::Function(pin, cursor), gpio); }
                Code::Esc => {}
                _ => self.panel = Some((pin, cursor, names)),
            }
            return Flow::Continue;
        }
        let count = self.slots().len();
        match key.code() {
            Code::Esc => return Flow::Quit,
            Code::Up if self.selected >= 2 => self.selected -= 2,
            Code::Down if self.selected + 2 < count => self.selected += 2,
            Code::Left if self.selected % 2 == 1 => self.selected -= 1,
            Code::Right if self.selected % 2 == 0 && self.selected + 1 < count => self.selected += 1,
            Code::Enter => match self.selected_pin() {
                Some(pin) => {
                    let names = gpio.functions(self.controller, pin).unwrap_or_default();
                    self.panel = Some((pin, self.pin(pin).map_or(0, |p| p.function), names));
                }
                None => self.notice = String::from("power or ground: no pin to change"),
            },
            _ => match (ch, self.selected_pin().and_then(|p| self.pin(p))) {
                (Some('q'), _) => return Flow::Quit,
                (Some('w'), Some(p)) if p.function == OUTPUT => self.change(Change::Level(p.pin, !p.level), gpio),
                (Some('w'), Some(p)) => self.notice = format!("pin {} is not an output: Enter, then output", p.pin),
                (Some('p'), Some(p)) => {
                    let next = match p.pull { Pull::None => Pull::Up, Pull::Up => Pull::Down, Pull::Down | Pull::Unknown => Pull::None };
                    self.change(Change::Pull(p.pin, next), gpio);
                }
                _ => {}
            },
        }
        Flow::Continue
    }

    // A pin's look: its function's colour, the reserved ones dim.
    fn style(p: &Pin, theme: &Theme) -> Style {
        if p.reserved { theme.dim } else if p.function == OUTPUT { theme.marked } else if p.function >= 2 { theme.header } else { theme.panel }
    }

    pub fn draw(&self, grid: &mut Grid, theme: &Theme, gpio: &mut dyn Gpio) {
        grid.clear(theme.panel);
        let area = grid.area();
        let title = match self.controllers.get(self.controller as usize) {
            Some(c) if !c.board.is_empty() => format!(" pinmap — {} on {}: {} pins ", c.kind, c.board, c.pins),
            Some(c) => format!(" pinmap — {}: {} pins ", c.kind, c.pins),
            None => String::from(" pinmap "),
        };
        grid.frame_titled(area, Line::Double, &title, theme.frame, theme.header);
        let inner = area.inner();
        if let Some(problem) = &self.problem {
            crate::tui::widgets::message(grid, "No pin controller", &[problem.as_str(), "pinmap needs the gpio service's client: start it from the shell, on a board with one."], &["Quit"], 0, theme);
            return;
        }
        // Two columns around the middle: the left one right-aligned to it, as the header is on the board.
        let half = inner.w / 2;
        let slots = self.slots();
        let top = inner.y + 1;
        for (index, slot) in slots.iter().enumerate() {
            let row = index / 2;
            if top + row >= inner.bottom().saturating_sub(2) { break; }
            let y = top + row;
            let (text, style) = match slot.pin.and_then(|n| self.pin(n)) {
                Some(p) => (format!("GPIO{:<2} {:<16} {}", p.pin, self.function_text(gpio, &p), p.level as u8), Self::style(&p, theme)),
                None => (String::from("power or ground"), theme.dim),
            };
            let position = if slot.position == 0 { String::new() } else { format!("{:>2}", slot.position) };
            let style = if index == self.selected { theme.selected } else { style };
            if index % 2 == 0 {
                let cell = format!("{} {}", text, position);
                let x = (inner.x + half).saturating_sub(cell.chars().count() + 1);
                grid.text(x, y, &cell, style);
            } else {
                grid.text(inner.x + half + 1, y, &format!("{} {}", position, text), style);
            }
        }
        let bottom = inner.bottom().saturating_sub(1);
        grid.text_max(inner.x, bottom - 1, &self.notice, inner.w, theme.accent);
        grid.text_max(inner.x, bottom, "←↑↓→ move  Enter functions  w write  p pull  q quit", inner.w, theme.dim);
        if let Some((pin, cursor, names)) = &self.panel {
            let count = self.pin(*pin).map_or(0, |p| p.functions);
            let active = self.pin(*pin).map_or(0, |p| p.function);
            let dialog = crate::tui::widgets::dialog(grid, &format!("Pin {}", pin), 34, count as usize + 4, theme);
            for f in 0..count {
                let text = format!("{} {}", if f == active { '*' } else { ' ' }, label(f, names));
                grid.text_padded(dialog.x + 1, dialog.y + 1 + f as usize, &text, dialog.w.saturating_sub(2), if f == *cursor { theme.selected } else { theme.dialog });
            }
        }
        if let Some(change) = self.confirm {
            let what = match change {
                Change::Function(pin, f) => format!("pin {} to {}", pin, label(f, &self.panel.as_ref().map_or(Vec::new(), |p| p.2.clone()))),
                Change::Level(pin, high) => format!("pin {} to level {}", pin, high as u8),
                Change::Pull(pin, pull) => format!("pin {} to pull {}", pin, pull_name(pull)),
            };
            crate::tui::widgets::message(grid, "Change a pin?", &[&format!("Set {}.", what), "This drives the board's hardware. Changes are logged."], &["Yes", "No"], 0, theme);
        }
    }
}
