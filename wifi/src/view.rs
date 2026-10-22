//! `wifi`'s screen (550-APP-0033): the networks a scan found, one row a name with its strongest access point, the
//! strongest first; Enter joins one (a passphrase typed without echo for a protected one, kept by the station if
//! asked); the link's state at the top; c the country, from the station's catalogue (550-APP-0082). Only the drawing
//! and the keys: the program gives it a `Station`, the host test the stand-in (tests/wifi_host.rs).
use crate::keys::{Code, Key};
use crate::station::{check, mac, rate, shown, Country, CountryState, CountryStatus, Link, Network, Refusal, Security, Station};
use crate::tui::widgets::{Edit, InputLine, LINE_MAX};
use crate::tui::{Grid, Line, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// A row of the list: a network's name as sent (empty: hidden), its strongest access point's signal (minus dBm, 0
/// unknown) and channel, how many access points share the name, its protection, and whether the station remembers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row { pub ssid: Vec<u8>, pub weakness: u8, pub channel: u8, pub points: usize, pub security: Security, pub remembered: bool }

// The order of signals, strongest first; an unknown one (0) last.
fn rank(weakness: u8) -> u8 { if weakness == 0 { u8::MAX } else { weakness } }

/// The rows of a scan: one a name, the strongest first; hidden networks last, each a row.
pub fn rows(networks: &[Network]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for n in networks {
        match rows.iter_mut().find(|r| !n.ssid.is_empty() && r.ssid == n.ssid) {
            Some(r) => {
                r.points += 1;
                r.remembered |= n.remembered;
                if rank(n.weakness) < rank(r.weakness) { (r.weakness, r.channel, r.security) = (n.weakness, n.channel, n.security); }
            }
            None => rows.push(Row { ssid: n.ssid.clone(), weakness: n.weakness, channel: n.channel, points: 1, security: n.security, remembered: n.remembered }),
        }
    }
    rows.sort_by(|a, b| a.ssid.is_empty().cmp(&b.ssid.is_empty()).then(rank(a.weakness).cmp(&rank(b.weakness))));
    rows
}

/// Bars of signal, 0 to 4, from minus dBm (0 unknown: none).
pub fn bars(weakness: u8) -> usize {
    match weakness { 0 => 0, w if w <= 55 => 4, w if w <= 67 => 3, w if w <= 75 => 2, w if w <= 85 => 1, _ => 0 }
}

// A signal as dBm, or ? when unknown.
fn dbm(weakness: u8) -> String { if weakness == 0 { String::from("?") } else { format!("-{}", weakness) } }

/// The list, the passphrase being typed for a network, or the catalogue's countries to choose one from (550-APP-0082):
/// those whose code or name holds `find`.
pub enum Mode {
    List,
    Passphrase { ssid: Vec<u8>, security: Security, line: InputLine, remember: bool, problem: Option<&'static str> },
    Countries { all: Vec<Country>, find: InputLine, selected: usize },
}

/// The most pages of 32 countries the screen reads.
const PAGES: u16 = 16;

// The countries of `all` whose code or name holds `find`, any case.
fn found<'a>(all: &'a [Country], find: &str) -> Vec<&'a Country> {
    let find = find.to_ascii_lowercase();
    all.iter().filter(|c| find.is_empty() || c.code.to_ascii_lowercase().contains(&find) || crate::countries::name(&c.code).is_some_and(|n| n.to_ascii_lowercase().contains(&find))).collect()
}

/// The country as the screen names it: `UZ (Uzbekistan)`.
pub fn country_name(code: &str) -> String {
    match crate::countries::name(code) { Some(name) => format!("{} ({})", code, name), None => String::from(code) }
}

pub enum Flow { Quit, Continue }

