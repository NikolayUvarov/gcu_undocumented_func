#![no_std]
#![no_main]
// camera: what a camera sees (issue 158), through the video gateway's client the shell lends in SLOT_CAMERA once the
// user agreed (REQUEST_CAMERA). It shows the stream on its screen (a window in wm), takes a still as a BMP, or records
// an AVI of Motion JPEG frames (as `record`, issue 093). While the stream is open the compositor shows the camera mark.
// The camera's modes (video.wit 1.1, 158-APP-0152): -m picks one, m and Shift+m step through those offered, each opened
// at its own size and rate; the mode in use is in the log and the title.
extern crate alloc;
#[path = "../../shell/src/bmp.rs"]
mod bmp;
mod modes;
mod pick;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{BootInfo, CAP_GRANT, CAP_KIND_ENDPOINT, CAP_READ, CAP_WRITE, SLOT_CAMERA, SLOT_FILE};
use mind::avi::{self, Index};
use mind::fs::File;
use mind::gfx::Screen;
use mind::idl::video as idl;
use mind::ipc::Endpoint;
use mind::jpeg::{Encoder, Rows};
use mind::mem::Pages;

mind::request!(REQUEST_CAMERA | REQUEST_FILES);

const CAMERA: Endpoint = Endpoint(SLOT_CAMERA);
const QUALITY: u8 = 75;

struct Options { width: usize, height: usize, rate: u8, camera: u8, mode: Option<u16>, still: Option<String>, video: Option<(u32, String)> }

// The stream's size and rate: as asked, or a mode's own.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Shape { width: usize, height: usize, rate: u8 }

impl Shape { fn bytes(&self) -> usize { self.width * self.height * 4 } }

fn options(args: &str) -> Result<Options, String> {
    let mut options = Options { width: 320, height: 240, rate: 10, camera: 0, mode: None, still: None, video: None };
    let mut words = args.split_whitespace();
    let usage = || String::from("usage: camera [-c n] [-m mode] [-z WxH] [-r fps] [-s still.bmp] [-t seconds video.avi]");
    while let Some(word) = words.next() {
        match word {
            "-z" => {
                let (w, h) = words.next().and_then(|s| s.split_once(['x', 'X'])).ok_or_else(usage)?;
                (options.width, options.height) = (w.parse().map_err(|_| usage())?, h.parse().map_err(|_| usage())?);
            }
            "-r" => options.rate = words.next().and_then(|s| s.parse().ok()).ok_or_else(usage)?,
            "-c" => options.camera = words.next().and_then(|s| s.parse().ok()).ok_or_else(usage)?,
            "-m" => options.mode = Some(words.next().and_then(|s| s.parse().ok()).ok_or_else(usage)?),
            "-s" => options.still = Some(String::from(words.next().ok_or_else(usage)?)),
            "-t" => {
                let seconds = words.next().and_then(|s| s.parse().ok()).filter(|&s| s > 0).ok_or_else(usage)?;
                options.video = Some((seconds, String::from(words.next().ok_or_else(usage)?)));
            }
            _ => return Err(usage()),
        }
    }
    Ok(options)
}

// The frame buffer the gateway writes into: shared once, lent writable for each read and revoked after it.
struct Buffer { pages: Pages, cap: usize }

// A buffer given up for another mode's takes its shared capability with it; the pages go after.
impl Drop for Buffer { fn drop(&mut self) { let _ = mind::ipc::drop_cap(self.cap); } }

impl Buffer {
    // A buffer for frames of `bytes`; None without the memory or the share.
    fn new(bytes: usize) -> Option<Buffer> { let pages = Pages::new(bytes)?; let cap = pages.share().ok()?; Some(Buffer { pages, cap }) }

