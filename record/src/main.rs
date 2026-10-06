#![no_std]
#![no_main]
// record (issue 093): what is on the screen, as an AVI file of Motion JPEG frames. The screen comes through the
// compositor client the shell lends for REQUEST_DISPLAY (idl/display.wit: a sealed copy of the screen in front); while
// it records, the compositor shows a red dot in the screen's corner (issue 164). Started in the background
// (`run record -t 10 &`), it records the program then brought to the front.
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_DISPLAY, SLOT_FILE};
use mind::avi::{self, Index};
use mind::fs::File;
use mind::idl::display;
use mind::ipc::Endpoint;
use mind::jpeg::{Encoder, Rows};
use mind::mem::Mapping;

mind::request!(REQUEST_CONSOLE | REQUEST_FILES | REQUEST_DISPLAY);

const DISPLAY: Endpoint = Endpoint(SLOT_DISPLAY);
// Each capture's memory arrives here (21 is the shell's broker client; no application without a window uses it).
const RECEIVE: usize = 21;
const QUALITY: u8 = 75;
const USAGE: &str = "record [-r frames per second, 1-30] [-t seconds, 1-3600] [file]   (default: 10 per second, 10 s, ram:record-NNN.avi)";

struct Options { fps: u32, seconds: u32, path: String }

fn options(args: &str) -> Result<Options, String> {
    let mut options = Options { fps: 10, seconds: 10, path: String::new() };
    let mut words = args.split_whitespace();
    while let Some(word) = words.next() {
        let number = |value: Option<&str>, low: u32, high: u32| value.and_then(|v| v.parse::<u32>().ok()).filter(|n| (low..=high).contains(n));
        match word {
            "-r" => options.fps = number(words.next(), 1, 30).ok_or_else(|| String::from(USAGE))?,
            "-t" => options.seconds = number(words.next(), 1, 3600).ok_or_else(|| String::from(USAGE))?,
            _ if word.starts_with('-') || !options.path.is_empty() => return Err(String::from(USAGE)),
            _ => options.path = String::from(word),
        }
    }
    if options.path.is_empty() {
        let free = (1..1000).map(|n| format!("ram:record-{:03}.avi", n)).find(|name| mind::fs::metadata(name).is_err());
        options.path = free.ok_or_else(|| String::from("no free name on ram:"))?;
    }
    Ok(options)
}

// The screen in front now: a mapping of the compositor's sealed copy.
fn capture() -> Result<Mapping, &'static str> {
    match display::capture(DISPLAY, RECEIVE) {
        Ok(Ok(())) => {}
        Ok(Err(display::Error::NoScreen)) => return Err("nothing on the screen"),
        Ok(Err(_)) => return Err("no memory for the copy"),
        Err(_) => return Err("the compositor does not answer"),
    }
    let mapping = Mapping::new(RECEIVE).map_err(|_| "cannot map the copy");
    let _ = mind::ipc::drop_cap(RECEIVE); // the mapping keeps the memory
    mapping
}

fn pixels(mapping: &Mapping, count: usize) -> &[u32] { unsafe { core::slice::from_raw_parts(mapping.as_ptr::<u32>() as *const u32, count) } }

struct Recording { file: File, index: Index, width: u32, height: u32, fps: u32, encoded: u32, bytes: usize, encoding_ms: usize, rows: usize }

impl Recording {
    // A frame of `data` (none: the previous one again).
    fn frame(&mut self, data: &[u8]) -> Result<(), mind::fs::Error> {
        let (at, chunk) = self.index.add(data.len() as u32);
        self.file.write_at(at, &chunk)?;
        if !data.is_empty() {
            self.file.write_at(at + 8, data)?;
            if data.len() % 2 == 1 { self.file.write_at(at + 8 + data.len(), &[0])?; }
            self.encoded += 1;
        }
        self.bytes = self.index.end();
        Ok(())
    }
    fn header(&mut self, indexed: bool) -> Result<(), mind::fs::Error> {
        self.file.write_at(0, &avi::header(self.width, self.height, self.fps, &self.index, indexed)).map(|_| ())
    }
    // The index after the frames, the header with the final counts.
    fn finish(&mut self) -> Result<(), mind::fs::Error> {
        let index = self.index.index();
        self.file.write_at(self.index.end(), &index)?;
        self.bytes = self.index.end() + index.len();
        self.header(true)?;
        self.file.flush()
    }
}