pub struct Wifi {
    pub rows: Vec<Row>,
    pub selected: usize,
    pub mode: Mode,
    pub link: Link,
    /// The last thing done or refused, for the line under the list.
    pub notice: String,
    /// Why there is nothing to show (no station).
    pub problem: Option<String>,
    /// With the stand-in station (`--demo`): the title says so.
    pub demo: bool,
    /// In a window of `wm`, whose title bar already names `wifi`: the frame names only the demo.
    pub windowed: bool,
    // Remembering was asked and the station answered that it does not remember networks: said when joined.
    unkept: bool,
    /// The station's country (wifi.wit 1.1), read with each scan; None until read or with no station.
    pub country: Option<CountryStatus>,
}

// The line's bytes overwritten before it is emptied: a passphrase does not stay in memory after use.
fn wipe(line: &mut InputLine) {
    line.set(core::str::from_utf8(&[0u8; LINE_MAX]).unwrap_or(""));
    line.clear();
}

impl Wifi {
    pub fn new(demo: bool) -> Self {
        Self { rows: Vec::new(), selected: 0, mode: Mode::List, link: Link::Off, notice: String::new(), problem: None, demo, windowed: false, unkept: false, country: None }
    }

    fn refused(&mut self, refusal: Refusal) {
        match refusal {
            Refusal::NoStation(why) => self.problem = Some(why),
            other => self.notice = other.text(),
        }
    }

    /// A new scan: the rows again, the same name kept chosen.
    pub fn refresh(&mut self, station: &mut dyn Station) {
        let chosen = self.rows.get(self.selected).map(|r| r.ssid.clone());
        match station.scan() {
            Ok(found) => {
                self.rows = rows(&found);
                self.selected = chosen.and_then(|s| self.rows.iter().position(|r| r.ssid == s)).unwrap_or(0);
                self.problem = None;
            }
            Err(refusal) => self.refused(refusal),
        }
        self.country = station.country().ok();
    }

    // The catalogue, every page, for the country list; why not as the line under the list.
    fn open_countries(&mut self, station: &mut dyn Station) {
        let (mut all, mut from) = (Vec::new(), 0u16);
        for _ in 0..PAGES {
            match station.countries(from) {
                Ok((total, page)) => {
                    from += page.len() as u16;
                    all.extend(page);
                    if from >= total || from == 0 { break; }
                }
                Err(Refusal::Unsupported) => { self.notice = String::from("No catalogue in this station: it runs without a country, as before"); return; }
                Err(refusal) => return self.refused(refusal),
            }
        }
        // The cursor on the country applied, if any.
        let applied = self.country.as_ref().map(|c| c.applied.clone()).unwrap_or_default();
        let selected = all.iter().position(|c| c.code == applied).unwrap_or(0);
        self.mode = Mode::Countries { all, find: InputLine::new(), selected };
    }

    // Asking for country `code`: the station leaves the network first; the line says what it applied.
    fn choose_country(&mut self, station: &mut dyn Station, code: &str) {
        match station.set_country(code) {
            Ok(status) => {
                self.notice = match status.state {
                    CountryState::Applied => format!("Country {}: applied{}", country_name(&status.applied), if status.kept { ", kept" } else { ", not kept across restarts" }),
                    CountryState::Failed => format!("Country {}: the radio refused its limits, nothing applied", country_name(&status.asked)),
                    CountryState::None => format!("Country {}: not applied", country_name(code)),
                };
                self.country = Some(status);
                self.poll(station);
            }
            Err(Refusal::Denied) => self.notice = String::from("wifi's client of the station only reads: the country needs the change badge"),
            Err(Refusal::NotFound) => self.notice = format!("{} is not a country of the catalogue", code),
            Err(Refusal::Unsupported) => self.notice = String::from("No catalogue in this station: it runs without a country, as before"),
            Err(refusal) => self.refused(refusal),
        }
    }

