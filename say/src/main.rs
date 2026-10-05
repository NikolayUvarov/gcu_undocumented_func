#![no_std]
#![no_main]
// say [-p <pitch Hz>] [-r <rate %>] [text]: speaks the text and exits; without text, say.txt or a greeting until Esc.
use mind::abi::BootInfo;
use mind::fs::File;
use mind::gfx::Screen;

mod wrap;

const GREETING: &str = "Привет. Я разум корабля. Система готова к работе. Hello world.";

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("say — speaks text through the synthesizer.\nUsage: say [-p <pitch Hz>] [-r <rate %>] [text]   (without text: say.txt or a greeting, until Esc)");
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
            Ok(mut file) => {
                // The first 4 KiB; a letter cut at the end is left out.
                let n = file.read(&mut buffer).unwrap_or(0);
                let bytes = &buffer[..n];
                let text = core::str::from_utf8(bytes).unwrap_or_else(|e| core::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""));
                if text.trim().is_empty() { GREETING } else { text }
            }
            Err(_) => GREETING,
        }
    };
    if let Some(screen) = Screen::new(info) {
        screen.clear(0x00101018);
        screen.text(24, 24, b"SAY - TEXT TO SPEECH (ESC: EXIT)", 2, 0x0080FFC0, None);
        show(&screen, text);
    }
    match mind::tts::say_with(text, pitch, rate) {
        Ok(ms) => mind::println!("[SAY] SPOKE {} MS", ms),
        Err(error) => mind::println!("[SAY] ERROR {:?}", error),
    }
    mind::println!("[SAY] DONE");
    // Text from the command line: done once spoken. The demo text keeps its screen until Esc.
    if words.is_empty() { loop { mind::input::wait_or_exit(200); } }
}

// The text in the 8x16 font, which has Cyrillic, cut into rows at spaces to the screen's width (issue u010); when it
// has more rows than the screen, the last one shown ends in "…".
fn show(screen: &Screen, text: &str) {
    let (x, y) = (24, 64);
    let columns = screen.width.saturating_sub(2 * x) / 8;
    let fit = screen.height.saturating_sub(y + 8) / 16;
    if columns == 0 || fit == 0 { return; }
    let mut rows = wrap::rows(text, columns).peekable();
    for n in 0..fit {
        let Some(row) = rows.next() else { return };
        let mut count = 0;
        for ch in row.chars() {
            screen.glyph16(x + count * 8, y + n * 16, if ch.is_control() { ' ' } else { ch }, 0x00E0E0E0, None);
            count += 1;
        }
        if n + 1 == fit && rows.peek().is_some() { screen.glyph16(x + count.min(columns - 1) * 8, y + n * 16, '…', 0x0080FFC0, None); }
    }
}
