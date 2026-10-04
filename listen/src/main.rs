#![no_std]
#![no_main]
// listen [seconds]: records from the microphone through audio_gw with a level meter, reports peak and RMS,
// then plays the recording back. Esc stops early.
// listen --vad [seconds]: speech detection on the microphone (mind::voice): every utterance with its start, length and
// level. listen [--vad] --wav FILE: the same for a 16-bit PCM WAV file instead of the microphone; without --vad the
// file is converted to 16 kHz mono, measured and played back.
extern crate alloc;
use alloc::format;
use alloc::vec::Vec;
use mind::abi::BootInfo;
use mind::gfx::Screen;
use mind::mem::Pages;
use mind::voice::{Detector, Microphone, Source, Stream, Utterance, Wav};

const RATE: usize = 48_000;
const BACKGROUND: u32 = 0x00101018;
const USAGE: &str = "USAGE: LISTEN [SECONDS] | LISTEN --vad [SECONDS] | LISTEN [--vad] --wav FILE";

struct Options { vad: bool, wav: Option<&'static str>, seconds: Option<usize> }

fn options() -> Option<Options> {
    let mut options = Options { vad: false, wav: None, seconds: None };
    let mut words = mind::process::args_str().split_whitespace();
    while let Some(word) = words.next() {
        match word {
            "--vad" => options.vad = true,
            "--wav" => options.wav = Some(words.next()?),
            number => options.seconds = Some(number.parse::<usize>().ok()?),
        }
    }
    if options.wav.is_some() && options.seconds.is_some() { return None; }
    Some(options)
}

fn samples(pages: &mut Pages) -> &mut [i16] {
    let bytes = pages.as_mut_slice();
    unsafe { core::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut i16, bytes.len() / 2) }
}

fn meter(screen: &Option<Screen>, peak: i32, label: &[u8]) {
    let Some(screen) = screen else { return };
    let width = screen.width.saturating_sub(48);
    let filled = width * peak.min(32767) as usize / 32767;
    screen.fill(24, 96, width, 24, 0x00303040);
    screen.fill(24, 96, filled, 24, if peak > 26000 { 0x00FF6060 } else { 0x0060E080 });
    screen.fill(24, 136, width, 10, BACKGROUND);
    screen.text(24, 136, label, 1, 0x00E0E0E0, None);
}

fn peak(samples: &[i16]) -> i32 { samples.iter().map(|s| (*s as i32).abs()).max().unwrap_or(0) }

// Integer square root (no_std has no f64::sqrt).
fn isqrt(n: u64) -> u64 { let mut x = n; let mut y = (x + 1) / 2; while y < x { x = y; y = (x + n / x) / 2; } x }

fn rms(samples: &[i16]) -> i32 {
    let mean = if samples.is_empty() { 0 } else { samples.iter().map(|s| (*s as i64) * (*s as i64)).sum::<i64>() / samples.len() as i64 };
    isqrt(mean as u64) as i32
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let screen = Screen::new(info);
    match options() { Some(options) => run(&screen, options), None => mind::println!("{}", USAGE) }
    mind::println!("[LISTEN] DONE");
    loop { mind::input::wait_or_exit(200); }
}

fn run(screen: &Option<Screen>, options: Options) {
    let title: &[u8] = match (options.vad, options.wav.is_some()) {
        (false, false) => b"LISTEN - MICROPHONE (ESC: STOP)",
        (true, false) => b"LISTEN - SPEECH IN THE MICROPHONE (ESC: STOP)",
        (true, true) => b"LISTEN - SPEECH IN A FILE",
        (false, true) => b"LISTEN - FILE",
    };
    if let Some(screen) = screen {
        screen.clear(BACKGROUND);
        screen.text(24, 24, title, 2, 0x0080FFC0, None);
    }
    match (options.vad, options.wav) {
        (false, None) => record(screen, options.seconds.unwrap_or(3).clamp(1, 10)),
        (true, None) => match Microphone::start() {
            Ok(microphone) => detect(screen, Stream::new(microphone), Some(options.seconds.unwrap_or(10).clamp(1, 60))),
            Err(error) => mind::println!("[LISTEN] NO MICROPHONE: {:?}", error),
        },
        (vad, Some(path)) => match Wav::open(path) {
            Ok(wav) => {
                mind::println!("[LISTEN] FILE {}: {} HZ, {} CHANNELS, {} MS", path, wav.rate(), wav.channels(), wav.duration_ms());
                if vad { detect(screen, Stream::new(wav), None) } else { file(screen, Stream::new(wav)) }
            }
            Err(error) => mind::println!("[LISTEN] CANNOT READ {}: {:?}", path, error),
        },
    }
}

