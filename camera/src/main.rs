#![no_std]
#![no_main]
// camera: what a camera sees (issue 158), through the video gateway's client the shell lends in SLOT_CAMERA once the
// user agreed (REQUEST_CAMERA). It shows the stream on its screen (a window in wm), takes a still as a BMP, or records
// an AVI of Motion JPEG frames (as `record`, issue 093). While the stream is open the compositor shows the camera mark.
extern crate alloc;
#[path = "../../shell/src/bmp.rs"]
mod bmp;
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

struct Options { width: usize, height: usize, rate: u8, camera: u8, still: Option<String>, video: Option<(u32, String)> }

fn options(args: &str) -> Result<Options, String> {
    let mut options = Options { width: 320, height: 240, rate: 10, camera: 0, still: None, video: None };
    let mut words = args.split_whitespace();
    let usage = || String::from("usage: camera [-c n] [-z WxH] [-r fps] [-s still.bmp] [-t seconds video.avi]");
    while let Some(word) = words.next() {
        match word {
            "-z" => {
                let (w, h) = words.next().and_then(|s| s.split_once(['x', 'X'])).ok_or_else(usage)?;
                (options.width, options.height) = (w.parse().map_err(|_| usage())?, h.parse().map_err(|_| usage())?);
            }
            "-r" => options.rate = words.next().and_then(|s| s.parse().ok()).ok_or_else(usage)?,
            "-c" => options.camera = words.next().and_then(|s| s.parse().ok()).ok_or_else(usage)?,
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

impl Buffer {
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
enum Asked { Nothing, Quit, Next }

// Esc or q: the stream is closed before the program ends; n or Tab: the next camera (158-APP-0147).
fn asked() -> Asked {
    let mut asked = Asked::Nothing;
    while let Some(key) = mind::input::read_key() {
        if key.is_escape() || key.char() == Some('q') { asked = Asked::Quit; }
        else if asked == Asked::Nothing && matches!(key.char(), Some('n' | '\t')) { asked = Asked::Next; }
    }
    asked
}

// The cameras the gateway lists, and the one shown.
struct Cameras { list: mind::idl::codec::List<idl::Camera, 4>, current: usize }

impl Cameras {
    fn name(&self) -> &str { self.list.as_slice()[self.current].name.as_str() }
    fn title(&self) -> String { pick::title(self.current, self.list.len(), self.name()) }
}

// The stream moved to the next camera, at the same size and rate; the one before again if the next does not open.
fn switch(cameras: &mut Cameras, options: &Options) {
    let count = cameras.list.len();
    if count < 2 { mind::println!("[CAMERA] ONE CAMERA ONLY: {}", cameras.name()); return; }
    let before = cameras.current;
    let next = pick::next(before, count);
    let _ = idl::close(CAMERA);
    let open = |index: usize| idl::open(CAMERA, index as u8, options.width as u16, options.height as u16, options.rate);
    match open(next) {
        Ok(Ok(())) => {
            cameras.current = next;
            mind::println!("[CAMERA] SWITCHED TO {} ({}/{})", cameras.name(), next + 1, count);
        }
        other => {
            let name = cameras.list.as_slice()[next].name.as_str();
            mind::println!("[CAMERA] {} DID NOT OPEN AT {}X{} {}/S ({:?}); BACK TO {}", name, options.width, options.height, options.rate, other, cameras.name());
            let _ = open(before);
        }
    }
    if let Some(surface) = mind::windowed::surface() { surface.set_title(&cameras.title()); }
}

fn fail(message: &str) -> ! { mind::println!("camera: {}", message); mind::process::exit_with(1) }

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("camera — what a camera sees, through the video gateway (lent by the shell as it starts): on its screen or in a\nwindow, a still (BMP) or a recording (AVI, Motion JPEG). A green mark in the screen's corner shows a camera is on.\nUsage: camera [-c n] [-z WxH] [-r fps] [-s still.bmp] [-t seconds video.avi]   (default: camera 0, 320x240 at 10 per second)\nn or Tab: the next camera. Esc or q: stop.");
    if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let options = match options(mind::process::args_str()) { Ok(o) => o, Err(e) => fail(&e) };
    if mind::dev::cap_info(SLOT_CAMERA).0 != CAP_KIND_ENDPOINT { fail("no camera was granted (start camera from the shell and allow it)") }
    let list = idl::cameras(CAMERA).unwrap_or_default();
    let current = match pick::chosen(options.camera, list.len()) { Ok(index) => index, Err(e) => fail(&e) };
    let mut cameras = Cameras { list, current };
    let (width, height) = (options.width, options.height);
    match idl::open(CAMERA, current as u8, width as u16, height as u16, options.rate) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => fail(&format!("cannot open {} at {}x{} {}/s: {:?}", cameras.name(), width, height, options.rate, error)),
        Err(error) => fail(&format!("the gateway does not answer: {:?}", error)),
    }
    mind::println!("[CAMERA] OPENED {} {}X{} AT {}/S", cameras.name(), width, height, options.rate);
    let bytes = width * height * 4;
    let Some(pages) = Pages::new(bytes) else { let _ = idl::close(CAMERA); fail("no memory for a frame") };
    let Ok(cap) = pages.share() else { let _ = idl::close(CAMERA); fail("cannot share the frame buffer") };
    let mut buffer = Buffer { pages, cap };
    let shown = mind::windowed::pixels(info, width, height, &cameras.title());
    let windowed = !core::ptr::eq(shown, info);
    let screen = Screen::new(shown);
    if let Some(screen) = &screen { screen.clear(0); }
    let result = run(&options, &mut cameras, &mut buffer, screen, windowed);
    let _ = idl::close(CAMERA);
    match result { Ok(summary) => mind::println!("[CAMERA] {}", summary), Err(e) => fail(&e) }
}

// Shows frames until Esc, or takes the still, or records; returns what it did.
fn run(options: &Options, cameras: &mut Cameras, buffer: &mut Buffer, mut screen: Option<Screen>, windowed: bool) -> Result<String, String> {
    let (width, height) = (options.width, options.height);
    let mut at = screen.as_ref().map(|screen| place(screen, width, height, windowed));
    let bytes = width * height * 4;
    let mut recording = match &options.video {
        Some((seconds, path)) => {
            let mut file = File::create(path).map_err(|e| format!("{}: {:?}", path, e))?;
            file.write_at(0, &avi::header(width as u32, height as u32, options.rate as u32, &Index::default(), false)).map_err(|e| format!("{}: {:?}", path, e))?;
            Some((file, Index::default(), *seconds * options.rate as u32, path.as_str()))
        }
        None => None,
    };
    let (encoder, mut rows, mut jpeg) = (Encoder::new(QUALITY), Rows::default(), Vec::new());
    // Frames come numbered at the rate; a reader that fell behind gets the latest one, so their times stay on the rate's
    // grid (start + (n - 1) / rate) whatever the reader's speed. A recording keeps camera time: a number it did not get
    // is the picture before it again (an empty chunk), as `record` keeps screen time.
    let period_us = 1_000_000 / options.rate as u64;
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
                file.write_at(0, &avi::header(width as u32, height as u32, options.rate as u32, index, true)).map_err(|e| format!("{}: {:?}", path, e))?;
                file.flush().map_err(|e| format!("{}: {:?}", path, e))?;
                return Ok(format!("VIDEO {}: {} FRAMES ({} PICTURES) {}X{} AT {}/S, SEQUENCE {}..{}, TIMESTAMPS {}, {} BYTES", path, total, pictures, width, height, options.rate,
                                  start.sequence, start.sequence + *total - 1, if on_grid { "ON THE RATE" } else { "OFF THE RATE" }, index.end() + table.len()));
            }
        }
        match asked() {
            Asked::Quit => return Ok(format!("SHOWN {} FRAMES", frames)),
            // Only while it shows: a still or a recording keeps its camera to the end.
            Asked::Next if options.still.is_none() && recording.is_none() => {
                switch(cameras, options);
                (first, last) = (None, None);
            }
            _ => {}
        }
    }
}
