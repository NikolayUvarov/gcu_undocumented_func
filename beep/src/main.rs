#![no_std]
#![no_main]
// Демонстрация аудиошлюза: аккорд тонами и PCM-свип, сгенерированный в своей памяти.
use mind::abi::BootInfo;
use mind::gfx::Screen;
use mind::mem::Pages;

mind::entry!(main);
fn main(info: &'static BootInfo) {
    if let Some(s) = Screen::new(info) { s.clear(0x00101010); s.text(24, 24, b"BEEP - AUDIO GATEWAY DEMO (ESC: EXIT)", 2, 0x00FFD080, None); }
    let info = match mind::audio::info() {
        Ok(info) => info,
        Err(error) => { mind::println!("[BEEP] GATEWAY ERROR: {:?}", error); return; }
    };
    mind::println!("[BEEP] DEVICE={} RATE={}", info.present, info.rate);
    if !info.present { return; }
    for hz in [523, 659, 784] { let _ = mind::audio::tone(hz, 150); }
    // Свип 300 -> 1200 Гц, 0.5 с, стерео: так клиент отдаёт произвольный PCM (например, от TTS).
    let frames = info.rate / 2;
    let Some(mut pcm) = Pages::new(frames * 4) else { mind::println!("[BEEP] NO MEMORY"); return };
    let samples = unsafe { core::slice::from_raw_parts_mut(pcm.as_mut_slice().as_mut_ptr() as *mut i16, frames * 2) };
    let mut phase = 0u32;
    for (i, frame) in samples.chunks_exact_mut(2).enumerate() {
        let hz = 300 + 900 * i / frames;
        phase = phase.wrapping_add((((hz as u64) << 32) / info.rate as u64) as u32); // фаза 2^32 на период
        let t = phase >> 16;
        let triangle = if t < 32768 { t as i32 - 16384 } else { 49152 - t as i32 };
        let value = (triangle / 2) as i16;
        frame[0] = value; frame[1] = value;
    }
    match mind::audio::play_all(samples) { Ok(()) => mind::println!("[BEEP] PCM QUEUED {} FRAMES", frames), Err(error) => mind::println!("[BEEP] PLAY ERROR: {:?}", error) }
    mind::println!("[BEEP] DONE");
    loop { mind::input::wait_or_exit(200); }
}
