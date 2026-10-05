//! The file manager: two panels over the boot disk and the RAM disk, the built-in viewer and editor, quick view,
//! information, find, copy/move/mkdir/delete with progress, the menu and the key bar (docs/tools §4.1). The disk is
//! reached through `Disk` (vfs_server in the system, memory in tests).
use crate::panel::{VFS_ENTRY_ARCHIVE, VFS_ENTRY_HIDDEN, VFS_ENTRY_READ_ONLY, VFS_ENTRY_SYSTEM};
use crate::editor::{Editor, Outcome as EditOutcome};
use crate::abi::{KEY_DOWN, KEY_F1, KEY_UP, POINTER_LEFT, POINTER_RIGHT};
use crate::keys::{self, Code, Key};
use crate::panel::{self, display, inside, is_root, join, matches, parent, resolve, same_volume, Entry, Mode, Panel, Sort};
use crate::tui::viewer::{Action, Source, Viewer};
use crate::tui::widgets::{buttons_key, dialog, fkey_bar, input_dialog, message, progress, Edit, InputLine, KeyBars, ListState, MenuAction, MenuBar};
use crate::tui::{Grid, Line, Rect, Theme};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Why a change failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure { Exists, NotEmpty, Denied, NoSpace, NotFound, Other(String) }

impl Failure {
    pub fn text(&self) -> String {
        match self {
            Failure::Exists => String::from("it already exists"),
            Failure::NotEmpty => String::from("the directory is not empty"),
            Failure::Denied => String::from("denied (only ram: and data/ are writable)"),
            Failure::NoSpace => String::from("the disk is full"),
            Failure::NotFound => String::from("not found"),
            Failure::Other(text) => text.clone(),
        }
    }
}

/// A file being written; dropping it ends the write.
pub trait Sink { fn write(&mut self, data: &[u8]) -> Result<(), Failure>; }

/// A mounted volume as the information panel and the volume menu show it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VolumeInfo { pub label: String, pub fat_bits: u8, pub bytes: u64, pub free: u64 }

/// The file system as the file manager sees it. Paths name a volume first (`ram:docs`) or are on the boot disk.
pub trait Disk {
    /// Entries of a directory (`""` and `"ram:"` are the roots).
    fn list(&mut self, path: &str) -> Result<Vec<Entry>, String>;
    /// A file to read.
    fn open(&mut self, path: &str) -> Option<Box<dyn Source>>;
    /// Starts a program with arguments; says where it shows itself.
    fn run(&mut self, path: &str, args: &str) -> Result<Started, String>;
    /// A file to write: an existing one is emptied if `replace`, else `Failure::Exists`.
    fn create(&mut self, path: &str, replace: bool) -> Result<Box<dyn Sink>, Failure>;
    /// Makes a directory and missing parents (an existing one is fine).
    fn mkdir(&mut self, path: &str) -> Result<(), Failure>;
    /// Removes a file or an empty directory.
    fn remove(&mut self, path: &str) -> Result<(), Failure>;
    /// Renames or moves within a volume.
    fn rename(&mut self, from: &str, to: &str) -> Result<(), Failure>;
    /// The file may be changed in place.
    fn writable(&mut self, path: &str) -> bool;
    /// The volume of a path.
    fn volume(&mut self, path: &str) -> Option<VolumeInfo>;
    /// Writes what is cached for the volume of `path` to the disk.
    fn flush(&mut self, path: &str);
}

/// Where a program fm started shows itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// A window of its own next to fm's (fm runs in a window of `wm`, issue 099).
    Window,
    /// A screen of its own, in the background: fm cannot bring it to the front (Ctrl+Z, then FG in the shell).
    Screen,
    /// No screen: a console program, its output in its log.
    Console,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Started { pub pid: u64, pub place: Place }

impl Started {
    /// What fm says after starting `name`.
    pub fn text(&self, name: &str) -> String {
        let pid = self.pid;
        match self.place {
            Place::Window => format!("Started {} (PID {}) in a window of its own", name, pid),
            Place::Screen => format!("Started {} as PID {} in the background: Ctrl+Z, then FG {} in the shell shows it", name, pid, pid),
            Place::Console => format!("Started {} as PID {}: a console program, LOGS {} in the shell shows what it printed", name, pid, pid),
        }
    }
}

/// A boxed source as a `Source`.
pub struct Boxed(pub Box<dyn Source>);
impl Source for Boxed {
    fn size(&self) -> u64 { self.0.size() }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize { self.0.read(offset, out) }
}

/// The first bytes of a file, for quick view.
struct Bytes<'a>(&'a [u8]);
impl Source for Bytes<'_> {
    fn size(&self) -> u64 { self.0.len() as u64 }
    fn read(&mut self, offset: u64, out: &mut [u8]) -> usize {
        let start = (offset as usize).min(self.0.len());
        let n = out.len().min(self.0.len() - start);
        out[..n].copy_from_slice(&self.0[start..start + n]);
        n
    }
}

const PREVIEW: usize = 8 * 1024;
const OUTPUT_MAX: usize = 200; // lines of the command line's output kept
const FIND_MAX: usize = 500;
/// Bytes copied per step of a job (the screen and the keys are served between steps).
pub const SLICE: usize = 64 * 1024;
const VOLUMES: [(&str, &str); 2] = [("", "A: boot disk"), ("ram:", "ram: RAM disk")];

const MENU_TITLES: [&str; 5] = ["Left", "Files", "Commands", "Options", "Right"];
const PANEL_ITEMS: [&str; 11] = ["Brief", "Full", "Info", "Quick view", "Name", "Extension", "Time", "Size", "Reverse order", "Reread  Ctrl+R", "Volume  Alt+F1/F2"];
const FILES_ITEMS: [&str; 11] = ["View  F3", "Edit  F4", "New file  Shift+F4", "Copy  F5", "Move or rename  F6", "Make directory  F7", "Delete  F8",
                                 "Run or open  Enter", "Select group  +", "Unselect group  -", "Invert selection  *"];
const COMMAND_ITEMS: [&str; 2] = ["Find file  Alt+F7", "Swap panels  Ctrl+U"];
const OPTION_ITEMS: [&str; 1] = ["Hidden and system files  Ctrl+H"];
const MENU_ITEMS: [&[&str]; 5] = [&PANEL_ITEMS, &FILES_ITEMS, &COMMAND_ITEMS, &OPTION_ITEMS, &PANEL_ITEMS];
const KEYS: KeyBars<'static> = KeyBars { plain: ["Help", "", "View", "Edit", "Copy", "RenMov", "Mkdir", "Delete", "PullDn", "Quit"],
                                         shift: ["", "", "", "New", "", "", "", "", "", ""], ctrl: ["LPanel", "RPanel", "Name", "Ext", "Time", "Size", "", "", "", ""],
                                         alt: ["Left", "Right", "", "", "", "", "Find", "", "", ""] };

const HELP: [&str; 19] = [
    "Typing goes to the command line: Enter runs it — cd <dir>, edit or view <file>,",
    "  a program with arguments; Esc clears it; Alt+Enter adds the name under the cursor",
    "Ctrl+O — hide or show the panels; Ctrl+F1 / Ctrl+F2 — the left / right panel;",
    "  Ctrl+P — the other panel",
    "Tab — other panel; Enter — open a directory, run a program, view a file",
    "Backspace — parent directory; ↑ ↓ PgUp PgDn Home End (← → in brief mode)",
    "F3 — view; F4 — edit (Shift+F4: a new file); F9 — menu; F10 or Esc — quit",
    "F5 — copy, F6 — move or rename, F7 — make a directory, F8 — delete:",
    "  the marked files or the one under the cursor; Esc stops an operation",
    "Ins — mark; + / - — mark or unmark by mask; * — invert",
    "Ctrl+F3 / F4 / F5 / F6 — sort by name / extension / time / size",
    "Ctrl+H — hidden files; Ctrl+R — reread; Ctrl+U — swap panels",
    "Ctrl+L — information, Ctrl+Q — quick view in the other panel",
    "Alt+F1 / Alt+F2 — volume of the left / right panel; Alt+F7 — find",
    "Mouse: click — cursor; double click — open; right click — mark;",
    "  wheel — move the cursor; a click on the key bar presses that key",
    "Programs started here open a window of their own under wm; on a screen they run in the",
    "background (FG <pid> in the shell). Only ram: and data/ on the boot disk are writable.",
    "",
];