mind::entry!(main);
fn main(_info: &'static mind::BootInfo) {
    mind::about!("record — records the screen in front as an AVI file (Motion JPEG); a red dot in the corner shows it is recording.\nUsage: record [-r fps] [-t seconds] [file]   (default: 10 per second, 10 s, ram:record-NNN.avi)\nStart it in the background (run record -t 10 &), then bring the program to record to the front.");
    // The user's files the shell lends for REQUEST_FILES (ram: and data/ writable); else the read-only client.
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let options = match options(mind::process::args_str()) { Ok(o) => o, Err(e) => { mind::println!("record: {}", e); return; } };
    let Ok(mode) = display::mode(DISPLAY) else { mind::println!("record: no compositor client (start it from the shell)"); return };
    let (width, height, stride) = (mode.width as usize, mode.height as usize, mode.stride as usize);
    let file = match File::create(&options.path) { Ok(f) => f, Err(e) => { mind::println!("record: {}: {:?}", options.path, e); return; } };
    let mut recording = Recording { file, index: Index::default(), width: mode.width, height: mode.height, fps: options.fps, encoded: 0, bytes: avi::HEADER, encoding_ms: 0, rows: 0 };
    if let Err(e) = recording.header(false) { mind::println!("record: {}: {:?}", options.path, e); return; }
    mind::println!("[RECORD] {} {}X{} AT {}/S FOR {} S", options.path, width, height, options.fps, options.seconds);
    let encoder = Encoder::new(QUALITY);
    let total = options.fps * options.seconds;
    let period = 1000.0 / options.fps as f32;
    let start = mind::time::uptime_ms();
    let mut previous: Option<Mapping> = None;
    let mut jpeg = Vec::new();
    let mut rows = Rows::default();
    let mut problem = None;
    while recording.index.frames.len() < total as usize {
        // Frame k is due at start + k periods: the screen as it is then.
        let due = start + (recording.index.frames.len() as f32 * period) as usize;
        let now = mind::time::uptime_ms();
        if now < due { mind::time::sleep(due - now); }
        let screen = match capture() { Ok(s) => s, Err(e) => { problem = Some(e); break; } };
        let count = stride * height;
        let before = previous.as_ref().map(|p| pixels(p, count));
        jpeg.clear();
        // An unchanged screen repeats the last frame; a changed one codes again only the rows of blocks that changed.
        if before != Some(pixels(&screen, count)) {
            let began = mind::time::uptime_ms();
            recording.rows += encoder.encode_again(pixels(&screen, count), before, width, height, stride, &mut rows, &mut jpeg);
            recording.encoding_ms += mind::time::uptime_ms() - began;
        }
        if recording.frame(&jpeg).is_err() { problem = Some("the disk is full"); break; }
        previous = Some(screen);
        // Frames whose time passed while this one was encoded show it too.
        let elapsed = (mind::time::uptime_ms() - start) as f32;
        while ((recording.index.frames.len() as f32) * period) < elapsed - period && recording.index.frames.len() < total as usize {
            if recording.frame(&[]).is_err() { problem = Some("the disk is full"); break; }
        }
        if recording.index.frames.len() % options.fps as usize == 0 { let _ = recording.header(false); }
    }
    drop(previous);
    if let Err(e) = recording.finish() { mind::println!("record: {}: {:?}", options.path, e); return; }
    let frames = recording.index.frames.len();
    mind::println!("[RECORD] {}: {} FRAMES ({} ENCODED, {} ROWS OF BLOCKS CODED, {} MS EACH), {}X{} AT {}/S, {} BYTES{}", options.path, frames,
                   recording.encoded, recording.rows, recording.encoding_ms / recording.encoded.max(1) as usize, width, height, options.fps, recording.bytes,
                   problem.map_or(String::new(), |p| format!(" (STOPPED: {})", p)));
}
