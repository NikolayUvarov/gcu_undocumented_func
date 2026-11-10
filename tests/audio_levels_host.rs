// Host tests of audio_gw's levels (audio.wit 1.2, 000-DRV-0057): per cent and settings, the registers, HD Audio's
// amplifier capabilities and the scaled samples' factors.
#[path = "../audio_gw/src/levels.rs"]
mod levels;

use levels::*;

#[test]
fn per_cent_and_settings_both_ways() {
    for amp in [SOFT_OUTPUT, SOFT_CAPTURE, Amp { steps: 128, step_qdb: 3, zero: 74 }, Amp { steps: 2, step_qdb: 40, zero: 0 }] {
        assert_eq!(setting(0, amp), 0);
        assert_eq!(setting(100, amp), amp.steps - 1);
        assert_eq!(setting(255, amp), amp.steps - 1, "above 100 % is the top");
        // With fewer settings than per cents each setting has its per cent; with more, each per cent its setting.
        if amp.steps <= 101 { for s in 0..amp.steps { assert_eq!(setting(percent(s, amp), amp), s, "{amp:?} setting {s}"); } }
        else { for p in 0..=100 { assert_eq!(percent(setting(p, amp), amp), p, "{amp:?} {p} %"); } }
    }
    assert_eq!(setting(50, SOFT_OUTPUT), 16);
    assert_eq!(percent(31, SOFT_OUTPUT), 100);
}

#[test]
fn a_settings_level_in_quarter_db() {
    assert_eq!(qdb(31, SOFT_OUTPUT), 0);
    assert_eq!(qdb(0, SOFT_OUTPUT), -186, "-46.5 dB");
    assert_eq!(qdb(15, SOFT_CAPTURE), 90, "+22.5 dB");
    // Half the output's per cent is about 24 dB down in 1.5 dB steps.
    assert_eq!(qdb(setting(50, SOFT_OUTPUT), SOFT_OUTPUT), -90);
}

#[test]
fn hd_audio_amplifier_capabilities() {
    // QEMU's hda-duplex: offset 0x4A, 0x4A steps of 0.75 dB? Whatever it says, read as HD Audio defines it.
    let caps = 1 << 31 | 0x02 << 16 | 0x4A << 8 | 0x4A;
    let (amp, mute) = hda_amp(caps).unwrap();
    assert_eq!(amp, Amp { steps: 0x4B, step_qdb: 3, zero: 0x4A });
    assert!(mute);
    assert_eq!(hda_amp(0x0000_0040), None, "no step: a fixed amplifier");
    let (amp, mute) = hda_amp(0x0003_0303).unwrap();
    assert_eq!((amp, mute), (Amp { steps: 4, step_qdb: 4, zero: 3 }, false));
    // An offset beyond the steps is taken as the top.
    assert_eq!(hda_amp(0x0001_027F).unwrap().0.zero, 2);
    assert_eq!(hda_gain(true, 0, 0x4A, false), 0xB04A);
    assert_eq!(hda_gain(true, 0, 0x10, true), 0xB090);
    assert_eq!(hda_gain(false, 2, 5, false), 0x7205);
}

#[test]
fn ac97_registers() {
    assert_eq!(ac97_master(31, SOFT_OUTPUT, false), 0x0000, "0 dB");
    assert_eq!(ac97_master(0, SOFT_OUTPUT, false), 0x1F1F, "-46.5 dB");
    assert_eq!(ac97_master(31, SOFT_OUTPUT, true), 0x8000);
    let six = Amp { steps: 64, step_qdb: 6, zero: 63 };
    assert_eq!(ac97_master(0, six, false), 0x3F3F);
    assert_eq!(ac97_record(0), 0x0000);
    assert_eq!(ac97_record(15), 0x0F0F);
    assert_eq!(ac97_microphone(false), 0x0008);
    assert_eq!(ac97_microphone(true), 0x0048);
    assert_eq!(AC97_BOOST_QDB, 80, "+20 dB");
}

#[test]
fn scaled_samples() {
    assert_eq!(factor_q15(0, false), 32768);
    assert_eq!(factor_q15(0, true), 0);
    // -6 dB is four steps of 1.5 dB: about half; -46.5 dB about 1/211.
    let half = factor_q15(-24, false);
    assert!((16_380..=16_460).contains(&half), "{half}");
    let floor = factor_q15(-186, false);
    assert!((150..=160).contains(&floor), "{floor}");
    let up = factor_q15(90, false); // +22.5 dB, about 13.3 times
    assert!((435_000..=437_500).contains(&up), "{up}");
    assert_eq!(scale(10_000, half) / 10, 501, "-6 dB is 0.501");
    assert_eq!(scale(30_000, up), i16::MAX, "clipped");
    assert_eq!(scale(-30_000, up), i16::MIN);
    let mut pcm = [0x10, 0x27, 0xF0, 0xD8]; // 10000, -10000
    scale_bytes(&mut pcm, half);
    let (a, b) = (i16::from_le_bytes([pcm[0], pcm[1]]), i16::from_le_bytes([pcm[2], pcm[3]]));
    assert!((5000..=5020).contains(&a) && (-5020..=-5000).contains(&b), "{a} {b}");
}
