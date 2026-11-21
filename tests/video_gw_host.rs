//! Host tests of video_gw/src/choices.rs (158-DRV-0082): which camera a number names, a stream for each camera with one
//! owner each, and the test patterns the boot disk asks for.
extern crate alloc;
#[path = "../video_gw/src/choices.rs"]
mod choices;
#[path = "../libmind/src/uvc.rs"]
#[allow(dead_code)]
mod uvc;
use choices::*;

const LISTED: usize = 4; // video.wit's list of cameras

#[test]
fn the_usb_cameras_come_first_then_the_patterns() {
    let usb = [0x0101, 0x0201];
    assert_eq!(key(0, &usb, 0, LISTED), Some(Key::Camera(0x0101)));
    assert_eq!(key(1, &usb, 0, LISTED), Some(Key::Camera(0x0201)));
    assert_eq!(key(2, &usb, 0, LISTED), None, "two cameras, no pattern");
    assert_eq!(key(2, &usb, 1, LISTED), Some(Key::Pattern(0)));
    assert_eq!(key(3, &usb, 2, LISTED), Some(Key::Pattern(1)));
    assert_eq!(key(4, &usb, 2, LISTED), None);
    assert_eq!(key(0, &[], 2, LISTED), Some(Key::Pattern(0)), "the patterns alone");
    assert_eq!(key(1, &[], 2, LISTED), Some(Key::Pattern(1)));
    assert_eq!(key(2, &[], 2, LISTED), None);
    assert_eq!(key(255, &[], 2, LISTED), None);
}

#[test]
fn the_patterns_keep_their_places_when_more_cameras_are_plugged_in() {
    let usb = [1, 2, 3, 4, 5];
    assert_eq!(real(usb.len(), 2, LISTED), 2, "four places, two for the patterns");
    assert_eq!(key(1, &usb, 2, LISTED), Some(Key::Camera(2)));
    assert_eq!(key(2, &usb, 2, LISTED), Some(Key::Pattern(0)));
    assert_eq!(key(3, &usb, 2, LISTED), Some(Key::Pattern(1)));
    assert_eq!(real(usb.len(), 0, LISTED), 4);
    assert_eq!(key(3, &usb, 0, LISTED), Some(Key::Camera(4)));
    assert_eq!(key(4, &usb, 0, LISTED), None, "the fifth is not listed");
}

#[test]
fn a_camera_held_is_busy_for_another_program_and_another_camera_opens_beside_it() {
    let (a, b, c) = (Key::Camera(0x0101), Key::Camera(0x0201), Key::Pattern(0));
    assert_eq!(admit(&[], 7, Some(a)), Ok(None), "nobody streams");
    let held = [(7, a)];
    assert_eq!(admit(&held, 8, Some(a)), Err(Refused::Busy), "one owner a camera");
    assert_eq!(admit(&held, 8, Some(b)), Ok(None), "the other camera opens beside the first");
    let held = [(7, a), (8, b)];
    assert_eq!(admit(&held, 9, Some(a)), Err(Refused::Busy));
    assert_eq!(admit(&held, 9, Some(b)), Err(Refused::Busy));
    assert_eq!(admit(&held, 9, Some(c)), Ok(None), "a third camera beside both");
    assert_eq!(admit(&held, 9, None), Err(Refused::NotFound));
}

#[test]
fn a_programs_own_earlier_stream_is_closed_first_when_it_opens_again_or_switches() {
    let (a, b) = (Key::Camera(0x0101), Key::Camera(0x0201));
    let held = [(7, a), (8, b)];
    assert_eq!(admit(&held, 7, Some(a)), Ok(Some(0)), "the same camera again, at another size");
    assert_eq!(admit(&held, 8, Some(Key::Pattern(1))), Ok(Some(1)), "a switch to another camera closes the first");
    assert_eq!(admit(&held, 7, Some(b)), Err(Refused::Busy), "a switch to a camera another program holds is refused");
    assert_eq!(admit(&held, 7, None), Err(Refused::NotFound), "a number past the list");
}

