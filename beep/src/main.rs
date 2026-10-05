#![no_std]
#![no_main]
// beep (issue u005): tones through the audio gateway, and nothing on the screen. `beep 440` sounds 440 Hz for 500 ms;
// `beep 440 200 660 300` a series of notes, frequency and duration each (0 Hz: a pause). Without arguments, the
// gateway demo: a chord of tones and a PCM sweep. It ends when the sound has played.
mod notes;

use core::fmt::Write;
use mind::abi::BootInfo;
use mind::mem::Pages;
use mind::util::FixedBuf;
use notes::{Note, MAX_NOTES};

mind::request!(REQUEST_CONSOLE);

// Frames generated at a time (stereo i16), then queued with play_all, which waits for room in the gateway's ring.
const CHUNK_FRAMES: usize = 4800;

fn demo(rate: usize) {
    for hz in [523, 659, 784] { let _ = mind::audio::tone(hz, 150); }
    // Sweep 300 -> 1200 Hz, 0.5 s, stereo: this is how a client submits arbitrary PCM (e.g. from TTS).
    let frames = rate / 2;
    let Some(mut pcm) = Pages::new(frames * 4) else { mind::println!("[BEEP] NO MEMORY"); return };
    let samples = unsafe { core::slice::from_raw_parts_mut(pcm.as_mut_slice().as_mut_ptr() as *mut i16, frames * 2) };
    let mut phase = 0u32;
    for (i, frame) in samples.chunks_exact_mut(2).enumerate() {
        let hz = 300 + 900 * i / frames;
        phase = phase.wrapping_add((((hz as u64) << 32) / rate as u64) as u32); // phase 2^32 per period
        let t = phase >> 16;
        let triangle = if t < 32768 { t as i32 - 16384 } else { 49152 - t as i32 };
        let value = (triangle / 2) as i16;
        frame[0] = value; frame[1] = value;
    }
    match mind::audio::play_all(samples) { Ok(()) => mind::println!("[BEEP] PCM QUEUED {} FRAMES", frames), Err(error) => mind::println!("[BEEP] PLAY ERROR: {:?}", error) }
}

fn play(notes: &[Note], rate: usize) -> bool {
    let Some(mut pcm) = Pages::new(CHUNK_FRAMES * 4) else { mind::println!("[BEEP] NO MEMORY"); return false };
    let chunk = unsafe { core::slice::from_raw_parts_mut(pcm.as_mut_slice().as_mut_ptr() as *mut i16, CHUNK_FRAMES * 2) };
    for note in notes {
        let total = rate * note.ms as usize / 1000;
        let (mut at, mut phase) = (0, 0u32);
        while at < total {
            let frames = (total - at).min(CHUNK_FRAMES);
            phase = notes::fill(&mut chunk[..frames * 2], note.hz, rate as u32, phase, at, total);
            if let Err(error) = mind::audio::play_all(&chunk[..frames * 2]) { mind::println!("[BEEP] PLAY ERROR: {:?}", error); return false; }
            at += frames;
        }
    }
    true
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("beep — tones through the audio gateway (no screen).\nUsage: beep [hz] | beep hz ms [hz ms ...]\nOne frequency sounds 500 ms; pairs of frequency and duration (ms) play one after the other, 0 Hz is a pause\n(20-20000 Hz, up to 10000 ms each, 64 notes). Without arguments: a chord and a sweep.");
    let args = mind::process::args_str().trim();
    let mut notes = [Note::default(); MAX_NOTES];
    let count = match notes::parse(args, &mut notes) {
        Ok(count) => count,
        Err(error) => { let mut text = FixedBuf::<96>::new(); let _ = error.describe(&mut text); mind::println!("BEEP: {}", text.as_str()); return; }
    };
    let info = match mind::audio::info() {
        Ok(info) => info,
        Err(error) => { mind::println!("[BEEP] GATEWAY ERROR: {:?}", error); return; }
    };
    mind::println!("[BEEP] DEVICE={} RATE={}", info.present, info.rate);
    if !info.present { return; }
    let start = mind::time::uptime_ms();
    let total: usize = if count == 0 { demo(info.rate); 950 } else {
        if !play(&notes[..count], info.rate) { return; }
        let mut line = FixedBuf::<64>::new();
        let total = notes[..count].iter().map(|n| n.ms as usize).sum();
        let _ = write!(line, "[BEEP] PLAYED {} NOTES, {} MS", count, total);
        mind::println!("{}", line.as_str());
        total
    };
    mind::println!("[BEEP] DONE");
    // The gateway plays what was queued; the program ends with the sound.
    let elapsed = mind::time::uptime_ms() - start;
    if total > elapsed { mind::time::sleep(total - elapsed); }
}
