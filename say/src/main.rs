#![no_std]
#![no_main]
// say [-p <pitch Hz>] [-r <rate %>] [text]: speaks the text; without text, say.txt from disk or a greeting.
use mind::abi::BootInfo;
use mind::fs::File;
use mind::gfx::Screen;

const GREETING: &str = "Привет. Я разум корабля. Система готова к работе. Hello world.";

mind::entry!(main);
fn main(info: &'static BootInfo) {
    // Options come first: -p <pitch Hz>, -r <rate %>; the rest is the text.
    let (mut pitch, mut rate) = (0u16, 0u16);
    let mut words = mind::process::args_str().trim();
    loop {
        let (option, value) = match words.split_once(' ') { Some((o, rest)) if o == "-p" || o == "-r" => (o, rest.trim_start()), _ => break };
        let (number, rest) = value.split_once(' ').unwrap_or((value, ""));
        let Ok(number) = number.parse::<u16>() else { break };
        if option == "-p" { pitch = number.clamp(60, 300); } else { rate = number.clamp(50, 200); }
        words = rest.trim_start();
    }
    let mut buffer = [0u8; 4096];
    let text = if !words.is_empty() { words } else {
        match File::open("say.txt") {
            Ok(mut file) => { let n = file.read(&mut buffer).unwrap_or(0); core::str::from_utf8(&buffer[..n]).unwrap_or(GREETING) }
            Err(_) => GREETING,
        }
    };
    if let Some(screen) = Screen::new(info) {
        screen.clear(0x00101018);
        screen.text(24, 24, b"SAY - TEXT TO SPEECH (ESC: EXIT)", 2, 0x0080FFC0, None);
        screen.text(24, 64, text.as_bytes().iter().map(|&b| if b.is_ascii() { b } else { b'?' }).collect::<FixedText>().as_bytes(), 1, 0x00E0E0E0, None);
    }
    match mind::tts::say_with(text, pitch, rate) {
        Ok(ms) => mind::println!("[SAY] SPOKE {} MS", ms),
        Err(error) => mind::println!("[SAY] ERROR {:?}", error),
    }
    mind::println!("[SAY] DONE");
    loop { mind::input::wait_or_exit(200); }
}

// Displayable part of the text for the screen: the 8x8 font knows only ASCII.
struct FixedText { bytes: [u8; 96], len: usize }
impl FixedText { fn as_bytes(&self) -> &[u8] { &self.bytes[..self.len] } }
impl FromIterator<u8> for FixedText {
    fn from_iter<I: IntoIterator<Item = u8>>(iter: I) -> Self {
        let mut text = Self { bytes: [0; 96], len: 0 };
        for byte in iter.into_iter().take(96) { text.bytes[text.len] = byte; text.len += 1; }
        text
    }
}
