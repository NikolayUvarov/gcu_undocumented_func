//! The shell's `screen` (213-APP-0158, for 213-KRN-0273): the second screen as the compositor shows it (`display.wit`
//! 1.4 `mirror`), and its fit (`mirror-fit`), through the shell's own client in SLOT_DISPLAY, which has no badge; kept
//! in data/displays.txt and set again at the shell's start.
use crate::clockzone::Load;
use crate::screens::{self, Asked, Fit};
use crate::Shell;
use core::fmt::Write;
use mind::abi::SLOT_DISPLAY;
use mind::idl::display as idl;
use mind::ipc::Endpoint;

const DISPLAY: Endpoint = Endpoint(SLOT_DISPLAY);

fn ours(fit: Fit) -> idl::Fit { match fit { Fit::Keep => idl::Fit::Keep, Fit::Stretch => idl::Fit::Stretch } }
fn theirs(fit: idl::Fit) -> Fit { match fit { idl::Fit::Keep => Fit::Keep, idl::Fit::Stretch => Fit::Stretch } }

// The second screen in words, or why the compositor gives none.
fn second() -> alloc::string::String {
    match idl::mirror(DISPLAY) {
        Ok(Ok(m)) => screens::second(Some((m.width, m.height, theirs(m.fit), m.picture_width, m.picture_height))),
        Ok(Err(idl::Error::NoScreen)) => screens::second(None),
        Ok(Err(error)) => alloc::format!("SCREEN: THE COMPOSITOR REFUSED MIRROR: {:?}", error),
        Err(mind::sys::Error::Invalid) => alloc::string::String::from("SCREEN: THE COMPOSITOR DOES NOT KNOW MIRROR: IT IS OLDER THAN DISPLAY.WIT 1.4"),
        Err(_) => alloc::string::String::from("SCREEN: THE COMPOSITOR DOES NOT ANSWER"),
    }
}

/// `screen [second [keep|stretch]]`.
pub fn command(shell: &mut Shell, args: &[u8]) {
    let asked = match screens::parse(core::str::from_utf8(args).unwrap_or("")) { Ok(asked) => asked, Err(usage) => { let _ = writeln!(shell.term, "{}", usage); return } };
    let fit = match asked { Asked::Show => { let _ = writeln!(shell.term, "{}", second()); return } Asked::Fit(fit) => fit };
    match idl::mirror_fit(DISPLAY, ours(fit)) {
        Ok(Ok(())) => {
            let kept = mind::fs::File::create(screens::FILE).and_then(|mut f| f.write_at(0, screens::text(fit).as_bytes())).map(drop).map_err(crate::files::text);
            let line = screens::chosen(fit, kept);
            let _ = writeln!(shell.term, "{}", line);
            mind::println!("[SHELL] {}", line);
            let _ = writeln!(shell.term, "{}", second());
        }
        Ok(Err(error)) => { let _ = writeln!(shell.term, "SCREEN: THE COMPOSITOR REFUSED THE FIT: {:?}", error); }
        Err(mind::sys::Error::Invalid) => { let _ = writeln!(shell.term, "SCREEN: THE COMPOSITOR DOES NOT KNOW MIRROR-FIT: IT IS OLDER THAN DISPLAY.WIT 1.4"); }
        Err(_) => { let _ = writeln!(shell.term, "SCREEN: THE COMPOSITOR DOES NOT ANSWER"); }
    }
}

/// At start: the fit data/displays.txt keeps given to the compositor. `Later` while the volume does not answer, unless
/// `last`.
pub fn load(last: bool) -> Load {
    let text = match mind::idl::wire::with_timeout(200, read) {
        Ok(text) => text,
        Err("NOT FOUND") => return Load::Done,
        Err("VFS NOT AVAILABLE") if !last => return Load::Later,
        Err(why) => { mind::println!("[SHELL] THE SCREENS NOT READ FROM {}: {}", screens::FILE, why); return Load::Done; }
    };
    let Some(fit) = screens::read(&text) else { return Load::Done };
    match idl::mirror_fit(DISPLAY, ours(fit)) {
        Ok(Ok(())) => mind::println!("[SHELL] THE SECOND SCREEN'S FIT FROM {}: {}", screens::FILE, fit.name().to_ascii_uppercase()),
        other => mind::println!("[SHELL] THE SECOND SCREEN'S FIT FROM {} NOT SET: {:?}", screens::FILE, other),
    }
    Load::Done
}

fn read() -> Result<alloc::string::String, &'static str> {
    let mut file = mind::fs::File::open(screens::FILE).map_err(crate::files::text)?;
    let mut bytes = alloc::vec![0u8; file.size().min(1024)];
    let mut at = 0;
    while at < bytes.len() { match file.read(&mut bytes[at..]) { Ok(0) => break, Ok(n) => at += n, Err(e) => return Err(crate::files::text(e)) } }
    bytes.truncate(at);
    alloc::string::String::from_utf8(bytes).map_err(|_| "NOT UTF-8")
}
