#![no_std]
#![no_main]
// listen [seconds]: records from the microphone through audio_gw with a level meter, reports peak and RMS,
// then plays the recording back. Esc stops early.
use mind::abi::BootInfo;
use mind::gfx::Screen;
use mind::mem::Pages;

const RATE: usize = 48_000;
const BACKGROUND: u32 = 0x00101018;

fn seconds() -> usize {
    let text = mind::process::args_str().trim();
    text.parse::<usize>().unwrap_or(3).clamp(1, 10)
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

// Integer square root (no_std has no f64::sqrt).
fn isqrt(n: u64) -> u64 { let mut x = n; let mut y = (x + 1) / 2; while y < x { x = y; y = (x + n / x) / 2; } x }

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let seconds = seconds();
    let screen = Screen::new(info);
    if let Some(screen) = &screen {
        screen.clear(BACKGROUND);
        screen.text(24, 24, b"LISTEN - MICROPHONE (ESC: STOP)", 2, 0x0080FFC0, None);
    }
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
            let peak = samples(&mut buffer)[filled..filled + count].iter().map(|s| (*s as i32).abs()).max().unwrap_or(0);
            filled += count; idle = 0;
            meter(&screen, peak, b"RECORDING");
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
    let peak = recorded.iter().map(|s| (*s as i32).abs()).max().unwrap_or(0);
    let mean = if recorded.is_empty() { 0 } else { recorded.iter().map(|s| (*s as i64) * (*s as i64)).sum::<i64>() / recorded.len() as i64 };
    let rms = isqrt(mean as u64) as i32;
    mind::println!("[LISTEN] RECORDED {} FRAMES ({} MS), PEAK {}, RMS {}, OVERFLOWS {}", filled / 2, filled / 2 * 1000 / RATE, peak, rms, overflows);
    meter(&screen, peak, b"PLAYBACK");
    if !stopped && filled > 0 {
        match mind::audio::play_all(recorded) { Ok(()) => mind::println!("[LISTEN] PLAYED BACK"), Err(error) => mind::println!("[LISTEN] PLAYBACK FAILED: {:?}", error) }
    }
    mind::println!("[LISTEN] DONE");
    loop { mind::input::wait_or_exit(200); }
}
