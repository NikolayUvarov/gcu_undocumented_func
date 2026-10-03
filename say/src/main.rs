#![no_std]
#![no_main]
// Демонстрация синтеза речи: читает say.txt с диска (если есть) или произносит приветствие.
use mind::abi::BootInfo;
use mind::fs::File;
use mind::gfx::Screen;

const GREETING: &str = "Привет. Я разум корабля. Система готова к работе. Hello world.";

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let mut buffer = [0u8; 4096];
    let text = match File::open("say.txt") {
        Ok(mut file) => { let n = file.read(&mut buffer).unwrap_or(0); core::str::from_utf8(&buffer[..n]).unwrap_or(GREETING) }
        Err(_) => GREETING,
    };
    if let Some(screen) = Screen::new(info) {
        screen.clear(0x00101018);
        screen.text(24, 24, b"SAY - TEXT TO SPEECH (ESC: EXIT)", 2, 0x0080FFC0, None);
        screen.text(24, 64, text.as_bytes().iter().map(|&b| if b.is_ascii() { b } else { b'?' }).collect::<FixedText>().as_bytes(), 1, 0x00E0E0E0, None);
    }
    match mind::tts::say(text) {
        Ok(ms) => mind::println!("[SAY] SPOKE {} MS", ms),
        Err(error) => mind::println!("[SAY] ERROR {:?}", error),
    }
    mind::println!("[SAY] DONE");
    loop { mind::input::wait_or_exit(200); }
}

// Видимая часть текста для экрана: шрифт 8x8 знает только ASCII.
struct FixedText { bytes: [u8; 96], len: usize }
impl FixedText { fn as_bytes(&self) -> &[u8] { &self.bytes[..self.len] } }
impl FromIterator<u8> for FixedText {
    fn from_iter<I: IntoIterator<Item = u8>>(iter: I) -> Self {
        let mut text = Self { bytes: [0; 96], len: 0 };
        for byte in iter.into_iter().take(96) { text.bytes[text.len] = byte; text.len += 1; }
        text
    }
}
