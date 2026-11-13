//! Host tests of `beep`'s notes (beep/src/notes.rs, issue u005): one frequency sounds 500 ms, pairs are frequency and
//! duration, 0 Hz is a pause; what is refused; the samples rise and fall without a click; `left` or `right` first plays
//! on that channel alone (551-APP-0138).
#![allow(dead_code)]
#[path = "../beep/src/notes.rs"]
mod notes;

use notes::{channel, fill, parse, Channel, Error, Note, DEFAULT_MS, MAX_NOTES};

fn notes(args: &str) -> Result<Vec<Note>, Error<'_>> {
    let mut out = [Note::default(); MAX_NOTES];
    parse(args, &mut out).map(|count| out[..count].to_vec())
}

#[test]
fn notes_from_arguments() {
    assert_eq!(notes(""), Ok(vec![]));
    assert_eq!(notes("440"), Ok(vec![Note { hz: 440, ms: DEFAULT_MS }]));
    assert_eq!(notes("440 200 660 300"), Ok(vec![Note { hz: 440, ms: 200 }, Note { hz: 660, ms: 300 }]));
    assert_eq!(notes("440,200, 0 100 880"), Ok(vec![Note { hz: 440, ms: 200 }, Note { hz: 0, ms: 100 }, Note { hz: 880, ms: 500 }]), "commas; a pause; a last frequency alone");
    assert_eq!(notes("la"), Err(Error::NotANumber("la")));
    assert_eq!(notes("440 x"), Err(Error::NotANumber("x")));
    assert_eq!(notes("10"), Err(Error::Frequency(10)));
    assert_eq!(notes("30000 100"), Err(Error::Frequency(30000)));
    assert_eq!(notes("440 0"), Err(Error::Duration(0)));
    assert_eq!(notes("440 20000"), Err(Error::Duration(20000)));
    assert_eq!(notes(&"440 1 ".repeat(MAX_NOTES)).map(|n| n.len()), Ok(MAX_NOTES));
    assert_eq!(notes(&"440 1 ".repeat(MAX_NOTES + 1)), Err(Error::TooMany));
    let mut text = String::new();
    Error::Frequency(10).describe(&mut text).unwrap();
    assert_eq!(text, "10 HZ: A FREQUENCY IS 20-20000 HZ, OR 0 FOR A PAUSE");
}

#[test]
fn samples_of_a_note() {
    // 1000 Hz at 48 kHz: 48 frames a period; it fades in and out over 2 ms (96 frames).
    let total = 4800;
    let mut out = vec![0i16; total * 2];
    fill(&mut out, 1000, 48_000, 0, 0, total, Channel::Both);
    let left: Vec<i16> = out.iter().step_by(2).copied().collect();
    assert_eq!(left[0], 0);
    let peak = left[..].iter().map(|s| s.abs()).max().unwrap();
    assert!((8000..=8192).contains(&peak), "{}", peak);
    assert!((0..96).all(|i| left[i].abs() as i32 <= 8192 * (i as i32 + 1) / 96 + 1), "fades in");
    assert!((0..96).all(|i| left[total - 1 - i].abs() as i32 <= 8192 * (i as i32 + 1) / 96 + 1), "fades out");
    assert_eq!(left.windows(2).filter(|w| w[0] < 0 && w[1] >= 0).count(), 99, "100 periods: a rising zero crossing each, but the first");
    // Generated in two parts, the same samples.
    let mut parts = vec![0i16; total * 2];
    let phase = fill(&mut parts[..2000 * 2], 1000, 48_000, 0, 0, total, Channel::Both);
    fill(&mut parts[2000 * 2..], 1000, 48_000, phase, 2000, total, Channel::Both);
    assert_eq!(parts, out);
    // A pause is silence.
    fill(&mut out, 0, 48_000, 0, 0, total, Channel::Both);
    assert!(out.iter().all(|&s| s == 0));
}

#[test]
fn a_channel_word_first() {
    assert_eq!(channel("left 440 200"), (Channel::Left, " 440 200"));
    assert_eq!(channel("RIGHT 440"), (Channel::Right, " 440"), "either case");
    assert_eq!(channel("Left"), (Channel::Left, ""), "the word alone");
    assert_eq!(channel("right,440,200"), (Channel::Right, ",440,200"));
    assert_eq!(channel("440 200"), (Channel::Both, "440 200"));
    assert_eq!(channel(""), (Channel::Both, ""));
    assert_eq!(channel("leftover 440"), (Channel::Both, "leftover 440"), "a longer word is not the channel's");
    // After the word, the notes as without it; a channel word anywhere else is not a number.
    assert_eq!(notes(channel("left 440 200 0 100").1), Ok(vec![Note { hz: 440, ms: 200 }, Note { hz: 0, ms: 100 }]));
    assert_eq!(notes(channel("440 left").1), Err(Error::NotANumber("left")));
    assert_eq!(notes(channel("left right").1), Err(Error::NotANumber("right")));
    assert_eq!(notes(channel("leftover").1), Err(Error::NotANumber("leftover")));
    assert_eq!((Channel::Both.words(), Channel::Left.words(), Channel::Right.words()), ("", " ON THE LEFT", " ON THE RIGHT"));
}

#[test]
fn one_channel_and_the_other_silent() {
    let total = 4800;
    let mut both = vec![0i16; total * 2];
    fill(&mut both, 1000, 48_000, 0, 0, total, Channel::Both);
    for (channel, sounds) in [(Channel::Left, 0), (Channel::Right, 1)] {
        let mut out = vec![0i16; total * 2];
        let phase = fill(&mut out, 1000, 48_000, 0, 0, total, channel);
        assert_eq!(phase, fill(&mut vec![0i16; total * 2], 1000, 48_000, 0, 0, total, Channel::Both), "the same phase");
        assert!(out.iter().skip(1 - sounds).step_by(2).all(|&s| s == 0), "{:?}: the other channel silent", channel);
        let named: Vec<i16> = out.iter().skip(sounds).step_by(2).copied().collect();
        assert_eq!(named, both.iter().step_by(2).copied().collect::<Vec<_>>(), "{:?}: the tone as on both", channel);
    }
}