/// What a job does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op { Copy, Move, Delete }

impl Op {
    fn name(self) -> &'static str { match self { Op::Copy => "COPY", Op::Move => "MOVE", Op::Delete => "DELETE" } }
    fn title(self) -> &'static str { match self { Op::Copy => "Copying", Op::Move => "Moving", Op::Delete => "Deleting" } }
    fn done(self) -> &'static str { match self { Op::Copy => "Copied", Op::Move => "Moved", Op::Delete => "Deleted" } }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step { MakeDir(String), Copy { from: String, to: String, size: u64 }, Remove(String), Rename { from: String, to: String } }

impl Step {
    fn describe(&self) -> String {
        match self {
            Step::MakeDir(path) => format!("Make {}", display(path)),
            Step::Copy { from, to, .. } => format!("Copy {} to {}", display(from), display(to)),
            Step::Remove(path) => format!("Delete {}", display(path)),
            Step::Rename { from, to } => format!("Move {} to {}", display(from), display(to)),
        }
    }
}

struct Copying { source: Box<dyn Source>, sink: Box<dyn Sink>, offset: u64, size: u64 }

/// A copy, move or delete in progress: steps planned up front, done a slice at a time.
pub struct Job {
    pub op: Op,
    steps: Vec<Step>,
    pub index: usize,
    copying: Option<Copying>,
    /// Bytes copied and to copy.
    pub done: u64,
    pub total: u64,
    pub files: usize,
    pub skipped: usize,
    pub errors: usize,
    replace: bool,
    /// A failed step waiting for Retry / Skip / Abort, and the selected button.
    pub failure: Option<Failure>,
    pub choice: usize,
    /// Esc was pressed: asking whether to stop.
    pub asking: bool,
    kept: Vec<String>, // sources of a move whose copy failed: not removed
    sources: Vec<String>,
    target: Option<String>,
    focus: Option<String>,
    buffer: Vec<u8>,
}

impl Job {
    fn new(op: Op) -> Self {
        Self { op, steps: Vec::new(), index: 0, copying: None, done: 0, total: 0, files: 0, skipped: 0, errors: 0, replace: true, failure: None, choice: 0,
               asking: false, kept: Vec::new(), sources: Vec::new(), target: None, focus: None, buffer: Vec::new() }
    }
    pub fn steps(&self) -> usize { self.steps.len() }

    // The steps for `from` (a file or a directory tree) copied to `to`.
    fn plan_copy(&mut self, from: &str, to: &str, entry: &Entry, disk: &mut dyn Disk, depth: usize) -> Result<(), String> {
        if !entry.dir { self.total += entry.size; self.steps.push(Step::Copy { from: from.into(), to: to.into(), size: entry.size }); return Ok(()); }
        if depth > 32 { return Err(format!("{} is nested too deep", display(from))); }
        self.steps.push(Step::MakeDir(to.into()));
        for child in disk.list(from).map_err(|e| format!("{}: {}", display(from), e))? {
            self.plan_copy(&join(from, &child.name), &join(to, &child.name), &child, disk, depth + 1)?;
        }
        Ok(())
    }

    // The steps that remove `path`: what is inside a directory first.
    fn plan_remove(&mut self, path: &str, entry: &Entry, disk: &mut dyn Disk, depth: usize) -> Result<(), String> {
        if entry.dir {
            if depth > 32 { return Err(format!("{} is nested too deep", display(path))); }
            for child in disk.list(path).map_err(|e| format!("{}: {}", display(path), e))? { self.plan_remove(&join(path, &child.name), &child, disk, depth + 1)?; }
        }
        self.steps.push(Step::Remove(path.into()));
        Ok(())
    }

    fn kept(&self, path: &str) -> bool { self.kept.iter().any(|k| inside(k, path)) }

    // Does the current step, or a slice of it. Ok(true): the step is finished.
    fn step(&mut self, disk: &mut dyn Disk) -> Result<bool, Failure> {
        let step = self.steps[self.index].clone();
        match step {
            Step::MakeDir(path) => disk.mkdir(&path).map(|_| true),
            Step::Remove(path) => {
                if self.op == Op::Move && self.kept(&path) { return Ok(true); }
                match disk.remove(&path) { Ok(()) | Err(Failure::NotFound) => Ok(true), Err(e) => Err(e) }
            }
            Step::Rename { from, to } => {
                if self.replace && disk.list(&to).is_err() { let _ = disk.remove(&to); } // an existing file is replaced
                disk.rename(&from, &to).map(|_| true)
            }
            Step::Copy { from, to, size } => {
                if self.copying.is_none() {
                    let source = disk.open(&from).ok_or(Failure::NotFound)?;
                    let sink = match disk.create(&to, self.replace) {
                        Ok(sink) => sink,
                        Err(Failure::Exists) => { self.skipped += 1; self.done += size; self.kept.push(from); return Ok(true); }
                        Err(e) => return Err(e),
                    };
                    self.copying = Some(Copying { source, sink, offset: 0, size });
                }
                if self.buffer.is_empty() { self.buffer = vec![0u8; SLICE]; }
                let copying = self.copying.as_mut().unwrap();
                if copying.offset >= copying.size { self.copying = None; return Ok(true); }
                let want = ((copying.size - copying.offset) as usize).min(SLICE);
                let got = copying.source.read(copying.offset, &mut self.buffer[..want]);
                if got == 0 { return Err(Failure::Other(String::from("cannot read the file"))); }
                copying.sink.write(&self.buffer[..got])?;
                copying.offset += got as u64;
                self.done += got as u64;
                if copying.offset >= copying.size { self.copying = None; return Ok(true); }
                Ok(false)
            }
        }
    }

    // Forgets a partly copied file (Retry starts it again, Skip leaves it).
    fn drop_copy(&mut self) {
        if let Some(copying) = self.copying.take() { self.done -= copying.offset; }
    }
}

pub enum Dialog {
    Help,
    Message { title: String, lines: Vec<String> },
    Mask { select: bool, line: InputLine },
    Find { line: InputLine },
    Results { mask: String, found: Vec<String>, list: ListState },
    Volume { side: usize, list: ListState, lines: Vec<String> },
    /// F5 / F6: where to.
    Target { op: Op, line: InputLine, sources: Vec<(String, Entry)> },
    Mkdir { line: InputLine },
    NewFile { line: InputLine },
    /// F8: delete what is planned?
    Delete { job: Box<Job>, count: usize, selected: usize },
    /// Some targets exist: overwrite them, skip them or cancel.
    Overwrite { job: Box<Job>, count: usize, selected: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { Quit, Redraw, Ignored }

pub struct Fm<'b> {
    pub panels: [Panel; 2],
    pub active: usize,
    pub menu: MenuBar<'static>,
    pub dialog: Option<Dialog>,
    pub notice: Option<String>,
    pub job: Option<Box<Job>>,
    pub editor: Option<Editor>,
    viewer: Option<Viewer<'b, Boxed>>,
    window: Option<&'b mut [u8]>,
    quick: Vec<u8>,
    preview: Option<(String, Vec<u8>, u64)>, // name, first bytes, size: what quick view shows
    volumes: [String; 2], // the volume line of each panel for the information panel
    /// The modifiers held (MOD_*, `mind::input::modifiers`): the key bars show what the keys do with them.
    pub modifiers: u8,
    /// The command line under the panels: typed text goes there, Enter runs it.
    pub command: InputLine,
    /// Panels hidden with Ctrl+O, Ctrl+F1, Ctrl+F2 or Ctrl+P: their place shows what the command line did.
    pub hidden: [bool; 2],
    output: Vec<String>,
    /// The grid's size at the last `draw`: where the mouse is (issue u001).
    size: (usize, usize),
    /// The buttons held at the last mouse event, and the last click on an entry (panel, entry, ms): a second one soon
    /// after on the same entry is a double click.
    buttons: u8,
    click: Option<(usize, usize, usize)>,
}

/// Two clicks on an entry within this many milliseconds open it.
pub const DOUBLE_CLICK_MS: usize = 500;

impl<'b> Fm<'b> {
    /// Both panels on the root; `window` is the viewer's buffer (64 KiB is plenty).
    pub fn new(window: &'b mut [u8], disk: &mut dyn Disk) -> Self {
        let mut fm = Self { panels: [Panel::new(Mode::Full), Panel::new(Mode::Brief)], active: 0, menu: MenuBar::new(&MENU_TITLES, &MENU_ITEMS), dialog: None, notice: None,
                            job: None, editor: None, viewer: None, window: Some(window), quick: vec![0; PREVIEW], preview: None, volumes: [String::new(), String::new()],
                            modifiers: 0, command: InputLine::new(), hidden: [false; 2], output: Vec::new(), size: (0, 0), buttons: 0, click: None };
        fm.load(0, "", None, disk);
        fm.load(1, "", None, disk);
        fm
    }