// Records `seconds` of 48 kHz stereo, reports the level and plays the recording back.
fn record(screen: &Option<Screen>, seconds: usize) {
    let total = seconds * RATE * 2; // interleaved L/R
    // The gateway hands out whole 4 KiB capture buffers: room for the last one, the recording is cut to `total`.
    let Some(mut buffer) = Pages::new((total * 2).next_multiple_of(4096)) else { mind::println!("[LISTEN] OUT OF MEMORY"); return };
    if let Err(error) = mind::audio::record_start() { mind::println!("[LISTEN] NO MICROPHONE: {:?}", error); return; }
    mind::println!("[LISTEN] RECORDING {} S", seconds);
    let (mut filled, mut idle, mut overflows) = (0usize, 0usize, 0usize);
    let mut stopped = false;
    while filled < total {
        let (count, overflow) = mind::audio::record_read(&mut samples(&mut buffer)[filled..]).unwrap_or((0, false));
        overflows += overflow as usize;
        if count > 0 {
            let peak = peak(&samples(&mut buffer)[filled..filled + count]);
            filled += count; idle = 0;
            meter(screen, peak, b"RECORDING");
        } else {
            idle += 1;
            if idle > 100 { mind::println!("[LISTEN] NO INPUT FROM THE MICROPHONE"); break; } // 2 s without data
            if mind::input::read_key().is_some_and(mind::input::is_escape) { stopped = true; break; }
            mind::time::sleep(20);
        }
    }
    let _ = mind::audio::record_stop();
    let filled = filled.min(total);
    let recorded = &samples(&mut buffer)[..filled];
    let peak = peak(recorded);
    mind::println!("[LISTEN] RECORDED {} FRAMES ({} MS), PEAK {}, RMS {}, OVERFLOWS {}", filled / 2, filled / 2 * 1000 / RATE, peak, rms(recorded), overflows);
    meter(screen, peak, b"PLAYBACK");
    if !stopped && filled > 0 {
        match mind::audio::play_all(recorded) { Ok(()) => mind::println!("[LISTEN] PLAYED BACK"), Err(error) => mind::println!("[LISTEN] PLAYBACK FAILED: {:?}", error) }
    }
}

fn show(screen: &Option<Screen>, index: usize, utterance: &Utterance) {
    let line = format!("[LISTEN] SPEECH AT {} MS, {} MS, LEVEL {} DBFS", utterance.start_ms, utterance.length_ms(), utterance.level);
    mind::println!("{}", line);
    let Some(screen) = screen else { return };
    let y = 168 + 20 * index;
    if y + 16 < screen.height { screen.text16(24, y, &line[9..], 0x00E0E0E0, None); }
}

/// Speech detection over a stream: the microphone for `seconds`, or a file to its end.
fn detect<S: Source>(screen: &Option<Screen>, mut stream: Stream<S>, seconds: Option<usize>) where S::Error: core::fmt::Debug {
    if let Some(seconds) = seconds { mind::println!("[LISTEN] LISTENING FOR SPEECH {} S", seconds); }
    let limit = seconds.map(|s| s * mind::voice::RATE as usize);
    let mut detector = Detector::new();
    let (mut chunk, mut heard, mut total, mut idle) = (Vec::new(), 0usize, 0usize, 0usize);
    loop {
        chunk.clear();
        match stream.read(&mut chunk) {
            Ok(_) => {}
            Err(error) => { mind::println!("[LISTEN] READ FAILED: {:?}", error); break; }
        }
        if let Some(limit) = limit { chunk.truncate(limit - total); }
        total += chunk.len();
        detector.push(&chunk, &mut |u| { show(screen, heard, &u); heard += 1; });
        if limit.is_some_and(|limit| total >= limit) || stream.finished() { break; }
        if chunk.is_empty() {
            idle += 1;
            if idle > 100 { mind::println!("[LISTEN] NO INPUT FROM THE MICROPHONE"); break; } // 2 s without data
            if mind::input::read_key().is_some_and(mind::input::is_escape) { break; }
            mind::time::sleep(20);
        } else {
            idle = 0;
            meter(screen, peak(&chunk), if detector.speaking() { b"SPEECH" } else { b"LISTENING" });
        }
    }
    detector.finish(&mut |u| { show(screen, heard, &u); heard += 1; });
    meter(screen, 0, b"DONE");
    mind::println!("[LISTEN] {} UTTERANCES IN {} MS, NOISE FLOOR {} DBFS", heard, total / 16, detector.floor_dbfs());
}

/// A file as the recognizer would hear it: 16 kHz mono, measured and played back (upsampled to 48 kHz stereo).
fn file<S: Source>(screen: &Option<Screen>, mut stream: Stream<S>) where S::Error: core::fmt::Debug {
    let mut mono = Vec::new();
    while !stream.finished() {
        if let Err(error) = stream.read(&mut mono) { mind::println!("[LISTEN] READ FAILED: {:?}", error); return; }
    }
    let peak = peak(&mono);
    mind::println!("[LISTEN] 16 KHZ MONO: {} MS, PEAK {}, RMS {}", mono.len() / 16, peak, rms(&mono));
    meter(screen, peak, b"PLAYBACK");
    let Ok(mut out) = mind::audio::Stream::new() else { mind::println!("[LISTEN] NO AUDIO OUTPUT"); return };
    let mut previous = 0i32;
    let mut frames = [0i16; 6 * 256];
    for block in mono.chunks(256) {
        let mut at = 0;
        for &sample in block {
            let (a, b) = (previous, sample as i32);
            for value in [(2 * a + b) / 3, (a + 2 * b) / 3, b] { frames[at] = value as i16; frames[at + 1] = value as i16; at += 2; }
            previous = b;
        }
        if let Err(error) = out.write(&frames[..at]) { mind::println!("[LISTEN] PLAYBACK FAILED: {:?}", error); return; }
    }
    match out.flush() { Ok(()) => mind::println!("[LISTEN] PLAYED BACK"), Err(error) => mind::println!("[LISTEN] PLAYBACK FAILED: {:?}", error) }
}
