//! Host tests of `camera`'s choice of camera (camera/src/pick.rs, 158-APP-0147): `-c <n>` within what the gateway lists,
//! the next one round from the last, and the window's title.
#![allow(dead_code)]
extern crate alloc;
#[path = "../camera/src/pick.rs"]
mod pick;

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
