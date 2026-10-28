//! The loader's reading of a monitor's EDID and its choice of a graphics mode (211-PRT-0011): the preferred timing of a
//! base block, the refusals, and the mode picked with it or without it.
#[path = "../bootloader/src/edid.rs"]
mod edid;
use edid::{choose, preferred, Choice, Refused};

// A base EDID block whose first detailed timing is `width` x `height` (pixel clock 148.5 MHz), its checksum set.
fn block(width: usize, height: usize) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
    b[18] = 1; b[19] = 4; // EDID 1.4
    b[54] = 0x02; b[55] = 0x3A; // 14850 × 10 kHz
    b[56] = (width & 0xFF) as u8; b[58] = ((width >> 8) << 4) as u8;
    b[59] = (height & 0xFF) as u8; b[61] = ((height >> 8) << 4) as u8;
    let sum = b.iter().fold(0u8, |s, &x| s.wrapping_add(x));
    b[127] = 0u8.wrapping_sub(sum);
    b
}

#[test]
fn the_first_detailed_timing_is_the_preferred_mode() {
    assert_eq!(preferred(&block(1920, 1080)), Ok((1920, 1080)));
    assert_eq!(preferred(&block(2560, 1440)), Ok((2560, 1440)));
    assert_eq!(preferred(&block(1366, 768)), Ok((1366, 768)));
    // An extension block after the base one changes nothing.
    let mut two = block(1280, 1024);
    two.extend_from_slice(&[0u8; 128]);
    assert_eq!(preferred(&two), Ok((1280, 1024)));
}

#[test]
fn a_broken_or_timeless_edid_gives_none() {
    assert_eq!(preferred(&block(1920, 1080)[..100]), Err(Refused::Short));
    let mut bad = block(1920, 1080);
    bad[0] = 0x01;
    assert_eq!(preferred(&bad), Err(Refused::Header));
    let mut sum = block(1920, 1080);
    sum[100] ^= 0x55;
    assert_eq!(preferred(&sum), Err(Refused::Checksum));
    // A display descriptor (pixel clock 0) where the first timing should be.
    let mut name = block(1920, 1080);
    name[54] = 0; name[55] = 0;
    let s = name[..127].iter().fold(0u8, |s, &x| s.wrapping_add(x));
    name[127] = 0u8.wrapping_sub(s);
    assert_eq!(preferred(&name), Err(Refused::NoTiming));
}

#[test]
fn the_preferred_mode_wins_when_listed_with_a_framebuffer() {
    let modes = [(800, 600, true), (1024, 768, true), (1920, 1080, true), (1920, 1200, true), (2560, 1600, true)];
    // The DEXP's case (2026-10-27): the firmware left 800x600; the monitor prefers 1920x1080.
    assert_eq!(choose(&modes, true, Some((1920, 1080))), Choice::Preferred(2));
    // Not listed, or listed without a linear framebuffer: the firmware's mode stays.
    assert_eq!(choose(&modes, true, Some((1680, 1050))), Choice::Keep);
    assert_eq!(choose(&[(1920, 1080, false), (1024, 768, true)], true, Some((1920, 1080))), Choice::Keep);
    // Larger than the services are sized for (a 4K monitor): kept.
    assert_eq!(choose(&[(3840, 2160, true), (1920, 1080, true)], true, Some((3840, 2160))), Choice::Keep);
    // The Retina screen, 2880x1800, is within it.
    assert_eq!(choose(&[(1440, 900, true), (2880, 1800, true)], true, Some((2880, 1800))), Choice::Preferred(1));
}

#[test]
fn without_an_edid_the_firmwares_mode_stays_unless_it_has_no_framebuffer() {
    let modes = [(800, 600, true), (1920, 1200, true), (2560, 1600, true), (1024, 768, false)];
    assert_eq!(choose(&modes, true, None), Choice::Keep);
    // A BltOnly firmware mode: the largest drawable one up to 1920x1200, as before.
    assert_eq!(choose(&modes, false, None), Choice::Largest(1));
    assert_eq!(choose(&[(1024, 768, false)], false, None), Choice::Nothing);
}