    fn read(&mut self, bytes: usize) -> Result<(idl::Frame, &[u32]), String> {
        let lent = mind::ipc::mint(self.cap, CAP_READ | CAP_WRITE | CAP_GRANT, 0, 0).map_err(|e| format!("cannot lend the buffer: {:?}", e))?;
        let read = idl::read(CAMERA, bytes as u32, lent);
        let _ = mind::ipc::revoke(self.cap);
        read.map_err(|e| format!("the gateway does not answer: {:?}", e))?.map_err(|e| format!("read: {:?}", e))?;
        let frame = idl::frame(CAMERA).map_err(|e| format!("the gateway does not answer: {:?}", e))?.map_err(|e| format!("frame: {:?}", e))?;
        let pixels = unsafe { core::slice::from_raw_parts(self.pages.as_slice().as_ptr() as *const u32, bytes / 4) };
        Ok((frame, pixels))
    }
}

// Where a frame goes: in a window scaled to fit it whole, its aspect kept; on a screen at its own size, clipped; centred
// either way. The source column and row of each place are worked out once a size.
struct Place { x0: usize, y0: usize, columns: Vec<usize>, rows: Vec<usize> }

fn place(screen: &Screen, width: usize, height: usize, fit: bool) -> Place {
    let (w, h) = if !fit { (width.min(screen.width), height.min(screen.height)) }
                 else if screen.width * height <= screen.height * width { (screen.width, (screen.width * height / width).max(1)) }
                 else { ((screen.height * width / height).max(1), screen.height) };
    Place { x0: (screen.width - w) / 2, y0: (screen.height - h) / 2, columns: (0..w).map(|x| x * width / w).collect(), rows: (0..h).map(|y| y * height / h).collect() }
}

// A frame drawn at its place: the nearest source pixel for each one, so at its own size it is copied unchanged.
fn show(screen: &Screen, place: &Place, pixels: &[u32], width: usize) {
    for (y, &row) in place.rows.iter().enumerate() {
        let line = &pixels[row * width..(row + 1) * width];
        for (x, &column) in place.columns.iter().enumerate() { screen.pixel(place.x0 + x, place.y0 + y, line[column]); }
    }
}

