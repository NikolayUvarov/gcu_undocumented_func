//! `recovery`'s screen (308-APP-0122): a recovery key of the store's volume key made, shown once and typed back, or a
//! store that no keeper of this machine opens opened with one. The key is drawn on the screen alone: it reaches no log
//! and no serial line, and `status` names the step, never the key. Only the drawing and the keys: the program gives it
//! a `Store`, the host test a stand-in (tests/recovery_host.rs).
use crate::keyfile::{canonical, typo, wipe};
use crate::keys::{Code, Key};
use crate::tui::widgets::{Edit, InputLine, LINE_MAX};
use crate::tui::{Grid, Line, Rect, Theme};
use alloc::format;
use alloc::string::String;

/// What the store's client did not do: its error (`blockstore.wit`'s code), or no client, or no answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused { Store(u8), NoClient, NoAnswer }

// blockstore.wit's error codes this program tells apart.
pub const FULL: u8 = 2;
pub const READ_ONLY: u8 = 4;
pub const DEVICE: u8 = 5;
pub const RIGHTS: u8 = 7;
pub const INVALID: u8 = 8;
pub const KEY: u8 = 12;
pub const NO_RANDOM: u8 = 13;

/// The store's recovery calls (blockstore.wit 2.4) through the client badged `BADGE_RECOVER`.
pub trait Store {
    /// `recoveries`: the recovery records on the medium; `Store(KEY)` while no keeper of this machine opens it.
    fn records(&mut self) -> Result<u32, Refused>;
    /// `recovery-new`: a new key, in its written form.
    fn make(&mut self) -> Result<String, Refused>;
    /// `recovery-keep`: the key typed back is kept; how many other records were erased.
    fn keep(&mut self, key: &str) -> Result<u32, Refused>;
    /// `recover`: the medium opened with the key, which this machine's keystore keeps from then on.
    fn open(&mut self, key: &str) -> Result<(), Refused>;
}

/// What a refusal means, for the call that got it.
pub fn refusal(call: &str, r: Refused) -> String {
    let why = match (call, r) {
        (_, Refused::NoClient) => "this program holds no recovery client: the shell lends it when you allow it, each time (REQUEST_RECOVER)",
        (_, Refused::NoAnswer) => "the store does not answer",
        ("make", Refused::Store(INVALID)) => "this store keeps its volume key in its superblock (a RAM disk, or no keeper): a recovery key would open nothing more",
        ("make", Refused::Store(NO_RANDOM)) => "no random source for a key",
        ("keep", Refused::Store(KEY)) => "the key opens no record of this medium: nothing was erased",
        ("open", Refused::Store(INVALID)) => "the store is open already, or keeps its key in its superblock",
        ("open", Refused::Store(KEY)) => "the key opens no record of this medium, or no keeper kept it: nothing changed",
        (_, Refused::Store(KEY)) => "the store is closed: no keeper of this machine keeps its volume key",
        (_, Refused::Store(FULL)) => "the medium is full",
        (_, Refused::Store(READ_ONLY)) => "the medium is read-only",
        (_, Refused::Store(DEVICE)) => "the medium did not answer",
        (_, Refused::Store(RIGHTS)) => "the client may not do this (not badged BADGE_RECOVER)",
        (_, Refused::Store(_)) => "",
    };
    match r { Refused::Store(code) if why.is_empty() => format!("the store refused (error {})", code), _ => String::from(why) }
}

/// The store as the program last read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State { Open { records: u32 }, Closed, Unknown(String) }

/// The menu, the key shown once, the key typed back, or a key typed to open the store.
pub enum Mode { Menu, Shown { key: String }, Confirm { key: String, line: InputLine, problem: Option<String> }, Opening { line: InputLine, problem: Option<String> } }

pub enum Flow { Continue, Quit }

pub struct Recovery {
    pub state: State,
    pub mode: Mode,
    /// The menu's row: 0 make, 1 open, 2 quit.
    pub selected: usize,
    /// What the last step did, under the menu and in the log.
    pub notice: String,
    /// In a window of wm's: the frame without a title.
    pub windowed: bool,
}

pub const ROWS: [&str; 3] = ["Make a recovery key", "Open the store with a recovery key", "Quit"];

// The line's bytes overwritten before it is emptied: a key does not stay in memory after use.
fn clean(line: &mut InputLine) {
    line.set(core::str::from_utf8(&[0u8; LINE_MAX]).unwrap_or(""));
    line.clear();
}

impl Recovery {
    pub fn new() -> Self { Self { state: State::Unknown(String::new()), mode: Mode::Menu, selected: 0, notice: String::new(), windowed: false } }

