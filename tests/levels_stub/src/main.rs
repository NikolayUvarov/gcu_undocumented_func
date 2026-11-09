#![no_std]
#![no_main]
//! Test-only console program (QEMU `audio` suite, 000-DRV-0057): reads the levels through the plain audio client every
//! program gets from the loader, and tries to set them, which the gateway refuses (only the shell's client is badged
//! BADGE_LEVELS). Never packaged into the normal OS; the shell's `volume` and `mic` are APP's (000-APP-0076, 0077).
use mind::abi::*;
use mind::sys::Error;

fn said(result: mind::sys::Result<mind::audio::Levels>) -> &'static str {
    match result { Ok(_) => "SET", Err(Error::Rights) => "RIGHTS", Err(Error::NotFound) => "NOT-FOUND", Err(Error::Invalid) => "INVALID", Err(_) => "ANOTHER ERROR" }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    match mind::audio::levels() {
        Ok(l) => mind::println!("LEVELS STUB: VOLUME {} MUTED {} OUTPUT {}/{}/{} SCALED {}; GAIN {} BOOST {} OF {} QDB CAPTURE {}/{}/{} SCALED {}",
                                l.volume, l.muted, l.output.steps, l.output.step_qdb, l.output.zero, l.output.scaled,
                                l.gain, l.boost, l.boost_qdb, l.capture.steps, l.capture.step_qdb, l.capture.zero, l.capture.scaled),
        Err(e) => mind::println!("LEVELS STUB: LEVELS FAILED {:?}", e),
    }
    mind::println!("LEVELS STUB: SET-VOLUME {}, SET-GAIN {}", said(mind::audio::set_volume(50, false)), said(mind::audio::set_gain(50, true)));
    mind::process::exit_with(0)
}