#[test]
fn video_synthetic_asks_for_one_pattern_or_two() {
    assert_eq!(patterns(None), 0, "no file");
    assert_eq!(patterns(Some(None)), 1, "an empty file");
    assert_eq!(patterns(Some(Some(b't'))), 1);
    assert_eq!(patterns(Some(Some(b'2'))), 2);
    assert_eq!(patterns(Some(Some(b'3'))), 1, "two at most");
}

#[test]
fn the_second_pattern_runs_half_a_minute_ahead() {
    assert_eq!(pattern_frame(0, 5, 10), 5);
    assert_eq!(pattern_frame(1, 5, 10), 305);
    assert_eq!(pattern_frame(1, 5, 30), 905);
    assert_eq!(pattern_frame(1, u32::MAX, 30), 899, "it wraps");
}

// 158-DRV-0083: which modes the gateway offers.
#[test]
fn a_yuy2_mode_is_offered_within_opens_sizes_and_rates_and_what_the_settings_carry() {
    let vga30 = 640 * 480 * 2 * 30;
    assert!(offered(true, 640, 480, 30, vga30, None), "a bulk camera's YUY2 mode");
    assert!(offered(true, 640, 480, 30, vga30, Some(24_480_000)), "carried by 3 × 1024 bytes a microframe");
    assert!(!offered(true, 640, 480, 30, vga30, Some(928_000)), "beyond a setting of 128 bytes");
    assert!(!offered(false, 640, 480, 30, vga30, None), "MJPEG is not decoded yet");
    assert!(!offered(true, 1920, 1080, 5, 1920 * 1080 * 2 * 5, None), "wider than open takes");
    assert!(offered(true, 1280, 1024, 5, 1280 * 1024 * 2 * 5, None), "the largest open takes");
    assert!(!offered(true, 320, 240, 0, 0, None), "less than one a second");
    assert!(!offered(true, 160, 120, 120, 160 * 120 * 2 * 120, None), "faster than open takes");
    assert!(opens(16, 16, 1) && !opens(15, 16, 1) && !opens(16, 1025, 1) && !opens(16, 16, 61));
}

#[test]
fn a_test_pattern_has_three_modes_the_pattern_draws() {
    assert_eq!(PATTERN_MODES.len(), 3, "two at least, for a program that steps through them");
    for &(w, h, rate) in &PATTERN_MODES {
        assert!(w % 32 == 0 && (64..=640).contains(&w) && (32..=480).contains(&h) && (1..=30).contains(&rate), "{w}x{h} at {rate}/s");
    }
}

// The Ryzen's two cameras (tests/data/uvc-0471-*.hex, logged by video_gw on 2026-11-21): the YUY2 modes open takes are
// offered, those wider than 1280 or taller than 1024 are not, and MJPEG is not yet.
#[test]
fn the_ryzens_cameras_offer_their_yuy2_modes_up_to_1280_by_1024() {
    for (name, offered_sizes) in [
        ("0471-a6ca", vec![(640, 480), (1280, 1024), (1280, 960), (1280, 720), (1024, 768), (800, 600), (640, 360), (352, 288), (320, 240), (160, 120)]),
        ("0471-a6cd", vec![(640, 480), (1280, 1024), (1280, 960), (1280, 720), (1024, 768), (960, 720), (800, 600), (640, 360), (432, 240), (352, 288), (320, 240)]),
    ] {
        let hex = std::fs::read_to_string(std::path::Path::new(file!()).parent().unwrap().join(format!("data/uvc-{name}.hex"))).unwrap();
        let bytes: Vec<u8> = (0..hex.trim().len()).step_by(2).map(|i| u8::from_str_radix(&hex.trim()[i..i + 2], 16).unwrap()).collect();
        let camera = uvc::parse(&bytes).unwrap();
        let capacity = camera.capacity(3);
        let offered_now: Vec<(u16, u16)> = camera.modes().iter()
            .filter(|m| offered(m.encoding == uvc::Encoding::Yuy2, m.width, m.height, m.rate(), m.bytes_per_second(), capacity)).map(|m| (m.width, m.height)).collect();
        assert_eq!(offered_now, offered_sizes, "{name}");
    }
}
