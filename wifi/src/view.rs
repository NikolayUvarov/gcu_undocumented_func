//! `wifi`'s screen (550-APP-0033): the networks a scan found, one row a name with its strongest access point, the
//! strongest first; Enter joins one (a passphrase typed without echo for a protected one, kept by the station if
//! asked); the link's state at the top. Only the drawing and the keys: the program gives it a `Station`, the host test
//! the stand-in (tests/wifi_host.rs).
use crate::keys::{Code, Key};
use crate::station::{check, dotted, Link, Network, Refusal, Security, Station};
use crate::tui::widgets::{Edit, InputLine, LINE_MAX};
use crate::tui::{Grid, Line, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// A row of the list: a network's name (empty: hidden), its strongest access point's signal and channel, how many
/// access points share the name, its protection, and whether the station remembers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row { pub ssid: String, pub signal: i8, pub channel: u8, pub points: usize, pub security: Security, pub remembered: bool }

/// The rows of a scan: one a name, the strongest first; hidden networks last, each a row.
pub fn rows(networks: &[Network]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for n in networks {
        match rows.iter_mut().find(|r| !n.ssid.is_empty() && r.ssid == n.ssid) {
            Some(r) => {
                r.points += 1;
                r.remembered |= n.remembered;
                if n.signal > r.signal { (r.signal, r.channel, r.security) = (n.signal, n.channel, n.security); }
            }
            None => rows.push(Row { ssid: n.ssid.clone(), signal: n.signal, channel: n.channel, points: 1, security: n.security, remembered: n.remembered }),
        }
    }
    rows.sort_by(|a, b| a.ssid.is_empty().cmp(&b.ssid.is_empty()).then(b.signal.cmp(&a.signal)));
    rows
}

/// Bars of signal, 0 to 4, from dBm.
pub fn bars(signal: i8) -> usize {
    match signal { s if s >= -55 => 4, s if s >= -67 => 3, s if s >= -75 => 2, s if s >= -85 => 1, _ => 0 }
}

/// The list, or the passphrase being typed for a network.
pub enum Mode { List, Passphrase { ssid: String, security: Security, line: InputLine, remember: bool, problem: Option<&'static str> } }

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
}

// The line's bytes overwritten before it is emptied: a passphrase does not stay in memory after use.
fn wipe(line: &mut InputLine) {
    line.set(core::str::from_utf8(&[0u8; LINE_MAX]).unwrap_or(""));
    line.clear();
}

impl Wifi {
    pub fn new(demo: bool) -> Self {
        Self { rows: Vec::new(), selected: 0, mode: Mode::List, link: Link::Off, notice: String::new(), problem: None, demo }
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
    }

    /// The link's state; a line when joining ends.
    pub fn poll(&mut self, station: &mut dyn Station) {
        match station.status() {
            Ok(link) => {
                if link != self.link {
                    match &link {
                        Link::Joined { ssid, address, .. } => self.notice = format!("Joined {}{}", ssid, if *address == 0 { String::from(", waiting for an address") } else { format!(": {}", dotted(*address)) }),
                        Link::Failed { ssid, reason } => self.notice = format!("{} not joined: {}", ssid, reason.text()),
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

    // Joining `ssid`, with the station's answer as the line.
    fn join(&mut self, station: &mut dyn Station, ssid: &str, passphrase: Option<&str>, remember: bool) {
        match station.join(ssid, passphrase, remember) {
            Ok(()) => { self.notice = format!("Joining {}…", ssid); self.poll(station); }
            Err(refusal) => self.refused(refusal),
        }
    }

    /// A key: ↑↓ choose, Enter joins, r scans again, f forgets, d leaves, q or Esc ends; while a passphrase is typed,
    /// Tab ticks "remember", Enter joins, Esc goes back.
    pub fn key(&mut self, key: Key, station: &mut dyn Station) -> Flow {
        if let Mode::Passphrase { ssid, security, line, remember, problem } = &mut self.mode {
            match key.code() {
                Code::Tab => *remember = !*remember,
                _ => match line.key(key) {
                    Edit::Cancel => { wipe(line); self.mode = Mode::List; self.notice = String::from("Not joined"); }
                    Edit::Submit => match check(*security, line.as_str()) {
                        Err(why) => *problem = Some(why),
                        Ok(()) => {
                            let (name, typed, keep) = (ssid.clone(), String::from(line.as_str()), *remember);
                            wipe(line);
                            self.mode = Mode::List;
                            self.join(station, &name, Some(&typed), keep);
                        }
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
                if row.ssid.is_empty() { self.notice = String::from("A hidden network: joining one by its name is not done yet"); return Flow::Continue; }
                if matches!(&self.link, Link::Joined { ssid, .. } if *ssid == row.ssid) { self.notice = format!("{} is joined already (d leaves it)", row.ssid); return Flow::Continue; }
                if matches!(row.security, Security::Wep | Security::Enterprise) {
                    self.notice = format!("{}: {}", row.ssid, check(row.security, "").err().unwrap_or("not joined"));
                } else if row.security == Security::Open || row.remembered {
                    self.join(station, &row.ssid, None, false);
                } else {
                    self.mode = Mode::Passphrase { ssid: row.ssid, security: row.security, line: InputLine::new(), remember: true, problem: None };
                }
            }
            _ => match key.char() {
                Some('q') => return Flow::Quit,
                Some('r') => { self.refresh(station); self.notice = format!("{} networks", self.rows.len()); }
                Some('d') => match station.leave() { Ok(()) => { self.notice = String::from("Left the network"); self.poll(station); } Err(r) => self.refused(r) },
                Some('f') => {
                    let Some(ssid) = self.rows.get(self.selected).map(|r| r.ssid.clone()) else { return Flow::Continue };
                    match station.forget(&ssid) {
                        Ok(true) => { self.notice = format!("{} forgotten: its passphrase is gone from the station", ssid); self.refresh(station); self.poll(station); }
                        Ok(false) => self.notice = format!("{} is not remembered", ssid),
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
            Link::Joining { ssid } => format!("JOINING {}", ssid),
            Link::Joined { ssid, address, .. } => format!("JOINED {} {}", ssid, dotted(*address)),
            Link::Failed { ssid, reason } => format!("FAILED {} {:?}", ssid, reason),
        };
        let mode = match &self.mode { Mode::List => String::from("LIST"), Mode::Passphrase { ssid, remember, .. } => format!("PASSPHRASE {} REMEMBER={}", ssid, remember) }; // never the passphrase or its length
        format!("MODE={} ROWS={} AT={} LINK={}", mode, self.rows.len(), self.rows.get(self.selected).map_or("-", |r| if r.ssid.is_empty() { "(hidden)" } else { r.ssid.as_str() }), link)
    }

    /// Draws the screen; returns where the text cursor is (in the passphrase's field).
    pub fn draw(&self, grid: &mut Grid, theme: &Theme) -> Option<(usize, usize)> {
        let (cols, rows) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        let area = grid.area();
        grid.frame_titled(area, Line::Single, if self.demo { " wifi — DEMO: a stand-in station, no radio " } else { " wifi " }, theme.frame, theme.header);
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
            Link::Joining { ssid } => format!("Joining {}…", ssid),
            Link::Joined { ssid, address, signal } => format!("Joined {} ({} dBm), address {}", ssid, signal, if *address == 0 { String::from("not yet") } else { dotted(*address) }),
            Link::Failed { ssid, reason } => format!("{} not joined: {}", ssid, reason.text()),
        };
        grid.text_max(2, 1, &link, w, if matches!(self.link, Link::Failed { .. }) { theme.error } else { theme.header });
        grid.text_max(2, 3, &format!("{:<32} {:<6} {:>7} {:>3} {:<10} {}", "NETWORK", "SIGNAL", "DBM", "CH", "SECURITY", ""), w, theme.dim);
        let top = 4;
        let room = rows.saturating_sub(top + 4);
        let first = self.selected.saturating_sub(room.saturating_sub(1));
        for (i, row) in self.rows.iter().enumerate().skip(first).take(room) {
            let joined = matches!(&self.link, Link::Joined { ssid, .. } if !row.ssid.is_empty() && *ssid == row.ssid);
            let name = if row.ssid.is_empty() { String::from("(hidden network)") } else { row.ssid.clone() };
            let strength: String = (0..4).map(|b| if b < bars(row.signal) { '█' } else { '░' }).collect();
            let marks = [(joined, "joined"), (row.remembered, "remembered"), (row.points > 1, "")].iter().filter(|(on, text)| *on && !text.is_empty()).map(|(_, t)| *t).collect::<Vec<_>>().join(", ");
            let points = if row.points > 1 { format!(" ({} access points)", row.points) } else { String::new() };
            let line = format!("{:<32} {:<6} {:>7} {:>3} {:<10} {}{}", name, strength, row.signal, row.channel, row.security.name(), marks, points);
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
            grid.frame_titled(r, Line::Double, &format!(" {} ({}) ", ssid, security.name()), theme.dialog_frame, theme.dialog_frame);
            grid.text_max(bx + 2, by + 1, "Passphrase (not shown as it is typed):", bw - 4, theme.dialog);
            let typed: String = core::iter::repeat('•').take(line.as_str().chars().count()).collect();
            grid.text_padded(bx + 2, by + 2, &typed, bw - 4, theme.input);
            cursor = Some((bx + 2 + line.cursor_chars().min(bw - 5), by + 2));
            grid.text_max(bx + 2, by + 3, &format!("[{}] remember it (the station keeps it in keystore)", if *remember { 'x' } else { ' ' }), bw - 4, theme.dialog);
            if let Some(why) = problem { grid.text_max(bx + 2, by + 4, why, bw - 4, theme.error); }
            grid.text_max(bx + 2, by + bh - 2, "Enter: join · Tab: remember · Esc: back", bw - 4, theme.dialog);
        }
        grid.text_max(2, rows.saturating_sub(3), &self.notice, w, theme.accent);
        let help = "↑↓: choose · Enter: join · r or F5: scan · d: leave · f: forget · q or Esc: quit";
        grid.text_max(2, rows.saturating_sub(2), help, w, theme.dim);
        cursor
    }
}

/// What `wifi list` prints: a line a network.
pub fn table(rows: &[Row]) -> String {
    let mut out = format!("{:<32} {:>4} {:>3} {:<10} {}\n", "NETWORK", "DBM", "CH", "SECURITY", "ACCESS POINTS");
    for row in rows {
        let name = if row.ssid.is_empty() { "(hidden network)" } else { row.ssid.as_str() };
        out.push_str(&format!("{:<32} {:>4} {:>3} {:<10} {}{}\n", name, row.signal, row.channel, row.security.name(), row.points, if row.remembered { "  remembered" } else { "" }));
    }
    out.push_str(&format!("{} networks", rows.len()));
    out
}