    /// The store's state read again.
    pub fn refresh(&mut self, store: &mut dyn Store) {
        self.state = match store.records() {
            Ok(records) => State::Open { records },
            Err(Refused::Store(KEY)) => State::Closed,
            Err(r) => State::Unknown(refusal("records", r)),
        };
        self.selected = if self.state == State::Closed { 1 } else { 0 };
    }

    fn back(&mut self, notice: String, store: &mut dyn Store) {
        if let Mode::Shown { key } | Mode::Confirm { key, .. } = &mut self.mode { wipe(key); }
        if let Mode::Confirm { line, .. } | Mode::Opening { line, .. } = &mut self.mode { clean(line); }
        self.mode = Mode::Menu;
        self.notice = notice;
        self.refresh(store);
    }

    fn act(&mut self, row: usize, store: &mut dyn Store) -> Flow {
        match (row, &self.state) {
            (0, State::Open { .. }) => match store.make() {
                Ok(key) => { self.mode = Mode::Shown { key }; self.notice.clear(); }
                Err(r) => self.notice = format!("No key made: {}", refusal("make", r)),
            },
            (0, State::Closed) => self.notice = String::from("The store is closed: open it with a recovery key first"),
            (1, State::Closed) => self.mode = Mode::Opening { line: InputLine::new(), problem: None },
            (1, State::Open { .. }) => self.notice = String::from("The store is open: this machine's keystore keeps its volume key"),
            (0 | 1, State::Unknown(why)) => self.notice = format!("The store cannot be asked: {}", why),
            _ => return Flow::Quit,
        }
        Flow::Continue
    }

    /// A key: ↑↓ choose, Enter acts, m makes, o opens, q or Esc ends; the key shown: Enter to type it back, Esc closes;
    /// a key typed: Enter checks it, Esc goes back.
    pub fn key(&mut self, key: Key, store: &mut dyn Store) -> Flow {
        match &mut self.mode {
            Mode::Menu => match key.code() {
                Code::Up => self.selected = self.selected.saturating_sub(1),
                Code::Down => self.selected = (self.selected + 1).min(ROWS.len() - 1),
                Code::Enter => return self.act(self.selected, store),
                Code::Esc => return Flow::Quit,
                Code::Char => match key.char() {
                    Some('m') => return self.act(0, store),
                    Some('o') => return self.act(1, store),
                    Some('q') => return Flow::Quit,
                    _ => {}
                },
                _ => {}
            },
            Mode::Shown { key: shown } => match key.code() {
                Code::Enter => { let shown = core::mem::take(shown); self.mode = Mode::Confirm { key: shown, line: InputLine::new(), problem: None }; }
                Code::Esc => self.back(String::from("Not kept: the older recovery keys still open the medium"), store),
                _ => {}
            },
            Mode::Confirm { key: shown, line, problem } => match line.key(key) {
                Edit::Cancel => self.back(String::from("Not kept: the older recovery keys still open the medium"), store),
                Edit::Submit => match canonical(line.as_str()) {
                    Err(t) => { *problem = Some(format!("Not taken: {}. Type it again", typo(t))); clean(line); }
                    Ok(mut typed) if typed != *shown => { wipe(&mut typed); *problem = Some(String::from("This is not the key shown: type it again (Esc: the older keys stay)")); clean(line); }
                    Ok(mut typed) => {
                        let kept = store.keep(&typed);
                        wipe(&mut typed);
                        match kept {
                            Ok(erased) => self.back(format!("Kept: this key opens the medium on any machine; {} older recovery record(s) erased", erased), store),
                            Err(r) => { *problem = Some(format!("Not kept: {}", refusal("keep", r))); clean(line); }
                        }
                    }
                },
                _ => {}
            },
            Mode::Opening { line, problem } => match line.key(key) {
                Edit::Cancel => self.back(String::from("The store stays closed"), store),
                Edit::Submit => match canonical(line.as_str()) {
                    Err(t) => { *problem = Some(format!("Not taken: {}. Type it again", typo(t))); clean(line); }
                    Ok(mut typed) => {
                        let opened = store.open(&typed);
                        wipe(&mut typed);
                        match opened {
                            Ok(()) => self.back(String::from("Opened: the store mounts, and this machine's keystore keeps its volume key from now on"), store),
                            Err(r) => { *problem = Some(format!("Not opened: {}", refusal("open", r))); clean(line); }
                        }
                    }
                },
                _ => {}
            },
        }
        Flow::Continue
    }