    pub fn viewing(&self) -> bool { self.viewer.is_some() }

    /// A job runs and needs `work` to be called (it is not waiting for an answer).
    pub fn busy(&self) -> bool { self.job.as_ref().is_some_and(|job| job.failure.is_none() && !job.asking) }

    fn volume_line(path: &str, disk: &mut dyn Disk) -> String {
        let (volume, _) = panel::volume(path);
        let name = VOLUMES.iter().find(|(v, _)| v.eq_ignore_ascii_case(volume)).map_or("?", |(_, name)| name);
        match disk.volume(path) {
            Some(v) => format!("{} {} FAT{}: {} KiB, {} KiB free", name, v.label, v.fat_bits, v.bytes / 1024, v.free / 1024),
            None => String::from(name),
        }
    }

    /// Lists `path` into a panel; on an error the panel keeps its listing and shows the error.
    pub fn load(&mut self, side: usize, path: &str, focus: Option<&str>, disk: &mut dyn Disk) {
        match disk.list(path) {
            Ok(entries) => {
                self.panels[side].set(path, entries, focus);
                self.volumes[side] = Self::volume_line(path, disk);
            }
            Err(error) => {
                let text = format!("{}: {}", display(path), error);
                self.notice = Some(format!("Cannot list {}", text));
                self.panels[side].error = Some(text);
            }
        }
        self.update_preview(disk);
    }

    fn reread(&mut self, side: usize, disk: &mut dyn Disk) { let path = self.panels[side].path.clone(); self.load(side, &path, None, disk); }

    // Quick view shows the file under the cursor of the active panel in the other one.
    fn update_preview(&mut self, disk: &mut dyn Disk) {
        self.preview = None;
        if self.panels[1 - self.active].mode != Mode::Quick { return; }
        let panel = &self.panels[self.active];
        let Some(entry) = panel.current().filter(|e| !e.dir) else { return };
        let path = join(&panel.path, &entry.name);
        if let Some(mut source) = disk.open(&path) {
            let size = source.size();
            let mut bytes = vec![0u8; (size as usize).min(PREVIEW)];
            let n = source.read(0, &mut bytes);
            bytes.truncate(n);
            self.preview = Some((entry.name.clone(), bytes, size));
        }
    }

    fn current_path(&self) -> Option<(String, Entry)> {
        let panel = &self.panels[self.active];
        panel.current().map(|e| (join(&panel.path, &e.name), e.clone()))
    }

    // What an operation works on: the marked entries of the active panel, or the one under the cursor.
    fn sources(&self) -> Vec<(String, Entry)> {
        let panel = &self.panels[self.active];
        let marked: Vec<(String, Entry)> = panel.items.iter().filter(|e| panel.is_marked(&e.name)).map(|e| (join(&panel.path, &e.name), e.clone())).collect();
        if !marked.is_empty() { return marked; }
        self.current_path().filter(|(_, e)| !e.is_up()).into_iter().collect()
    }

    fn view(&mut self, path: &str, disk: &mut dyn Disk) {
        let Some(source) = disk.open(path) else { self.notice = Some(format!("Cannot open {}", display(path))); return };
        let Some(window) = self.window.take() else { return };
        self.viewer = Some(Viewer::new(Boxed(source), window, &display(path)));
    }

    /// Opens `path` in the editor (a missing file when `new`).
    pub fn edit(&mut self, path: &str, new: bool, disk: &mut dyn Disk) {
        let text = match disk.open(path) {
            Some(mut source) => {
                let size = source.size();
                if size > crate::buffer::LIMIT as u64 { self.notice = Some(format!("{} is larger than 8 MiB", display(path))); return; }
                let mut text = vec![0u8; size as usize];
                let mut at = 0;
                while at < text.len() { let n = source.read(at as u64, &mut text[at..]); if n == 0 { break; } at += n; }
                text.truncate(at);
                text
            }
            None if new => Vec::new(),
            None => { self.notice = Some(format!("Cannot open {}", display(path))); return; }
        };
        let read_only = !new && !disk.writable(path);
        let mut editor = Editor::new(text, path, read_only);
        // Where the user may write: ram: and data/ on the boot disk.
        if read_only { editor.notice = Some(String::from("READ-ONLY: on the boot disk only data/ may be changed, and ram: (Shift+F2 saves a copy there)")); }
        self.editor = Some(editor);
    }

    // Saves the editor's text: `name.tmp` first, then it replaces the file (best effort on FAT).
    fn save(path: &str, text: &[u8], disk: &mut dyn Disk) -> Result<usize, String> {
        let temporary = format!("{}.tmp", path);
        let written = (|| -> Result<(), Failure> {
            let mut sink = disk.create(&temporary, true)?;
            for chunk in text.chunks(SLICE) { sink.write(chunk)?; }
            Ok(())
        })();
        if let Err(failure) = written { let _ = disk.remove(&temporary); return Err(failure.text()); }
        match disk.remove(path) {
            Ok(()) | Err(Failure::NotFound) => {}
            Err(failure) => { let _ = disk.remove(&temporary); return Err(failure.text()); }
        }
        disk.rename(&temporary, path).map_err(|f| format!("{} (the text is in {})", f.text(), display(&temporary)))?;
        disk.flush(path);
        Ok(text.len())
    }

    fn close_editor(&mut self, disk: &mut dyn Disk) {
        let Some(editor) = self.editor.take() else { return };
        let (dir, name) = parent(&editor.path);
        for side in 0..2 {
            let focus = (self.panels[side].path.eq_ignore_ascii_case(&dir) && side == self.active).then_some(name.as_str());
            let path = self.panels[side].path.clone();
            self.load(side, &path, focus, disk);
        }
    }

    fn editor_key(&mut self, key: Key, disk: &mut dyn Disk) -> Outcome {
        let Some(editor) = self.editor.as_mut() else { return Outcome::Ignored };
        let (path, quit) = match editor.key(key) {
            EditOutcome::Quit => { self.close_editor(disk); return Outcome::Redraw; }
            EditOutcome::Save(path) => (path, false),
            EditOutcome::SaveAndQuit(path) => (path, true),
            EditOutcome::Redraw | EditOutcome::Ignored => return Outcome::Redraw,
        };
        let result = Self::save(&path, &editor.buffer.text(), disk);
        let saved = result.is_ok();
        editor.saved(&path, result);
        if quit && saved { self.close_editor(disk); }
        Outcome::Redraw
    }

    fn up(&mut self, disk: &mut dyn Disk) {
        let side = self.active;
        if is_root(&self.panels[side].path) { return; }
        let (up, name) = parent(&self.panels[side].path);
        self.load(side, &up, Some(&name), disk);
    }

    /// Enter: a directory is opened, a program started, any other file viewed.
    fn open(&mut self, disk: &mut dyn Disk) {
        let Some((path, entry)) = self.current_path() else { return };
        if entry.is_up() { self.up(disk); }
        else if entry.dir { self.load(self.active, &path, None, disk); }
        else if entry.is_program() {
            self.notice = Some(match disk.run(&path, "") {
                Ok(started) => started.text(&entry.name),
                Err(error) => format!("Cannot start {}: {}", entry.name, error),
            });
        } else { self.view(&path, disk); }
    }

