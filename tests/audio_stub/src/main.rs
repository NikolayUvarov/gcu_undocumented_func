#![no_std]
#![no_main]
//! Test-only console program (000-DRV-0093): capture and playback open together through `audio_gw` for a time, so a
//! USB audio device's streams run both ways at once (the STM-Speaker's duplex runs on the Ryzen). Playback is a tone of
//! HZ, or silence for 0, its stream kept open all along; the capture is read as it comes. Neither call waits on the
//! other: a full playback queue or an empty capture just passes the turn. At the end it says the capture's buffers,
//! frames and overflows and the frames played, and after 3 s the hardware tests' marker line. Never packaged into the
//! normal OS.
//! Usage: audiostub duplex SECONDS [HZ]   (HZ 0: silence; default 440)
use mind::abi::*;

const RATE: usize = 48_000;
const USAGE: &str = "usage: audiostub duplex SECONDS [HZ]";
const CHUNK_FRAMES: usize = 480; // 10 ms of stereo a turn
const AMPLITUDE: i32 = 8000;

fn fail(message: core::fmt::Arguments) -> ! { mind::println!("AUDIO STUB: {}", message); mind::process::exit_with(1) }

// sin(2π · phase / 2^32) × AMPLITUDE, by Bhaskara I's approximation of a half wave (within 0.2 %): no floating point.
fn sine(phase: u32) -> i16 {
    let half = (phase >> 31) != 0;
    let x = ((phase & 0x7FFF_FFFF) >> 15) as i64; // 0..65536 over the half wave
    let p = x * (65_536 - x); // ∝ x(π − x)
    let value = 4 * p * AMPLITUDE as i64 / (5 * 65_536 * 65_536 / 4 - p); // 16x(π−x) / (5π² − 4x(π−x))
    (if half { -value } else { value }) as i16
}

// What the run did.
#[derive(Default)]
struct Counts { buffers: u64, frames: u64, overflows: u64, played: u64, refused_play: u64, idle_turns: u64, capture: bool }

fn duplex(seconds: u32, hz: u32) -> Counts {
    let mut counts = Counts::default();
    counts.capture = match mind::audio::record_start() {
        Ok(()) => true,
        Err(e) => { mind::println!("AUDIO STUB: NO CAPTURE ({:?}): PLAYBACK ALONE", e); false }
    };
    let step = ((hz as u64) << 32).checked_div(RATE as u64).unwrap_or(0) as u32;
    let mut phase = 0u32;
    let mut out = [0i16; CHUNK_FRAMES * 2];
    let (mut pending, mut at) = (0usize, 0usize); // samples of `out` not yet accepted
    let mut captured = [0i16; 8192];
    let started = mind::time::monotonic_ns();
    let until = started + seconds as u64 * 1_000_000_000;
    while mind::time::monotonic_ns() < until {
        let mut busy = false;
        // Playback: the next 10 ms made once the last were taken; what the queue takes now, the rest next turn.
        if pending == 0 {
            for frame in out.chunks_exact_mut(2) {
                let v = if hz == 0 { 0 } else { sine(phase) };
                (frame[0], frame[1], phase) = (v, v, phase.wrapping_add(step));
            }
            (pending, at) = (out.len(), 0);
        }
        match mind::audio::play(&out[at..at + pending]) {
            Ok(0) => counts.refused_play += 1,
            Ok(n) => { (pending, at) = (pending - n, at + n); counts.played += n as u64 / 2; busy = true; }
            Err(e) => fail(format_args!("PLAYBACK FAILED AFTER {} FRAMES: {:?}", counts.played, e)),
        }
        // Capture: whatever came, in whole buffers.
        if counts.capture {
            match mind::audio::record_read(&mut captured) {
                Ok((0, overflow)) => counts.overflows += overflow as u64,
                Ok((n, overflow)) => { counts.buffers += 1; counts.frames += n as u64 / 2; counts.overflows += overflow as u64; busy = true; }
                Err(e) => fail(format_args!("CAPTURE FAILED AFTER {} FRAMES: {:?}", counts.frames, e)),
            }
        }
        if !busy { counts.idle_turns += 1; let _ = mind::time::sleep(2); }
    }
    if counts.capture { let _ = mind::audio::record_stop(); }
    counts
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut words = mind::process::args_str().split_whitespace();
    if words.next() != Some("duplex") { fail(format_args!("{}", USAGE)) }
    let Some(seconds) = words.next().and_then(|w| w.parse::<u32>().ok()).filter(|&s| s > 0) else { fail(format_args!("{}", USAGE)) };
    let hz = match words.next() { None => 440, Some(w) => w.parse::<u32>().ok().filter(|&h| h < RATE as u32 / 2).unwrap_or_else(|| fail(format_args!("{}", USAGE))) };
    match mind::audio::info() {
        Ok(info) if info.present => {}
        other => fail(format_args!("NO AUDIO DEVICE ({:?})", other.map(|i| i.present))),
    }
    let started_ms = mind::time::uptime_ms();
    mind::println!("AUDIO STUB: DUPLEX {} S, PLAYBACK {} AT 48 KHZ STEREO, STARTED AT {} MS OF UPTIME", seconds, if hz == 0 { "SILENCE" } else { "A TONE" }, started_ms);
    let c = duplex(seconds, hz);
    let ended_ms = mind::time::uptime_ms();
    mind::println!("AUDIO STUB: ENDED AT {} MS OF UPTIME ({} MS): CAPTURE {}: {} BUFFERS, {} FRAMES, {} OVERFLOWS; PLAYBACK {} FRAMES AT {} HZ, THE QUEUE FULL {} TIMES; {} IDLE TURNS",
        ended_ms, ended_ms.saturating_sub(started_ms), if c.capture { "OPEN" } else { "NONE" }, c.buffers, c.frames, c.overflows, c.played, hz, c.refused_play, c.idle_turns);
    let _ = mind::time::sleep(3000);
    mind::println!("*** AUDIO DUPLEX Complete. Data collected ***");
    mind::process::exit_with(0)
}