    /// The step for the serial log: never the key or its length.
    pub fn status(&self) -> String {
        let state = match &self.state { State::Open { records } => format!("OPEN, {} RECOVERY RECORD(S)", records), State::Closed => String::from("CLOSED"), State::Unknown(_) => String::from("UNKNOWN") };
        let mode = match &self.mode { Mode::Menu => "MENU", Mode::Shown { .. } => "KEY SHOWN", Mode::Confirm { .. } => "TYPING THE KEY BACK", Mode::Opening { .. } => "TYPING A KEY TO OPEN" };
        format!("{}; THE STORE {}", mode, state)
    }

    /// Draws the screen; returns where the text cursor is (in a key's field).
    pub fn draw(&self, grid: &mut Grid, theme: &Theme) -> Option<(usize, usize)> {
        let (cols, rows) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        let area = grid.area();
        if self.windowed { grid.frame(area, Line::Single, theme.frame) } else { grid.frame_titled(area, Line::Single, " recovery ", theme.frame, theme.header) }
        let w = cols.saturating_sub(4);
        grid.text_max(2, 1, "A recovery key opens this store's medium on another machine, whose keystore does not keep its key.", w, theme.dim);
        let state = match &self.state {
            State::Open { records } => format!("The store is open. Recovery records on its medium: {}", records),
            State::Closed => String::from("The store is closed: no keeper of this machine keeps its volume key"),
            State::Unknown(why) => format!("The store cannot be asked: {}", why),
        };
        grid.text_max(2, 3, &state, w, if matches!(self.state, State::Open { .. }) { theme.header } else { theme.error });
        for (i, row) in ROWS.iter().enumerate() {
            grid.text_padded(2, 5 + i, row, 40.min(w), if i == self.selected { theme.selected } else { theme.panel });
        }
        grid.text_max(2, 9, &self.notice, w, theme.accent);
        grid.text_max(2, rows.saturating_sub(2), "↑↓ choose · Enter · m: make · o: open · q or Esc: quit", w, theme.dim);
        let (bw, bh) = (64.min(cols), 10);
        let (bx, by) = ((cols - bw) / 2, rows.saturating_sub(bh) / 2);
        let r = Rect::new(bx, by, bw, bh);
        let field = |grid: &mut Grid, line: &InputLine, y: usize| {
            let typed: String = core::iter::repeat('•').take(line.as_str().chars().count()).collect();
            grid.text_padded(bx + 2, y, &typed, bw - 4, theme.input);
            Some((bx + 2 + line.cursor_chars().min(bw - 5), y))
        };
        match &self.mode {
            Mode::Menu => None,
            Mode::Shown { key } => {
                grid.fill(r, ' ', theme.dialog);
                grid.frame_titled(r, Line::Double, " A new recovery key ", theme.dialog_frame, theme.dialog_frame);
                grid.text_max(bx + 2, by + 1, "Write it down. It is shown once and kept nowhere on this machine:", bw - 4, theme.dialog);
                grid.text_max(bx + 2 + (bw - 4).saturating_sub(key.len()) / 2, by + 3, key, bw - 4, theme.header);
                grid.text_max(bx + 2, by + 5, "Until you type it back, the older keys still open the medium.", bw - 4, theme.dialog);
                grid.text_max(bx + 2, by + bh - 2, "Enter: written down, type it back · Esc: close", bw - 4, theme.dialog);
                None
            }
            Mode::Confirm { line, problem, .. } => {
                grid.fill(r, ' ', theme.dialog);
                grid.frame_titled(r, Line::Double, " The key typed back ", theme.dialog_frame, theme.dialog_frame);
                grid.text_max(bx + 2, by + 1, "Type the key you wrote down (not shown as it is typed):", bw - 4, theme.dialog);
                let cursor = field(grid, line, by + 2);
                if let Some(why) = problem { grid.text_max(bx + 2, by + 4, why, bw - 4, theme.error); }
                grid.text_max(bx + 2, by + bh - 2, "Enter: keep it · Esc: close, the older keys stay", bw - 4, theme.dialog);
                cursor
            }
            Mode::Opening { line, problem } => {
                grid.fill(r, ' ', theme.dialog);
                grid.frame_titled(r, Line::Double, " Open with a recovery key ", theme.dialog_frame, theme.dialog_frame);
                grid.text_max(bx + 2, by + 1, "The medium's recovery key (not shown as it is typed):", bw - 4, theme.dialog);
                let cursor = field(grid, line, by + 2);
                if let Some(why) = problem { grid.text_max(bx + 2, by + 4, why, bw - 4, theme.error); }
                grid.text_max(bx + 2, by + bh - 2, "Enter: open · Esc: back", bw - 4, theme.dialog);
                cursor
            }
        }
    }
}

impl Default for Recovery { fn default() -> Self { Self::new() } }
