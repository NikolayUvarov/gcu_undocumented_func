//! Host tests of `camera`'s choice of camera (camera/src/pick.rs, 158-APP-0147): `-c <n>` within what the gateway lists,
//! the next one round from the last, and the window's title. The camera's modes (camera/src/modes.rs, 158-APP-0152):
//! `-m <n>` checked, `m` and Shift+`m` round the offered ones, a mode in words.
#![allow(dead_code)]
extern crate alloc;
#[path = "../camera/src/pick.rs"]
mod pick;
#[path = "../camera/src/modes.rs"]
mod modes;

#[test]
fn a_camera_chosen_and_the_next_one() {
    assert_eq!(pick::chosen(0, 2), Ok(0));
    assert_eq!(pick::chosen(1, 2), Ok(1));
    assert_eq!(pick::chosen(2, 2), Err("camera 2: the video gateway lists 2 cameras, numbers 0 to 1".into()));
    assert_eq!(pick::chosen(1, 1), Err("camera 1: the video gateway lists one camera, number 0".into()));
    assert_eq!(pick::chosen(0, 0), Err("no camera (the video gateway lists none)".into()));
    assert_eq!((pick::next(0, 3), pick::next(1, 3), pick::next(2, 3)), (1, 2, 0), "round from the last to the first");
    assert_eq!((pick::next(0, 1), pick::next(0, 0)), (0, 0));
    assert_eq!(pick::title(1, 3, "USB2.0 HD UVC WebCam"), "camera 2/3: USB2.0 HD UVC WebCam");
}

#[test]
fn the_cameras_modes_stepped_and_named() {
    // The Ryzen's camera 0 lists MJPEG modes not offered yet beside YUY2 ones (DRV, 2026-11-21): steps skip them.
    let offered = [true, false, true, false, true];
    assert_eq!((modes::step(&offered, Some(0), true), modes::step(&offered, Some(2), true), modes::step(&offered, Some(4), true)), (Some(2), Some(4), Some(0)));
    assert_eq!((modes::step(&offered, Some(0), false), modes::step(&offered, Some(4), false)), (Some(4), Some(2)), "Shift+m: the one before, round");
    assert_eq!((modes::step(&offered, None, true), modes::step(&offered, None, false)), (Some(0), Some(4)), "a size none of the modes has: from either end");
    assert_eq!(modes::step(&offered, Some(1), true), Some(2), "from a mode no longer offered");
    assert_eq!((modes::step(&[true], Some(0), true), modes::step(&[false, true], Some(1), false), modes::step(&[], None, true)), (None, None, None), "one mode only, or none");
    // The test pattern's three modes (DRV's 158-DRV-0083): 640x480 at 30, 320x240 at 10, 160x120 at 5.
    assert_eq!(modes::chosen(1, &[true, true, true], 0), Ok(1));
    assert_eq!(modes::chosen(3, &[true, true, true], 0), Err("mode 3: the gateway lists 3 modes for camera 0, numbers 0 to 2".into()));
    assert_eq!(modes::chosen(1, &offered, 0), Err("mode 1 of camera 0 is listed but not offered now".into()));
    assert_eq!(modes::chosen(0, &[], 1), Err("camera 1: the gateway lists no modes".into()));
    assert_eq!((modes::describe(0, 640, 480, 30), modes::describe(3, 160, 120, 5), modes::encoding(9)), ("640x480 at 30/s YUY2".into(), "160x120 at 5/s SYNTHETIC".into(), "UNKNOWN"));
}
