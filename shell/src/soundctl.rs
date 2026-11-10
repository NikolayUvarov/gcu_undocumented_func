//! The system calls around `sound` (000-APP-0076, 000-APP-0077): `volume` and `mic` through the shell's client of
//! `audio_gw`, which init badges `BADGE_LEVELS` (000-KRN-0118), a change kept in `data/sound.conf`, and the file's
//! levels set again at the shell's start.

use crate::clockzone::Load;
use crate::sound::{self, Amp, Kept, Mic, Volume};
use alloc::string::String;
use mind::audio::{Levels, Range};
use mind::idl::wire;

// How long a call to audio_gw may take from the shell, in ms.
const CALL_MS: u32 = 2000;

fn amp(range: Range) -> Amp { Amp { steps: range.steps, step_qdb: range.step_qdb, zero: range.zero } }

fn call(f: impl FnOnce() -> mind::sys::Result<Levels>) -> Result<Levels, String> {
    match wire::with_timeout(CALL_MS, f) {
        Ok(levels) => Ok(levels),
        Err(mind::sys::Error::NotFound) => Err(String::from("NO AUDIO DEVICE")),
        Err(mind::sys::Error::Rights) => Err(String::from("THE SHELL'S AUDIO CLIENT MAY NOT SET THE LEVELS (NO BADGE_LEVELS FROM INIT)")),
        Err(mind::sys::Error::Other(code)) if code == mind::abi::ERR_TIMEOUT => Err(String::from("THE AUDIO GATEWAY DID NOT ANSWER")),
        Err(error) => Err(alloc::format!("THE AUDIO GATEWAY REFUSED ({:?})", error)),
    }
}

/// `volume [0-100 | mute | unmute]` or `mic [0-100 | boost on | boost off]`: the line to print, or an error's.
pub fn command(mic: bool, args: &str) -> Result<String, String> {
    let now = call(mind::audio::levels)?;
    let (changed, levels) = if mic {
        let asked = sound::mic(args).map_err(String::from)?;
        match asked {
            Mic::Show => (false, now),
            Mic::Set(p) => (true, call(|| mind::audio::set_gain(p, now.boost))?),
            Mic::Boost(_) if now.boost_qdb == 0 => return Err(String::from("THE MICROPHONE HAS NO BOOST")),
            Mic::Boost(on) => (true, call(|| mind::audio::set_gain(now.gain, on))?),
        }
    } else {
        match sound::volume(args).map_err(String::from)? {
            Volume::Show => (false, now),
            Volume::Set(p) => (true, call(|| mind::audio::set_volume(p, now.muted))?),
            Volume::Mute(m) => (true, call(|| mind::audio::set_volume(now.volume, m))?),
        }
    };
    let line = if mic { sound::mic_line(levels.gain, levels.boost, levels.boost_qdb, amp(levels.capture), levels.capture.scaled) }
               else { sound::volume_line(levels.volume, levels.muted, amp(levels.output), levels.output.scaled) };
    if !changed { return Ok(line); }
    mind::println!("[SHELL] {}", line);
    Ok(match keep(&levels) {
        Ok(()) => alloc::format!("{}; KEPT IN {}", line, sound::FILE),
        Err(why) => alloc::format!("{}; UNTIL THE REBOOT: {} NOT WRITTEN ({})", line, sound::FILE, why),
    })
}

// Both levels as they are now, in the file.
fn keep(levels: &Levels) -> Result<(), &'static str> {
    let kept = Kept { volume: Some(levels.volume), muted: Some(levels.muted), gain: Some(levels.gain), boost: (levels.boost_qdb != 0).then_some(levels.boost) };
    let _ = mind::fs::Dir::create("data"); // a boot volume written without it
    let mut file = mind::fs::File::create(sound::FILE).map_err(crate::files::text)?;
    file.write(sound::render(&kept).as_bytes()).map_err(crate::files::text)?;
    file.flush().map_err(crate::files::text)
}

/// At start: the file's levels set. `Later` while the volume does not answer, unless `last`.
pub fn load(last: bool) -> Load {
    let text = match wire::with_timeout(200, read) {
        Ok(text) => text,
        Err("NOT FOUND") => return Load::Done,
        Err("VFS NOT AVAILABLE") if !last => return Load::Later,
        Err(why) => { mind::println!("[SHELL] THE LEVELS NOT READ FROM {}: {}", sound::FILE, why); return Load::Done; }
    };
    let kept = sound::parse(&text);
    let set = call(mind::audio::levels).and_then(|now| {
        let output = if kept.volume.is_some() || kept.muted.is_some() { call(|| mind::audio::set_volume(kept.volume.unwrap_or(now.volume), kept.muted.unwrap_or(now.muted)))? } else { now };
        if kept.gain.is_some() || kept.boost.is_some() { call(|| mind::audio::set_gain(kept.gain.unwrap_or(output.gain), kept.boost.unwrap_or(output.boost) && output.boost_qdb != 0)) } else { Ok(output) }
    });
    match set {
        Ok(l) => mind::println!("[SHELL] THE LEVELS FROM {}: {}; {}", sound::FILE, sound::volume_line(l.volume, l.muted, amp(l.output), l.output.scaled),
            sound::mic_line(l.gain, l.boost, l.boost_qdb, amp(l.capture), l.capture.scaled)),
        Err(why) => mind::println!("[SHELL] THE LEVELS FROM {} NOT SET: {}", sound::FILE, why),
    }
    Load::Done
}

fn read() -> Result<String, &'static str> {
    let mut file = mind::fs::File::open(sound::FILE).map_err(crate::files::text)?;
    let mut bytes = alloc::vec![0u8; file.size().min(1024)];
    let mut at = 0;
    while at < bytes.len() { match file.read(&mut bytes[at..]) { Ok(0) => break, Ok(n) => at += n, Err(e) => return Err(crate::files::text(e)) } }
    bytes.truncate(at);
    String::from_utf8(bytes).map_err(|_| "NOT UTF-8")
}