fn write_still(path: &str, pixels: &[u32], width: usize, height: usize) -> Result<usize, String> {
    let mut file = File::create(path).map_err(|e| format!("{}: {:?}", path, e))?;
    let mut out = Vec::with_capacity(bmp::file_bytes(width, height));
    out.extend_from_slice(&bmp::header(width, height));
    let mut row = alloc::vec![0u8; bmp::row_bytes(width)];
    for y in (0..height).rev() { let n = bmp::row(&pixels[y * width..(y + 1) * width], &mut row); out.extend_from_slice(&row[..n]); }
    file.write_at(0, &out).map_err(|e| format!("{}: {:?}", path, e))?;
    file.flush().map_err(|e| format!("{}: {:?}", path, e))?;
    Ok(out.len())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Asked { Nothing, Quit, Next, Mode(bool) }

// Esc or q: the stream is closed before the program ends; n or Tab: the next camera (158-APP-0147); m and Shift+m: the
// next and the earlier mode (158-APP-0152).
fn asked() -> Asked {
    let mut asked = Asked::Nothing;
    while let Some(key) = mind::input::read_key() {
        if key.is_escape() || key.char() == Some('q') { asked = Asked::Quit; }
        else if asked == Asked::Nothing && matches!(key.char(), Some('n' | '\t')) { asked = Asked::Next; }
        else if asked == Asked::Nothing && matches!(key.char(), Some('m' | 'M')) { asked = Asked::Mode(key.char() == Some('m')); }
    }
    asked
}

fn open(camera: usize, shape: Shape) -> mind::sys::Result<Result<(), idl::Error>> { idl::open(CAMERA, camera as u8, shape.width as u16, shape.height as u16, shape.rate) }

// The modes the gateway lists for `camera`; None from a gateway of video.wit 1.0, which rejects the call.
fn modes_of(camera: usize) -> Option<Vec<idl::Mode>> {
    match idl::modes(CAMERA, camera as u8) {
        Ok(Ok(list)) => Some(list.as_slice().to_vec()),
        Ok(Err(error)) => { mind::println!("[CAMERA] THE GATEWAY LISTS NO MODES FOR CAMERA {}: {:?}", camera, error); Some(Vec::new()) }
        Err(_) => None,
    }
}

// The cameras the gateway lists, the one shown, its modes and the one in use.
struct Cameras { list: mind::idl::codec::List<idl::Camera, 4>, current: usize, modes: Option<Vec<idl::Mode>>, mode: Option<usize>, mode_text: String }

impl Cameras {
    fn name(&self) -> &str { self.list.as_slice()[self.current].name.as_str() }
    fn title(&self) -> String {
        let title = pick::title(self.current, self.list.len(), self.name());
        if self.mode_text.is_empty() { title } else { format!("{title}, {}", self.mode_text) }
    }
    fn offered(&self) -> Vec<bool> { self.modes.iter().flatten().map(|m| m.offered).collect() }

    // After an open: what the gateway opened (video.wit 1.1's `opened`), in the log and the title.
    fn opened(&mut self, shape: Shape) {
        let mut line = format!("[CAMERA] OPENED {} {}X{} AT {}/S", self.name(), shape.width, shape.height, shape.rate);
        (self.mode, self.mode_text) = (None, String::new());
        if let Ok(Ok(o)) = idl::opened(CAMERA) {
            let count = self.modes.as_ref().map_or(0, Vec::len);
            let what = modes::describe(o.mode.encoding as usize, o.mode.width, o.mode.height, o.mode.rate);
            if (o.index as usize) < count {
                self.mode = Some(o.index as usize);
                line += &format!(": MODE {} (OF 0 TO {}), {}", o.index, count - 1, what);
            } else {
                line += &format!(": NONE OF ITS {} MODES, {}", count, what);
            }
            if o.scaled { line += ", SCALED BY THE GATEWAY"; }
            self.mode_text = what;
        }
        mind::println!("{}", line);
        if let Some(surface) = mind::windowed::surface() { surface.set_title(&self.title()); }
    }
}

// The stream moved to the next camera, at the same size and rate; the one before again if the next does not open.
fn switch(cameras: &mut Cameras, shape: Shape) {
    let count = cameras.list.len();
    if count < 2 { mind::println!("[CAMERA] ONE CAMERA ONLY: {}", cameras.name()); return; }
    let before = cameras.current;
    let next = pick::next(before, count);
    let _ = idl::close(CAMERA);
    match open(next, shape) {
        Ok(Ok(())) => {
            cameras.current = next;
            mind::println!("[CAMERA] SWITCHED TO {} ({}/{})", cameras.name(), next + 1, count);
        }
        other => {
            let name = cameras.list.as_slice()[next].name.as_str();
            mind::println!("[CAMERA] {} DID NOT OPEN AT {}X{} {}/S ({:?}); BACK TO {}", name, shape.width, shape.height, shape.rate, other, cameras.name());
            let _ = open(before, shape);
        }
    }
    cameras.modes = modes_of(cameras.current);
    cameras.opened(shape);
}

// The stream moved to the next mode the gateway offers (the earlier one, with `forward` false), at its own size and
// rate, with a buffer of that size; the mode before again if it does not open. The new shape and buffer, or None.
fn step_mode(cameras: &mut Cameras, shape: Shape, forward: bool) -> Option<(Shape, Buffer)> {
    let Some(list) = cameras.modes.clone() else { mind::println!("[CAMERA] THE GATEWAY LISTS NO MODES (VIDEO.WIT 1.0)"); return None };
    let Some(next) = modes::step(&cameras.offered(), cameras.mode, forward) else {
        mind::println!("[CAMERA] ONE MODE ONLY: {} OFFERS {} OF {} MODES", cameras.name(), cameras.offered().iter().filter(|o| **o).count(), list.len());
        return None;
    };
    let m = list[next];
    let new = Shape { width: m.width as usize, height: m.height as usize, rate: m.rate.max(1) };
    let what = modes::describe(m.encoding as usize, m.width, m.height, m.rate);
    let Some(buffer) = Buffer::new(new.bytes()) else { mind::println!("[CAMERA] NO MEMORY FOR A FRAME OF MODE {}, {}", next, what); return None };
    let _ = idl::close(CAMERA);
    match open(cameras.current, new) {
        Ok(Ok(())) => { cameras.opened(new); Some((new, buffer)) }
        other => {
            mind::println!("[CAMERA] MODE {} ({}) DID NOT OPEN ({:?}); BACK TO {}X{} AT {}/S", next, what, other, shape.width, shape.height, shape.rate);
            let _ = open(cameras.current, shape);
            cameras.opened(shape);
            None
        }
    }
}

fn fail(message: &str) -> ! { mind::println!("camera: {}", message); mind::process::exit_with(1) }

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("camera — what a camera sees, through the video gateway (lent by the shell as it starts): on its screen or in a\nwindow, a still (BMP) or a recording (AVI, Motion JPEG). A green mark in the screen's corner shows a camera is on.\nUsage: camera [-c n] [-m mode] [-z WxH] [-r fps] [-s still.bmp] [-t seconds video.avi]   (default: camera 0, 320x240 at 10 per second)\n-m: one of the camera's modes, at its own size and rate. n or Tab: the next camera. m, Shift+m: the next and the earlier mode. Esc or q: stop.");
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let options = match options(mind::process::args_str()) { Ok(o) => o, Err(e) => fail(&e) };
    if mind::dev::cap_info(SLOT_CAMERA).0 != CAP_KIND_ENDPOINT { fail("no camera was granted (start camera from the shell and allow it)") }
    let list = idl::cameras(CAMERA).unwrap_or_default();
    let current = match pick::chosen(options.camera, list.len()) { Ok(index) => index, Err(e) => fail(&e) };
    let mut cameras = Cameras { list, current, modes: modes_of(current), mode: None, mode_text: String::new() };
    // -m: that mode's own size and rate, in place of -z and -r.
    let shape = match options.mode {
        None => Shape { width: options.width, height: options.height, rate: options.rate },
        Some(asked) => {
            let Some(list) = cameras.modes.clone() else { fail("-m: the gateway lists no modes (video.wit 1.0)") };
            let m = match modes::chosen(asked, &cameras.offered(), current) { Ok(index) => list[index], Err(e) => fail(&e) };
            Shape { width: m.width as usize, height: m.height as usize, rate: m.rate.max(1) }
        }
    };
    match open(current, shape) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => fail(&format!("cannot open {} at {}x{} {}/s: {:?}", cameras.name(), shape.width, shape.height, shape.rate, error)),
        Err(error) => fail(&format!("the gateway does not answer: {:?}", error)),
    }
    let Some(mut buffer) = Buffer::new(shape.bytes()) else { let _ = idl::close(CAMERA); fail("no memory for a frame") };
    let shown = mind::windowed::pixels(info, shape.width, shape.height, &cameras.title());
    let windowed = !core::ptr::eq(shown, info);
    cameras.opened(shape);
    let screen = Screen::new(shown);
    if let Some(screen) = &screen { screen.clear(0); }
    let result = run(&options, shape, &mut cameras, &mut buffer, screen, windowed);
    let _ = idl::close(CAMERA);
    match result { Ok(summary) => mind::println!("[CAMERA] {}", summary), Err(e) => fail(&e) }
}