    // Recursive search from `path` for names matching `mask`.
    fn find(&self, path: &str, mask: &str, depth: usize, disk: &mut dyn Disk, found: &mut Vec<String>) {
        let Ok(entries) = disk.list(path) else { return };
        for entry in entries {
            if found.len() >= FIND_MAX { return; }
            let full = join(path, &entry.name);
            if matches(mask, &entry.name) { found.push(full.clone()); }
            if entry.dir && depth < 16 { self.find(&full, mask, depth + 1, disk, found); }
        }
    }

    /// Goes to the directory of `path` with the cursor on it.
    fn go_to(&mut self, path: &str, disk: &mut dyn Disk) {
        let (dir, name) = parent(path);
        self.load(self.active, &dir, Some(&name), disk);
    }

    fn volume_dialog(&mut self, side: usize, disk: &mut dyn Disk) {
        let lines = VOLUMES.iter().map(|(path, _)| Self::volume_line(path, disk)).collect();
        let selected = VOLUMES.iter().position(|(v, _)| v.eq_ignore_ascii_case(panel::volume(&self.panels[side].path).0)).unwrap_or(0);
        self.dialog = Some(Dialog::Volume { side, list: ListState { selected, top: 0 }, lines });
    }

    // F5 / F6: asks where to, with the other panel's directory filled in.
    fn ask_target(&mut self, op: Op) {
        let sources = self.sources();
        if sources.is_empty() { self.notice = Some(String::from("Nothing to copy or move here")); return; }
        let other = &self.panels[1 - self.active].path;
        let mut line = InputLine::new();
        line.set(&display(other));
        self.dialog = Some(Dialog::Target { op, line, sources });
    }

    // Plans a copy or move of `sources` to `typed` (a directory to put them in, or the new name of one entry).
    fn start_transfer(&mut self, op: Op, sources: Vec<(String, Entry)>, typed: &str, disk: &mut dyn Disk) {
        let current = self.panels[self.active].path.clone();
        let target = resolve(&current, typed);
        let into = disk.list(&target).is_ok();
        if !into && sources.len() > 1 { self.notice = Some(format!("No directory {}", display(&target))); return; }
        let mut job = Box::new(Job::new(op));
        let mut conflicts = 0;
        let mut listings: Vec<(String, Vec<Entry>)> = Vec::new();
        for (from, entry) in &sources {
            let to = if into { join(&target, &entry.name) } else { target.clone() };
            if from.eq_ignore_ascii_case(&to) { self.notice = Some(format!("Cannot {} {} onto itself", if op == Op::Copy { "copy" } else { "move" }, display(from))); return; }
            if entry.dir && inside(&to, from) { self.notice = Some(format!("Cannot put {} inside itself", display(from))); return; }
            let (dir, name) = parent(&to);
            if !listings.iter().any(|(d, _)| *d == dir) { let entries = disk.list(&dir).unwrap_or_default(); listings.push((dir.clone(), entries)); }
            let exists = listings.iter().find(|(d, _)| *d == dir).is_some_and(|(_, entries)| entries.iter().any(|e| e.name.eq_ignore_ascii_case(&name)));
            if exists { conflicts += 1; }
            let planned = if op == Op::Move && same_volume(from, &to) {
                job.steps.push(Step::Rename { from: from.clone(), to: to.clone() });
                Ok(())
            } else { job.plan_copy(from, &to, entry, disk, 0) };
            if let Err(error) = planned { self.notice = Some(error); return; }
            job.sources.push(from.clone());
        }
        if op == Op::Move {
            for (from, entry) in &sources { if !same_volume(from, &target) { if let Err(error) = job.plan_remove(from, entry, disk, 0) { self.notice = Some(error); return; } } }
        }
        job.target = Some(target.clone());
        job.focus = if into { sources.first().map(|(_, e)| e.name.clone()) } else { Some(parent(&target).1) };
        if conflicts > 0 { self.dialog = Some(Dialog::Overwrite { job, count: conflicts, selected: 0 }); } else { self.job = Some(job); }
    }

    fn ask_delete(&mut self, disk: &mut dyn Disk) {
        let sources = self.sources();
        if sources.is_empty() { self.notice = Some(String::from("Nothing to delete here")); return; }
        let mut job = Box::new(Job::new(Op::Delete));
        for (path, entry) in &sources {
            if let Err(error) = job.plan_remove(path, entry, disk, 0) { self.notice = Some(error); return; }
            job.sources.push(path.clone());
        }
        let count = sources.len();
        self.dialog = Some(Dialog::Delete { job, count, selected: 0 });
    }

    /// Does a slice of the running job (a step, or up to `SLICE` bytes of a copy); the caller serves keys and the
    /// screen between calls.
    pub fn work(&mut self, disk: &mut dyn Disk) {
        if !self.busy() { return; }
        let job = self.job.as_mut().unwrap();
        if job.index < job.steps.len() {
            match job.step(disk) {
                Ok(true) => { job.index += 1; job.files += 1; }
                Ok(false) => {}
                Err(failure) => { job.failure = Some(failure); job.choice = 0; }
            }
        }
        if job.index >= job.steps.len() && job.failure.is_none() { self.finish(false, disk); }
    }

    // The job is over: the volumes are flushed, the panels reread.
    fn finish(&mut self, stopped: bool, disk: &mut dyn Disk) {
        let Some(mut job) = self.job.take() else { return };
        job.copying = None; // closes a partly written file
        for path in job.sources.iter().chain(job.target.iter()) { disk.flush(path); }
        let bytes = if job.op == Op::Delete { String::new() } else { format!(", {} bytes", job.done) };
        let mut text = format!("{} {} of {}{}", if stopped { "Stopped:" } else { job.op.done() }, job.files, job.steps.len(), bytes);
        if job.skipped > 0 { text += &format!(", {} skipped", job.skipped); }
        if job.errors > 0 { text += &format!(", {} failed", job.errors); }
        self.notice = Some(text);
        self.panels[self.active].marked.clear();
        // The cursor goes to what was made: the first entry put in a directory, or the new name.
        let focus_for = |path: &str| -> Option<String> {
            let target = job.target.as_ref()?;
            if target.eq_ignore_ascii_case(path) { job.focus.clone() } else if parent(target).0.eq_ignore_ascii_case(path) { Some(parent(target).1) } else { None }
        };
        for side in 0..2 {
            let path = self.panels[side].path.clone();
            let focus = focus_for(&path);
            self.load(side, &path, focus.as_deref(), disk);
        }
    }

    fn job_key(&mut self, key: Key, disk: &mut dyn Disk) -> Outcome {
        let job = self.job.as_mut().unwrap();
        let mut stop = None; // Some(stopped): the job ends
        if job.failure.is_some() {
            match buttons_key(key, &mut job.choice, 3) {
                Some(Some(0)) => { job.drop_copy(); job.failure = None; }
                Some(Some(1)) => {
                    let partial = job.copying.is_some();
                    job.drop_copy();
                    if let Some(Step::Copy { from, to, size }) = job.steps.get(job.index).cloned() {
                        if partial { let _ = disk.remove(&to); } // a partly written copy is not left behind
                        job.kept.push(from); job.done += size;
                    }
                    job.failure = None; job.skipped += 1; job.errors += 1; job.index += 1;
                    if job.index >= job.steps.len() { stop = Some(false); }
                }
                Some(_) => {
                    if let (Some(Step::Copy { to, .. }), true) = (job.steps.get(job.index).cloned(), job.copying.is_some()) { job.copying = None; let _ = disk.remove(&to); }
                    job.errors += 1; stop = Some(true);
                }
                None => {}
            }
        } else if job.asking {
            match buttons_key(key, &mut job.choice, 2) {
                Some(Some(0)) => stop = Some(true),
                Some(_) => job.asking = false,
                None => {}
            }
        } else if key.code() == Code::Esc || key.code() == Code::F(10) {
            job.asking = true; job.choice = 1;
        }
        if let Some(stopped) = stop { self.finish(stopped, disk); }
        Outcome::Redraw
    }

    // What the command line did, for the place of hidden panels.
    fn say(&mut self, text: String) {
        if self.output.len() >= OUTPUT_MAX { self.output.remove(0); }
        self.output.push(text);
    }