    /// The link's state; a line when joining ends or the link goes.
    pub fn poll(&mut self, station: &mut dyn Station) {
        match station.status() {
            Ok(link) => {
                if link != self.link {
                    match &link {
                        Link::Joined { ssid, .. } => {
                            self.notice = format!("Joined {}{}", shown(ssid), if self.unkept { ": the station does not remember networks yet, so it did not keep the passphrase" } else { "" });
                            self.unkept = false;
                        }
                        Link::Failed { ssid, reason } => self.notice = format!("{} not joined: {}", shown(ssid), reason.text()),
                        Link::Lost { ssid } => self.notice = format!("The link to {} was lost: the station is joining it again", shown(ssid)),
                        _ => {}
                    }
                    // A remembered network shows its mark at once.
                    if let Link::Joined { .. } = link { self.refresh(station); }
                }
                self.link = link;
            }
            Err(refusal) => self.refused(refusal),
        }
    }

    // Joining `ssid`, with the station's answer as the line; a station that does not remember is asked again without.
    fn join(&mut self, station: &mut dyn Station, ssid: &[u8], passphrase: &str, remember: bool) {
        let mut answer = station.join(ssid, passphrase, remember);
        self.unkept = remember && answer == Err(Refusal::Unsupported);
        if self.unkept { answer = station.join(ssid, passphrase, false); }
        match answer {
            Ok(()) => { self.notice = format!("Joining {}…", shown(ssid)); self.poll(station); }
            Err(refusal) => { self.unkept = false; self.refused(refusal) }
        }
    }

    /// A key: ↑↓ choose, Enter joins, r scans again, f forgets, d leaves, q or Esc ends; while a passphrase is typed,
    /// Tab ticks "remember", Enter joins, Esc goes back.
    pub fn key(&mut self, key: Key, station: &mut dyn Station) -> Flow {
        // The country list: ↑↓ PgUp PgDn choose, typing finds, Enter asks for the one chosen, Esc goes back.
        if let Mode::Countries { all, find, selected } = &mut self.mode {
            let shown = found(all, find.as_str()).len();
            match key.code() {
                Code::Up => *selected = selected.saturating_sub(1),
                Code::Down => *selected = (*selected + 1).min(shown.saturating_sub(1)),
                Code::PageUp => *selected = selected.saturating_sub(10),
                Code::PageDown => *selected = (*selected + 10).min(shown.saturating_sub(1)),
                Code::Enter => {
                    let Some(code) = found(all, find.as_str()).get(*selected).map(|c| c.code.clone()) else { return Flow::Continue };
                    self.mode = Mode::List;
                    self.choose_country(station, &code);
                }
                _ => match find.key(key) {
                    Edit::Cancel => { self.mode = Mode::List; self.notice = String::from("The country stays as it was"); }
                    Edit::Changed => *selected = 0,
                    _ => {}
                },
            }
            return Flow::Continue;
        }
        if let Mode::Passphrase { security, line, remember, problem, .. } = &mut self.mode {
            match key.code() {
                Code::Tab => *remember = !*remember,
                _ => match line.key(key) {
                    Edit::Cancel => { wipe(line); self.mode = Mode::List; self.notice = String::from("Not joined"); }
                    Edit::Submit => match check(*security, line.as_str()) {
                        Err(why) => *problem = Some(why),
                        Ok(()) => if let Mode::Passphrase { ssid, mut line, remember, .. } = core::mem::replace(&mut self.mode, Mode::List) {
                            self.join(station, &ssid, line.as_str(), remember);
                            wipe(&mut line);
                        },
                    },
                    Edit::Changed => *problem = None,
                    _ => {}
                },
            }
            return Flow::Continue;
        }
        if self.problem.is_some() {
            return if key.is_escape() || key.code() == Code::Enter || key.char() == Some('q') { Flow::Quit } else { Flow::Continue };
        }
        match key.code() {
            Code::Up => self.selected = self.selected.saturating_sub(1),
            Code::Down => self.selected = (self.selected + 1).min(self.rows.len().saturating_sub(1)),
            Code::Esc => return Flow::Quit,
            Code::F(5) => { self.refresh(station); self.notice = format!("{} networks", self.rows.len()); }
            Code::Enter => {
                let Some(row) = self.rows.get(self.selected).cloned() else { return Flow::Continue };
                let name = shown(&row.ssid);
                if row.ssid.is_empty() { self.notice = String::from("A hidden network: joining one by its name is not done yet"); return Flow::Continue; }
                if matches!(&self.link, Link::Joined { ssid, .. } if *ssid == row.ssid) { self.notice = format!("{} is joined already (d leaves it)", name); return Flow::Continue; }
                if !matches!(row.security, Security::Open | Security::Wpa2) {
                    self.notice = format!("{}: {}", name, check(row.security, "").err().unwrap_or("not joined"));
                } else if row.security == Security::Open || row.remembered {
                    self.join(station, &row.ssid, "", false);
                } else {
                    self.mode = Mode::Passphrase { ssid: row.ssid, security: row.security, line: InputLine::new(), remember: true, problem: None };
                }
            }
            _ => match key.char() {
                Some('q') => return Flow::Quit,
                Some('r') => { self.refresh(station); self.notice = format!("{} networks", self.rows.len()); }
                Some('c') => self.open_countries(station),
                Some('d') => match station.leave() { Ok(()) => { self.notice = String::from("Left the network"); self.poll(station); } Err(r) => self.refused(r) },
                Some('f') => {
                    let Some(ssid) = self.rows.get(self.selected).map(|r| r.ssid.clone()) else { return Flow::Continue };
                    match station.forget(&ssid) {
                        Ok(true) => { self.notice = format!("{} forgotten: its key is gone from the station", shown(&ssid)); self.refresh(station); self.poll(station); }
                        Ok(false) => self.notice = format!("{} is not remembered", shown(&ssid)),
                        Err(Refusal::Unsupported) => self.notice = String::from("The station does not remember networks yet"),
                        Err(r) => self.refused(r),
                    }
                }
                _ => {}
            },
        }
        Flow::Continue
    }