// Shows frames until Esc, or takes the still, or records; returns what it did.
fn run(options: &Options, mut shape: Shape, cameras: &mut Cameras, buffer: &mut Buffer, mut screen: Option<Screen>, windowed: bool) -> Result<String, String> {
    let (mut width, mut height) = (shape.width, shape.height);
    let mut at = screen.as_ref().map(|screen| place(screen, width, height, windowed));
    let mut bytes = shape.bytes();
    let mut recording = match &options.video {
        Some((seconds, path)) => {
            let mut file = File::create(path).map_err(|e| format!("{}: {:?}", path, e))?;
            file.write_at(0, &avi::header(width as u32, height as u32, shape.rate as u32, &Index::default(), false)).map_err(|e| format!("{}: {:?}", path, e))?;
            Some((file, Index::default(), *seconds * shape.rate as u32, path.as_str()))
        }
        None => None,
    };
    let (encoder, mut rows, mut jpeg) = (Encoder::new(QUALITY), Rows::default(), Vec::new());
    // Frames come numbered at the rate; a reader that fell behind gets the latest one, so their times stay on the rate's
    // grid (start + (n - 1) / rate) whatever the reader's speed. A recording keeps camera time: a number it did not get
    // is the picture before it again (an empty chunk), as `record` keeps screen time.
    let mut period_us = 1_000_000 / shape.rate.max(1) as u64;
    let (mut first, mut last, mut on_grid, mut frames, mut pictures) = (None::<idl::Frame>, None::<idl::Frame>, true, 0u32, 0u32);
    loop {
        let (frame, pixels) = buffer.read(bytes)?;
        // A window the manager gave another size: the frame is scaled to it from now on (158).
        if let Some(resized) = mind::windowed::pixels_resized() {
            if let Some(new) = Screen::new(&resized) { new.clear(0); at = Some(place(&new, width, height, true)); screen = Some(new); }
        }
        // In a window the frame is published at once, or wm shows it only when something else redraws (158).
        if let (Some(screen), Some(at)) = (&screen, &at) { show(screen, at, pixels, width); mind::windowed::flush(); }
        let start = *first.get_or_insert(frame);
        on_grid &= last.is_none_or(|l| frame.sequence > l.sequence) && frame.timestamp_us - start.timestamp_us == (frame.sequence - start.sequence) as u64 * period_us;
        last = Some(frame);
        frames += 1;
        if let Some(path) = &options.still {
            let size = write_still(path, pixels, width, height)?;
            return Ok(format!("STILL {}: {}X{}, FRAME {}, {} BYTES", path, width, height, frame.sequence, size));
        }
        if let Some((file, index, total, path)) = recording.as_mut() {
            let put = |file: &mut File, index: &mut Index, data: &[u8]| -> Result<(), String> {
                let (at, chunk) = index.add(data.len() as u32);
                let written = file.write_at(at, &chunk).and_then(|_| if data.is_empty() { Ok(0) } else { file.write_at(at + 8, data) })
                    .and_then(|_| if data.len() % 2 == 1 { file.write_at(at + 8 + data.len(), &[0]) } else { Ok(0) });
                written.map(drop).map_err(|e| format!("{}: {:?}", path, e))
            };
            // The numbers between the last picture and this one: the last picture again.
            while (index.frames.len() as u32) < *total && start.sequence + index.frames.len() as u32 != frame.sequence { put(file, index, &[])?; }
            if (index.frames.len() as u32) < *total {
                jpeg.clear();
                encoder.encode_again(pixels, None, width, height, width, &mut rows, &mut jpeg);
                put(file, index, &jpeg)?;
                pictures += 1;
            }
            if index.frames.len() as u32 >= *total {
                let table = index.index();
                file.write_at(index.end(), &table).map_err(|e| format!("{}: {:?}", path, e))?;
                file.write_at(0, &avi::header(width as u32, height as u32, shape.rate as u32, index, true)).map_err(|e| format!("{}: {:?}", path, e))?;
                file.flush().map_err(|e| format!("{}: {:?}", path, e))?;
                return Ok(format!("VIDEO {}: {} FRAMES ({} PICTURES) {}X{} AT {}/S, SEQUENCE {}..{}, TIMESTAMPS {}, {} BYTES", path, total, pictures, width, height, shape.rate,
                                  start.sequence, start.sequence + *total - 1, if on_grid { "ON THE RATE" } else { "OFF THE RATE" }, index.end() + table.len()));
            }
        }
        match asked() {
            Asked::Quit => return Ok(format!("SHOWN {} FRAMES", frames)),
            // Only while it shows: a still or a recording keeps its camera to the end.
            Asked::Next if options.still.is_none() && recording.is_none() => {
                switch(cameras, shape);
                (first, last) = (None, None);
            }
            // A mode of its own size: the frames' buffer, place and period follow (158-APP-0152).
            Asked::Mode(forward) if options.still.is_none() && recording.is_none() => {
                if let Some((new, fresh)) = step_mode(cameras, shape, forward) {
                    *buffer = fresh;
                    shape = new;
                    (width, height, bytes, period_us) = (new.width, new.height, new.bytes(), 1_000_000 / new.rate.max(1) as u64);
                    if let Some(screen) = &screen { screen.clear(0); at = Some(place(screen, width, height, windowed)); }
                    (first, last) = (None, None);
                }
            }
            _ => {}
        }
    }
}