    // A hidden panel is not the active one while the other is shown.
    fn fix_active(&mut self, disk: &mut dyn Disk) {
        if self.hidden[self.active] && !self.hidden[1 - self.active] { self.active = 1 - self.active; self.update_preview(disk); }
    }

    // The command line and the panel switches (Ctrl+O, Ctrl+F1, Ctrl+F2, Ctrl+P, as in Midnight and Norton Commander).
    // None: the key is not for them.
    fn command_key(&mut self, key: Key, disk: &mut dyn Disk) -> Option<Outcome> {
        if key.is_ctrl('o') { let shown = !self.hidden[0] || !self.hidden[1]; self.hidden = [shown, shown]; self.fix_active(disk); return Some(Outcome::Redraw); }
        if key.is_ctrl('p') { self.hidden[1 - self.active] = !self.hidden[1 - self.active]; return Some(Outcome::Redraw); }
        match key.code() {
            Code::F(n @ 1..=2) if key.ctrl() => { let side = n as usize - 1; self.hidden[side] = !self.hidden[side]; self.fix_active(disk); return Some(Outcome::Redraw); }
            Code::Enter if key.alt() || key.ctrl() => {
                let name = self.panels[self.active].current().filter(|e| !e.is_up()).map(|e| e.name.clone())?;
                if !self.command.is_empty() && !self.command.as_str().ends_with(' ') { self.command.insert(' '); }
                for ch in name.chars().chain(Some(' ')) { self.command.insert(ch); }
                return Some(Outcome::Redraw);
            }
            Code::Enter if !self.command.is_empty() => { self.execute(disk); return Some(Outcome::Redraw); }
            Code::Esc if !self.command.is_empty() => { self.command.clear(); return Some(Outcome::Redraw); }
            Code::Left | Code::Right | Code::Home | Code::End | Code::Backspace | Code::Delete if !self.command.is_empty() => { self.command.key(key); return Some(Outcome::Redraw); }
            _ => {}
        }
        let ch = key.text()?;
        // On an empty line these keep their panel meaning: + - * mark, a space does nothing.
        if self.command.is_empty() && matches!(ch, '+' | '-' | '*' | ' ') { return None; }
        self.command.insert(ch);
        Some(Outcome::Redraw)
    }

    // Enter on the command line: cd, edit, view here; anything else is a program started with its arguments (names of
    // the active panel's entries become their paths).
    fn execute(&mut self, disk: &mut dyn Disk) {
        let line = String::from(self.command.as_str().trim());
        self.command.clear();
        let here = self.panels[self.active].path.clone();
        self.say(format!("{}> {}", display(&here), line));
        let (word, rest) = match line.split_once(' ') { Some((word, rest)) => (word, rest.trim()), None => (line.as_str(), "") };
        let failed = match word.to_ascii_lowercase().as_str() {
            "cd" => {
                let target = if rest.is_empty() { String::from(panel::volume(&here).0) } else { resolve(&here, rest) };
                match disk.list(&target) {
                    Ok(_) => {
                        // Going up puts the cursor on the directory left.
                        let (up, name) = parent(&here);
                        let focus = (!is_root(&here) && up.eq_ignore_ascii_case(&target)).then_some(name);
                        self.load(self.active, &target, focus.as_deref(), disk);
                        None
                    }
                    Err(error) => Some(format!("cd: {}: {}", display(&target), error)),
                }
            }
            "edit" | "view" if !rest.is_empty() => {
                let path = resolve(&here, rest);
                if disk.list(&path).is_ok() { Some(format!("{}: {} is a directory", word, display(&path))) }
                else if word.eq_ignore_ascii_case("view") { self.view(&path, disk); None }
                else { let exists = disk.open(&path).is_some(); self.edit(&path, !exists, disk); None }
            }
            _ => {
                let panel = &self.panels[self.active];
                let entry = |name: &str| panel.items.iter().find(|e| !e.is_up() && e.name.eq_ignore_ascii_case(name));
                // A program of this directory (with or without .elf) runs from here; another name is the loader's.
                let elf = format!("{}.elf", word);
                let program = match entry(word).or_else(|| entry(&elf)).filter(|e| e.is_program()) {
                    Some(e) => join(&here, &e.name),
                    None if word.contains('/') || word.contains(':') => resolve(&here, word),
                    None => String::from(word),
                };
                let args: Vec<String> = rest.split_whitespace().map(|arg| entry(arg).map_or_else(|| String::from(arg), |e| join(&here, &e.name))).collect();
                let text = match disk.run(&program, &args.join(" ")) {
                    Ok(started) => started.text(word),
                    Err(error) => format!("Cannot start {}: {}", word, error),
                };
                self.notice = Some(text.clone());
                self.say(text);
                None
            }
        };
        if let Some(text) = failed { self.notice = Some(text.clone()); self.say(text); }
    }

    fn panel_command(&mut self, side: usize, item: usize, disk: &mut dyn Disk) {
        let panel = &mut self.panels[side];
        match item {
            0 => panel.mode = Mode::Brief, 1 => panel.mode = Mode::Full, 2 => panel.mode = Mode::Info, 3 => panel.mode = Mode::Quick,
            4..=7 => {
                let sort = [Sort::Name, Sort::Extension, Sort::Time, Sort::Size][item - 4];
                panel.sort = sort; panel.arrange(None);
            }
            8 => { panel.reverse = !panel.reverse; panel.arrange(None); }
            9 => self.reread(side, disk),
            _ => self.volume_dialog(side, disk),
        }
        self.update_preview(disk);
    }

    fn command(&mut self, menu: usize, item: usize, disk: &mut dyn Disk) {
        match (menu, item) {
            (0, item) => self.panel_command(0, item, disk),
            (4, item) => self.panel_command(1, item, disk),
            (1, 0) => if let Some((path, entry)) = self.current_path() { if !entry.dir { self.view(&path, disk); } },
            (1, 1) => if let Some((path, entry)) = self.current_path() { if !entry.dir { self.edit(&path, false, disk); } },
            (1, 2) => self.dialog = Some(Dialog::NewFile { line: InputLine::new() }),
            (1, 3) => self.ask_target(Op::Copy),
            (1, 4) => self.ask_target(Op::Move),
            (1, 5) => self.dialog = Some(Dialog::Mkdir { line: InputLine::new() }),
            (1, 6) => self.ask_delete(disk),
            (1, 7) => self.open(disk),
            (1, 8) => self.dialog = Some(Dialog::Mask { select: true, line: mask_line() }),
            (1, 9) => self.dialog = Some(Dialog::Mask { select: false, line: mask_line() }),
            (1, _) => self.panels[self.active].invert(),
            (2, 0) => self.dialog = Some(Dialog::Find { line: mask_line() }),
            (2, _) => self.panels.swap(0, 1),
            (3, _) => for panel in self.panels.iter_mut() { panel.hidden = !panel.hidden; panel.arrange(None); },
            _ => {}
        }
    }