    /// The state for the serial log: the mode, the rows, the chosen name and the link.
    pub fn status(&self) -> String {
        let link = match &self.link {
            Link::Off => String::from("OFF"),
            Link::Scanning => String::from("SCANNING"),
            Link::Joining { ssid } => format!("JOINING {}", shown(ssid)),
            Link::Joined { ssid, bssid, .. } => format!("JOINED {} {}", shown(ssid), mac(*bssid)),
            Link::Lost { ssid } => format!("LOST {}", shown(ssid)),
            Link::Failed { ssid, reason } => format!("FAILED {} {:?}", shown(ssid), reason),
        };
        let mode = match &self.mode {
            Mode::List => String::from("LIST"),
            Mode::Passphrase { ssid, remember, .. } => format!("PASSPHRASE {} REMEMBER={}", shown(ssid), remember), // never the passphrase or its length
            Mode::Countries { all, find, selected } => {
                let list = found(all, find.as_str());
                format!("COUNTRIES {} OF {} AT {}", list.len(), all.len(), list.get(*selected).map_or("-", |c| c.code.as_str()))
            }
        };
        let at = self.rows.get(self.selected).map_or(String::from("-"), |r| if r.ssid.is_empty() { String::from("(hidden)") } else { shown(&r.ssid) });
        let country = match &self.country {
            Some(c) if c.state == CountryState::Applied => c.applied.clone(),
            Some(c) if c.state == CountryState::Failed => format!("FAILED-{}", c.asked),
            Some(c) if !c.catalogue.is_empty() => String::from("NONE"),
            _ => String::from("-"),
        };
        format!("MODE={} ROWS={} AT={} LINK={} COUNTRY={}", mode, self.rows.len(), at, link, country)
    }