    fn dialog_key(&mut self, key: Key, disk: &mut dyn Disk) -> Outcome {
        let Some(mut dialog) = self.dialog.take() else { return Outcome::Ignored };
        let keep = match &mut dialog {
            Dialog::Help | Dialog::Message { .. } => !matches!(key.code(), Code::Esc | Code::Enter | Code::F(10) | Code::F(1)),
            Dialog::Mask { select, line } => match line.key(key) {
                Edit::Submit => { let n = self.panels[self.active].mark_mask(line.as_str(), *select); self.notice = Some(format!("{} files {}", n, if *select { "marked" } else { "unmarked" })); false }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::Find { line } => match line.key(key) {
                Edit::Submit => {
                    let mask = String::from(line.as_str());
                    let mut found = Vec::new();
                    let start = self.panels[self.active].path.clone();
                    self.find(&start, &mask, 0, disk, &mut found);
                    found.sort_by_key(|path| path.to_lowercase());
                    self.dialog = Some(Dialog::Results { mask, found, list: ListState::default() });
                    return Outcome::Redraw;
                }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::Results { found, list, .. } => {
                if list.key(key, found.len(), 12) { true } else {
                    match key.code() {
                        Code::Enter => { if let Some(path) = found.get(list.selected).cloned() { self.go_to(&path, disk); } false }
                        Code::F(3) => { if let Some(path) = found.get(list.selected).cloned() { self.view(&path, disk); } true }
                        Code::Esc | Code::F(10) => false,
                        _ => true,
                    }
                }
            }
            Dialog::Volume { side, list, .. } => {
                if list.key(key, VOLUMES.len(), VOLUMES.len()) { true } else {
                    match key.code() {
                        Code::Enter => { let (side, path) = (*side, VOLUMES[list.selected.min(VOLUMES.len() - 1)].0); self.load(side, path, None, disk); false }
                        Code::Esc | Code::F(10) => false,
                        _ => true,
                    }
                }
            }
            Dialog::Target { op, line, sources } => match line.key(key) {
                Edit::Submit => { let (op, typed) = (*op, String::from(line.as_str())); let sources = core::mem::take(sources); self.start_transfer(op, sources, &typed, disk); false }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::Mkdir { line } => match line.key(key) {
                Edit::Submit => {
                    let typed = String::from(line.as_str().trim());
                    if !typed.is_empty() {
                        let path = resolve(&self.panels[self.active].path, &typed);
                        match disk.mkdir(&path) {
                            Ok(()) => {
                                disk.flush(&path);
                                // The cursor goes to the new directory, or to the one it was made in.
                                let here = self.panels[self.active].path.clone();
                                let below = if is_root(&here) { panel::volume(&path).1 } else { path.get(here.len() + 1..).unwrap_or("") };
                                let focus = (inside(&path, &here) && !path.eq_ignore_ascii_case(&here)).then(|| String::from(below.split('/').next().unwrap_or("")));
                                self.load(self.active, &here, focus.as_deref(), disk);
                                let other = self.panels[1 - self.active].path.clone();
                                self.load(1 - self.active, &other, None, disk);
                            }
                            Err(failure) => self.notice = Some(format!("Cannot make {}: {}", display(&path), failure.text())),
                        }
                    }
                    false
                }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::NewFile { line } => match line.key(key) {
                Edit::Submit => {
                    let typed = String::from(line.as_str().trim());
                    if !typed.is_empty() { let path = resolve(&self.panels[self.active].path, &typed); self.edit(&path, true, disk); }
                    false
                }
                Edit::Cancel => false,
                _ => true,
            },
            Dialog::Delete { job, selected, .. } => match buttons_key(key, selected, 2) {
                Some(Some(0)) => { self.job = Some(core::mem::replace(job, Box::new(Job::new(Op::Delete)))); false }
                Some(_) => false,
                None => true,
            },
            Dialog::Overwrite { job, selected, .. } => match buttons_key(key, selected, 3) {
                Some(Some(answer @ 0..=1)) => { let mut job = core::mem::replace(job, Box::new(Job::new(Op::Copy))); job.replace = answer == 0; self.job = Some(job); false }
                Some(_) => false,
                None => true,
            },
        };
        if keep && self.dialog.is_none() { self.dialog = Some(dialog); }
        Outcome::Redraw
    }

    /// Handles a key; the caller redraws afterwards.
    pub fn key(&mut self, key: Key, disk: &mut dyn Disk) -> Outcome {
        if self.job.is_some() { return self.job_key(key, disk); }
        if self.editor.is_some() { return self.editor_key(key, disk); }
        if let Some(viewer) = self.viewer.as_mut() {
            if viewer.key(key) == Action::Quit { let viewer = self.viewer.take().unwrap(); self.window = Some(viewer.into_buffer()); }
            return Outcome::Redraw;
        }
        if self.dialog.is_some() { return self.dialog_key(key, disk); }
        if self.menu.open {
            if let MenuAction::Chosen(menu, item) = self.menu.key(key) { self.command(menu, item, disk); }
            return Outcome::Redraw;
        }
        self.notice = None;
        if let Some(outcome) = self.command_key(key, disk) { return outcome; }
        let side = self.active;
        if self.hidden[side] && !matches!(key.code(), Code::F(1) | Code::F(9) | Code::F(10) | Code::Esc | Code::Tab) { return Outcome::Ignored; }
        if self.panels[side].key(key) { self.update_preview(disk); return Outcome::Redraw; }
        match key.code() {
            Code::F(10) | Code::Esc => return Outcome::Quit,
            Code::Tab => { self.active = 1 - self.active; self.fix_active(disk); self.update_preview(disk); return Outcome::Redraw; }
            Code::Enter => { self.open(disk); return Outcome::Redraw; }
            Code::Backspace => { self.up(disk); return Outcome::Redraw; }
            Code::Insert => { self.panels[side].toggle_mark(); self.update_preview(disk); return Outcome::Redraw; }
            Code::F(1) if key.alt() => { self.volume_dialog(0, disk); return Outcome::Redraw; }
            Code::F(2) if key.alt() => { self.volume_dialog(1, disk); return Outcome::Redraw; }
            Code::F(7) if key.alt() => { self.dialog = Some(Dialog::Find { line: mask_line() }); return Outcome::Redraw; }
            Code::F(n @ 3..=6) if key.ctrl() => {
                let panel = &mut self.panels[side];
                panel.sort = [Sort::Name, Sort::Extension, Sort::Time, Sort::Size][n as usize - 3];
                panel.arrange(None);
                return Outcome::Redraw;
            }
            Code::F(1) => { self.dialog = Some(Dialog::Help); return Outcome::Redraw; }
            Code::F(3) => {
                if let Some((path, entry)) = self.current_path() { if entry.dir { self.open(disk); } else { self.view(&path, disk); } }
                return Outcome::Redraw;
            }
            Code::F(4) if key.shift() => { self.command(1, 2, disk); return Outcome::Redraw; }
            Code::F(4) => {
                match self.current_path() { Some((path, entry)) if !entry.dir => self.edit(&path, false, disk), _ => self.notice = Some(String::from("F4 edits a file; Shift+F4 makes a new one")) }
                return Outcome::Redraw;
            }
            Code::F(5) => { self.ask_target(Op::Copy); return Outcome::Redraw; }
            Code::F(6) => { self.ask_target(Op::Move); return Outcome::Redraw; }
            Code::F(7) => { self.dialog = Some(Dialog::Mkdir { line: InputLine::new() }); return Outcome::Redraw; }
            Code::F(8) => { self.ask_delete(disk); return Outcome::Redraw; }
            Code::F(9) => { self.menu.open = true; self.menu.menu = if side == 0 { 0 } else { 4 }; self.menu.item = 0; return Outcome::Redraw; }
            _ => {}
        }
        if key.is_ctrl('r') { self.reread(side, disk); return Outcome::Redraw; }
        if key.is_ctrl('u') { self.panels.swap(0, 1); self.volumes.swap(0, 1); self.update_preview(disk); return Outcome::Redraw; }
        if key.is_ctrl('h') { self.command(3, 0, disk); return Outcome::Redraw; }
        if key.is_ctrl('l') || key.is_ctrl('q') {
            let mode = if key.is_ctrl('l') { Mode::Info } else { Mode::Quick };
            let other = &mut self.panels[1 - side];
            other.mode = if other.mode == mode { Mode::Full } else { mode };
            self.update_preview(disk);
            return Outcome::Redraw;
        }
        match key.text() {
            Some('+') => { self.dialog = Some(Dialog::Mask { select: true, line: mask_line() }); Outcome::Redraw }
            Some('-') => { self.dialog = Some(Dialog::Mask { select: false, line: mask_line() }); Outcome::Redraw }
            Some('*') => { self.panels[side].invert(); Outcome::Redraw }
            _ => Outcome::Ignored,
        }
    }

    // The panels' places on a grid of `size`, as `draw` lays them out.
    fn panel_area(&self, side: usize) -> Rect {
        let (w, h) = self.size;
        let left = w / 2;
        if side == 0 { Rect::new(0, 0, left, h.saturating_sub(2)) } else { Rect::new(left, 0, w - left, h.saturating_sub(2)) }
    }

    // The shown panel at cell (x, y).
    fn panel_at(&self, x: usize, y: usize) -> Option<usize> {
        (0..2).find(|&side| !self.hidden[side] && self.panel_area(side).contains(x, y))
    }

    /// A mouse event at cell (x, y) of the grid last drawn, with `buttons` held (`POINTER_*`) and the wheel turned
    /// `wheel` steps (negative: up), `now` ms after start (issue u001). A click on an entry makes its panel active and
    /// puts the cursor on it, a second click soon after opens it as Enter does, a right click marks it as Insert does;
    /// the wheel moves the cursor of the panel under the mouse, or scrolls the viewer or the editor, three lines a
    /// step; a click on the key bar presses that key with the modifiers held.
    pub fn pointer(&mut self, x: usize, y: usize, buttons: u8, wheel: i32, now: usize, disk: &mut dyn Disk) -> Outcome {
        let pressed = buttons & !self.buttons;
        self.buttons = buttons;
        let (w, h) = self.size;
        if w == 0 || h < 3 { return Outcome::Ignored; }
        let press = |code: u16, mods: u8| Key::from_event(keys::event(code, 0, mods));
        if pressed & POINTER_LEFT != 0 && y + 1 == h {
            let number = (x / (w / 10).max(1)).min(9) as u16;
            return match press(KEY_F1 + number, self.modifiers) { Some(key) => self.key(key, disk), None => Outcome::Ignored };
        }
        let lines = 3 * wheel.unsigned_abs() as usize;
        if self.job.is_some() || self.dialog.is_some() || self.menu.open { return Outcome::Ignored; }
        if self.editor.is_some() || self.viewer.is_some() {
            if wheel == 0 { return Outcome::Ignored; }
            let Some(key) = press(if wheel < 0 { KEY_UP } else { KEY_DOWN }, 0) else { return Outcome::Ignored };
            for _ in 0..lines { if self.key(key, disk) == Outcome::Quit { return Outcome::Quit; } }
            return Outcome::Redraw;
        }
        let Some(side) = self.panel_at(x, y) else { return Outcome::Ignored };
        // A panel showing information or quick view is not a listing.
        let listing = side == self.active || matches!(self.panels[side].mode, Mode::Full | Mode::Brief);
        if wheel != 0 && listing {
            self.activate(side, disk);
            self.panels[side].move_by(if wheel < 0 { -(lines as isize) } else { lines as isize });
            self.update_preview(disk);
            return Outcome::Redraw;
        }
        if pressed & (POINTER_LEFT | POINTER_RIGHT) == 0 { return Outcome::Ignored; }
        self.notice = None;
        let entry = if listing { self.panels[side].entry_at(self.panel_area(side), x, y) } else { None };
        self.activate(side, disk);
        let Some(index) = entry else { self.click = None; return Outcome::Redraw };
        self.panels[side].select(index);
        if pressed & POINTER_RIGHT != 0 {
            self.click = None;
            let marked = self.panels[side].list.selected;
            self.panels[side].toggle_mark();
            self.panels[side].select(marked);
        } else {
            let double = self.click.is_some_and(|(s, i, at)| s == side && i == index && now.saturating_sub(at) <= DOUBLE_CLICK_MS);
            self.click = if double { None } else { Some((side, index, now)) };
            if double { self.open(disk); }
        }
        self.update_preview(disk);
        Outcome::Redraw
    }

    // Makes `side` the active panel.
    fn activate(&mut self, side: usize, disk: &mut dyn Disk) {
        if self.active != side { self.active = side; self.update_preview(disk); }
    }

    fn info(&self, grid: &mut Grid, area: Rect, theme: &Theme) {
        grid.frame_titled(area, Line::Double, "Information", theme.frame, theme.frame);
        let inner = area.inner();
        let panel = &self.panels[self.active];
        let (files, bytes, dirs) = panel.totals();
        let (marked, marked_bytes) = panel.marked_size();
        let mut lines: Vec<(String, bool)> = vec![
            (String::from("MIND CORE file manager"), true),
            (self.volumes[self.active].clone(), false),
            (String::new(), false),
            (format!("Directory {}", display(&panel.path)), true),
            (format!("  {} files, {} bytes; {} directories", files, bytes, dirs), false),
            (format!("  marked: {} files, {} bytes", marked, marked_bytes), false),
            (String::new(), false),
        ];
        if let Some(entry) = panel.current().filter(|e| !e.is_up()) {
            let (y, mo, d, h, mi, s) = panel::fat_time(entry.modified);
            lines.push((entry.name.clone(), true));
            lines.push((if entry.dir { String::from("  directory") } else { format!("  {} bytes", entry.size) }, false));
            if entry.modified != 0 { lines.push((format!("  modified {}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, s), false)); }
            let attributes: Vec<&str> = [(VFS_ENTRY_READ_ONLY, "read-only"), (VFS_ENTRY_HIDDEN, "hidden"), (VFS_ENTRY_SYSTEM, "system"), (VFS_ENTRY_ARCHIVE, "archive")]
                .iter().filter(|(bit, _)| entry.flags & bit != 0).map(|(_, name)| *name).collect();
            if !attributes.is_empty() { lines.push((format!("  {}", attributes.join(", ")), false)); }
            if entry.is_program() { lines.push((String::from("  a program: Enter starts it"), false)); }
        }
        for (i, (text, head)) in lines.iter().enumerate().take(inner.h) {
            grid.text_max(inner.x + 1, inner.y + i, text, inner.w.saturating_sub(2), if *head { theme.header } else { theme.panel });
        }
    }

    fn quick_view(&mut self, grid: &mut Grid, area: Rect, theme: &Theme) {
        let inner = area.inner();
        match self.preview.as_ref() {
            Some((name, bytes, size)) => {
                let title = if *size as usize > bytes.len() { format!("Quick view: first {} of {} bytes", bytes.len(), size) } else { String::from("Quick view") };
                grid.frame_titled(area, Line::Double, &title, theme.frame, theme.frame);
                let mut viewer = Viewer::new(Bytes(bytes), &mut self.quick, name);
                viewer.wrap = true;
                viewer.draw(grid, inner, theme);
            }
            None => {
                grid.frame_titled(area, Line::Double, "Quick view", theme.frame, theme.frame);
                let text = match self.panels[self.active].current() { Some(e) if e.dir => "A directory", _ => "Nothing to show" };
                grid.text_centered(inner, inner.y + inner.h / 2, text, theme.dim);
            }
        }
    }

    fn draw_job(job: &Job, grid: &mut Grid, theme: &Theme) {
        if let Some(failure) = &job.failure {
            let what = job.steps.get(job.index).map_or(String::new(), |s| s.describe());
            let lines = [what, format!("failed: {}", failure.text())];
            message(grid, "Error", &[lines[0].as_str(), lines[1].as_str()], &["Retry", "Skip", "Abort"], job.choice, theme);
            return;
        }
        let width = (grid.cols * 3 / 4).clamp(20, 70).min(grid.cols);
        let inner = dialog(grid, job.op.title(), width, 8, theme);
        let what = job.steps.get(job.index).map_or(String::new(), |s| s.describe());
        grid.text_max(inner.x + 1, inner.y, &what, inner.w.saturating_sub(2), theme.dialog);
        let (offset, size) = job.copying.as_ref().map_or((0, 0), |c| (c.offset, c.size));
        progress(grid, inner.x + 1, inner.y + 1, inner.w.saturating_sub(2), offset, size, theme.selected, theme.dialog);
        progress(grid, inner.x + 1, inner.y + 3, inner.w.saturating_sub(2), job.done, job.total, theme.selected, theme.dialog);
        let totals = if job.op == Op::Delete { format!("{} of {}", job.index, job.steps.len()) } else { format!("{} of {}; {} of {} bytes", job.index, job.steps.len(), job.done, job.total) };
        grid.text_max(inner.x + 1, inner.y + 4, &totals, inner.w.saturating_sub(2), theme.dialog);
        grid.text_max(inner.x + 1, inner.y + 5, "Esc: stop", inner.w.saturating_sub(2), theme.dialog);
        if job.asking { message(grid, job.op.title(), &["Stop the operation?"], &["Stop", "Continue"], job.choice, theme); }
    }

    /// Draws everything; returns the cursor of an open input line.
    pub fn draw(&mut self, grid: &mut Grid, theme: &Theme) -> Option<(usize, usize)> {
        let (w, h) = (grid.cols, grid.rows);
        self.size = (w, h);
        grid.clear(theme.panel);
        if let Some(editor) = self.editor.as_mut() { editor.modifiers = self.modifiers; return editor.draw(grid, theme); }
        if let Some(viewer) = self.viewer.as_mut() { viewer.modifiers = self.modifiers; let area = grid.area(); return viewer.draw(grid, area, theme); }
        let height = h.saturating_sub(2);
        let left = w / 2;
        for side in 0..2 {
            let area = if side == 0 { Rect::new(0, 0, left, height) } else { Rect::new(left, 0, w - left, height) };
            if self.hidden[side] { continue; }
            match self.panels[side].mode {
                Mode::Info if side != self.active => self.info(grid, area, theme),
                Mode::Quick if side != self.active => self.quick_view(grid, area, theme),
                _ => { let active = side == self.active; self.panels[side].draw(grid, area, theme, active); }
            }
        }
        // Where panels are hidden: what the command line did, the latest lines last.
        let output = match self.hidden {
            [true, true] => Some(Rect::new(0, 0, w, height)), [true, false] => Some(Rect::new(0, 0, left, height)),
            [false, true] => Some(Rect::new(left, 0, w - left, height)), [false, false] => None,
        };
        if let Some(area) = output {
            let start = self.output.len().saturating_sub(area.h);
            for (i, text) in self.output[start..].iter().enumerate() { grid.text_max(area.x, area.y + i, text, area.w, theme.panel); }
            if self.output.is_empty() { grid.text_max(area.x, area.y, "Ctrl+O shows the panels again", area.w, theme.dim); }
        }
        // The line above the key bar: a notice, or the command line with where the active panel is.
        let mut cursor = None;
        match &self.notice {
            Some(notice) if self.command.is_empty() => grid.text_padded(0, h - 2, notice, w, theme.marked),
            _ => {
                let prompt = format!("{}> ", display(&self.panels[self.active].path));
                let x = prompt.chars().count().min(w.saturating_sub(8));
                grid.text_max(0, h - 2, &prompt, x, theme.fkey_number);
                let column = self.command.draw(grid, x, h - 2, w - x, theme.fkey_number);
                if !self.command.is_empty() || self.hidden == [true, true] { cursor = Some((column, h - 2)); }
            }
        }
        fkey_bar(grid, h - 1, KEYS.labels(self.modifiers), theme);
        if self.menu.open { self.menu.draw(grid, 0, theme); }
        if let Some(job) = self.job.as_ref() { Self::draw_job(job, grid, theme); return None; }
        match self.dialog.as_mut() {
            None => if self.menu.open { None } else { cursor },
            Some(Dialog::Help) => { message(grid, "fm — keys", &HELP[..HELP.len() - 1], &["OK"], 0, theme); None }
            Some(Dialog::Message { title, lines }) => { let refs: Vec<&str> = lines.iter().map(|l| l.as_str()).collect(); message(grid, title, &refs, &["OK"], 0, theme); None }
            Some(Dialog::Mask { select, line }) => Some(input_dialog(grid, if *select { "Select" } else { "Unselect" }, "Files matching (* and ?, several with ,):", line, 50, theme)),
            Some(Dialog::Find { line }) => Some(input_dialog(grid, "Find file", "Names matching (* and ?), from this directory down:", line, 60, theme)),
            Some(Dialog::Target { op, line, sources }) => {
                let what = if sources.len() == 1 { sources[0].1.name.clone() } else { format!("{} entries", sources.len()) };
                let prompt = format!("{} {} to (A:/..., ram:/...):", if *op == Op::Copy { "Copy" } else { "Move or rename" }, what);
                Some(input_dialog(grid, if *op == Op::Copy { "Copy" } else { "Move" }, &prompt, line, 64, theme))
            }
            Some(Dialog::Mkdir { line }) => Some(input_dialog(grid, "Make directory", "Name (parents are made too):", line, 50, theme)),
            Some(Dialog::NewFile { line }) => Some(input_dialog(grid, "Edit new file", "Name:", line, 50, theme)),
            Some(Dialog::Delete { job, count, selected }) => {
                let lines = [format!("Delete {} {}", count, if *count == 1 { "entry" } else { "entries" }), format!("({} files and directories in all)?", job.steps.len())];
                message(grid, "Delete", &[lines[0].as_str(), lines[1].as_str()], &["Delete", "Cancel"], *selected, theme);
                None
            }
            Some(Dialog::Overwrite { count, selected, .. }) => {
                let line = format!("{} of the targets exist already.", count);
                message(grid, "Overwrite", &[line.as_str(), "Overwrite them, skip them, or cancel?"], &["Overwrite", "Skip", "Cancel"], *selected, theme);
                None
            }
            Some(Dialog::Results { mask, found, list }) => {
                let rows = 12usize;
                let inner = dialog(grid, &format!("Found {} for {}", found.len(), mask), (w * 3 / 4).max(30).min(w), rows + 4, theme);
                list.scroll(found.len(), rows);
                for (i, path) in found.iter().enumerate().skip(list.top).take(rows) {
                    let style = if i == list.selected { theme.selected } else { theme.dialog };
                    grid.text_padded(inner.x + 1, inner.y + 1 + i - list.top, &display(path), inner.w.saturating_sub(2), style);
                }
                if found.is_empty() { grid.text(inner.x + 1, inner.y + 1, "Nothing found", theme.dialog); }
                grid.text(inner.x + 1, inner.bottom() - 1, "Enter: go to   F3: view   Esc: close", theme.dialog);
                None
            }
            Some(Dialog::Volume { side, list, lines }) => {
                let inner = dialog(grid, if *side == 0 { "Left panel volume" } else { "Right panel volume" }, 64, lines.len() + 4, theme);
                for (i, volume) in lines.iter().enumerate() {
                    grid.text_padded(inner.x + 1, inner.y + 1 + i, volume, inner.w.saturating_sub(2), if i == list.selected { theme.selected } else { theme.dialog });
                }
                None
            }
        }
    }

    /// One line of state for the log after each key (tests follow it).
    pub fn status(&self) -> String {
        if let Some(editor) = &self.editor { return format!("EDITOR {}", editor.status()); }
        let mode = |p: &Panel| match p.mode { Mode::Brief => "BRIEF", Mode::Full => "FULL", Mode::Info => "INFO", Mode::Quick => "QUICK" };
        let dialog = match self.dialog { None => "NONE", Some(Dialog::Help) => "HELP", Some(Dialog::Message { .. }) => "MESSAGE", Some(Dialog::Mask { .. }) => "MASK",
                                         Some(Dialog::Find { .. }) => "FIND", Some(Dialog::Results { .. }) => "RESULTS", Some(Dialog::Volume { .. }) => "VOLUME",
                                         Some(Dialog::Target { .. }) => "TARGET", Some(Dialog::Mkdir { .. }) => "MKDIR", Some(Dialog::NewFile { .. }) => "NEWFILE",
                                         Some(Dialog::Delete { .. }) => "DELETE", Some(Dialog::Overwrite { .. }) => "OVERWRITE" };
        let job = match &self.job { None => String::from("NONE"), Some(job) => format!("{}:{}/{}{}", job.op.name(), job.index, job.steps.len(), if job.failure.is_some() { ":FAILED" } else if job.asking { ":ASKING" } else { "" }) };
        let panel = &self.panels[self.active];
        format!("LEFT=/{} {} RIGHT=/{} {} ACTIVE={} CURRENT={} MARKED={} DIALOG={} MENU={} VIEW={} JOB={}", self.panels[0].path, mode(&self.panels[0]), self.panels[1].path,
                mode(&self.panels[1]), if self.active == 0 { "L" } else { "R" }, panel.current().map_or("", |e| e.name.as_str()), panel.marked.len(), dialog,
                self.menu.open as u8, self.viewer.as_ref().map_or(0, |v| v.top() as usize + 1), job)
            + &if self.command.is_empty() { String::new() } else { format!(" CMD={}", self.command.as_str()) }
            + match self.hidden { [false, false] => "", [true, false] => " HIDDEN=L", [false, true] => " HIDDEN=R", [true, true] => " HIDDEN=LR" }
    }
}

fn mask_line() -> InputLine { let mut line = InputLine::new(); line.set("*.*"); line }