    /// Draws the screen; returns where the text cursor is (in the passphrase's field).
    pub fn draw(&self, grid: &mut Grid, theme: &Theme) -> Option<(usize, usize)> {
        let (cols, rows) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        let area = grid.area();
        match (self.windowed, self.demo) {
            (false, demo) => grid.frame_titled(area, Line::Single, if demo { " wifi — DEMO: a stand-in station, no radio " } else { " wifi " }, theme.frame, theme.header),
            (true, true) => grid.frame_titled(area, Line::Single, " DEMO: a stand-in station, no radio ", theme.frame, theme.header),
            (true, false) => grid.frame(area, Line::Single, theme.frame),
        }
        let w = cols.saturating_sub(4);
        if let Some(why) = &self.problem {
            grid.text_max(2, 2, "No Wi-Fi station:", w, theme.error);
            grid.text_max(2, 3, why, w, theme.panel);
            grid.text_max(2, 5, "wifi --demo shows this program with a stand-in station.", w, theme.dim);
            grid.text_max(2, rows.saturating_sub(2), "Esc, Enter or q: quit", w, theme.dim);
            return None;
        }
        let link = match &self.link {
            Link::Off => String::from("Not joined to a network"),
            Link::Scanning => String::from("Scanning…"),
            Link::Joining { ssid } => format!("Joining {}…", shown(ssid)),
            Link::Joined { ssid, bssid, channel, weakness, rate: units } => {
                format!("Joined {}: {} dBm, channel {}, {} Mb/s, access point {}", shown(ssid), dbm(*weakness), channel, rate(*units), mac(*bssid))
            }
            Link::Lost { ssid } => format!("The link to {} was lost: joining it again…", shown(ssid)),
            Link::Failed { ssid, reason } => format!("{} not joined: {}", shown(ssid), reason.text()),
        };
        grid.text_max(2, 1, &link, w, if matches!(self.link, Link::Failed { .. } | Link::Lost { .. }) { theme.error } else { theme.header });
        if let Link::Joined { .. } = self.link { grid.text_max(2, 2, "The address is the network stack's: ip shows it", w, theme.dim); }
        grid.text_max(2, 3, &format!("{:<32} {:<6} {:>5} {:>3} {:<10} {}", "NETWORK", "SIGNAL", "DBM", "CH", "SECURITY", ""), w, theme.dim);
        let top = 4;
        let room = rows.saturating_sub(top + 5);
        let first = self.selected.saturating_sub(room.saturating_sub(1));
        for (i, row) in self.rows.iter().enumerate().skip(first).take(room) {
            let joined = matches!(&self.link, Link::Joined { ssid, .. } if !row.ssid.is_empty() && *ssid == row.ssid);
            let name: String = if row.ssid.is_empty() { String::from("(hidden network)") } else { shown(&row.ssid).chars().take(32).collect() };
            let strength: String = (0..4).map(|b| if b < bars(row.weakness) { '█' } else { '░' }).collect();
            let marks = [(joined, "joined"), (row.remembered, "remembered")].iter().filter(|(on, _)| *on).map(|(_, t)| *t).collect::<Vec<_>>().join(", ");
            let points = if row.points > 1 { format!(" ({} access points)", row.points) } else { String::new() };
            let line = format!("{:<32} {:<6} {:>5} {:>3} {:<10} {}{}", name, strength, dbm(row.weakness), row.channel, row.security.name(), marks, points);
            let style = if i == self.selected { theme.selected } else if joined { theme.accent } else { theme.panel };
            grid.text_padded(2, top + i - first, &line, w, style);
        }
        if self.rows.is_empty() { grid.text_max(2, top, "No networks heard yet: r scans again", w, theme.dim); }
        let mut cursor = None;
        if let Mode::Passphrase { ssid, security, line, remember, problem } = &self.mode {
            let (bw, bh) = (60.min(cols), 8);
            let (bx, by) = ((cols - bw) / 2, (rows.saturating_sub(bh)) / 2);
            let r = crate::tui::Rect::new(bx, by, bw, bh);
            grid.fill(r, ' ', theme.dialog);
            grid.frame_titled(r, Line::Double, &format!(" {} ({}) ", shown(ssid), security.name()), theme.dialog_frame, theme.dialog_frame);
            grid.text_max(bx + 2, by + 1, "Passphrase (not shown as it is typed):", bw - 4, theme.dialog);
            let typed: String = core::iter::repeat('•').take(line.as_str().chars().count()).collect();
            grid.text_padded(bx + 2, by + 2, &typed, bw - 4, theme.input);
            cursor = Some((bx + 2 + line.cursor_chars().min(bw - 5), by + 2));
            grid.text_max(bx + 2, by + 3, &format!("[{}] remember it (the station keeps it in keystore)", if *remember { 'x' } else { ' ' }), bw - 4, theme.dialog);
            if let Some(why) = problem { grid.text_max(bx + 2, by + 4, why, bw - 4, theme.error); }
            grid.text_max(bx + 2, by + bh - 2, "Enter: join · Tab: remember · Esc: back", bw - 4, theme.dialog);
        }
        if let Mode::Countries { all, find, selected } = &self.mode {
            let list = found(all, find.as_str());
            let (bw, bh) = (64.min(cols), rows.saturating_sub(4).clamp(8, 22));
            let (bx, by) = ((cols - bw) / 2, (rows.saturating_sub(bh)) / 2);
            let r = crate::tui::Rect::new(bx, by, bw, bh);
            grid.fill(r, ' ', theme.dialog);
            grid.frame_titled(r, Line::Double, &format!(" The country: {} of {} ", list.len(), all.len()), theme.dialog_frame, theme.dialog_frame);
            grid.text_max(bx + 2, by + 1, "Find (code or name):", bw - 4, theme.dialog);
            grid.text_padded(bx + 23, by + 1, find.as_str(), bw.saturating_sub(25), theme.input);
            cursor = Some((bx + 23 + find.cursor_chars().min(bw.saturating_sub(26)), by + 1));
            let room = bh.saturating_sub(5);
            let first = selected.saturating_sub(room.saturating_sub(1));
            for (i, c) in list.iter().enumerate().skip(first).take(room) {
                let none = if c.send == 0 && c.listen == 0 { "  none usable" } else { "" };
                let name: String = crate::countries::name(&c.code).unwrap_or("").chars().take(26).collect();
                let line = format!("{} {:<26} send {:>2} listen {:>2}{}", c.code, name, c.send, c.listen, none);
                grid.text_padded(bx + 2, by + 2 + i - first, &line, bw - 4, if i == *selected { theme.selected } else { theme.dialog });
            }
            if list.is_empty() { grid.text_max(bx + 2, by + 2, "No country holds that", bw - 4, theme.dim); }
            grid.text_max(bx + 2, by + bh - 2, "Enter: ask for it (the network is left first) · Esc: back", bw - 4, theme.dialog);
        }
        let country = match &self.country {
            Some(c) if c.catalogue.is_empty() => String::from("Country: no catalogue in this station"),
            Some(c) if c.state == CountryState::Applied && c.asked == c.applied => format!("Country: {}", country_name(&c.applied)),
            Some(c) if c.state == CountryState::Applied => format!("Country: {}, {} asked", country_name(&c.applied), country_name(&c.asked)),
            Some(c) if c.state == CountryState::Failed => format!("Country: {} asked, the radio refused its limits: none applied (c: another)", country_name(&c.asked)),
            Some(_) => String::from("Country: none asked yet (c: choose one)"),
            None => String::new(),
        };
        grid.text_max(2, rows.saturating_sub(4), &country, w, theme.dim);
        grid.text_max(2, rows.saturating_sub(3), &self.notice, w, theme.accent);
        let help = "↑↓: choose · Enter: join · r or F5: scan · d: leave · f: forget · c: country · q or Esc: quit";
        grid.text_max(2, rows.saturating_sub(2), help, w, theme.dim);
        cursor
    }
}
