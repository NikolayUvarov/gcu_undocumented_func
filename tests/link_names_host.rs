//! Where a name on the link goes (800-LNK-0027): data/link/, or data/firmware/ as `firmware/` in developer mode.
#[path = "../link/src/names.rs"] mod names;
use names::*;

#[test]
fn names_under_firmware_go_to_data_firmware() {
    let mut out = [0u8; PATH_MAX];
    assert_eq!(path("apps/test/x.elf", &mut out), "data/link/apps/test/x.elf");
    assert_eq!(path("firmware", &mut out), "data/firmware");
    assert_eq!(path("firmware/rtw88/rtw8821c_fw.bin", &mut out), "data/firmware/rtw88/rtw8821c_fw.bin");
    assert_eq!(path("firmwares/x", &mut out), "data/link/firmwares/x", "only the name firmware itself");
    assert_eq!(path("firmware.bin", &mut out), "data/link/firmware.bin");
    assert_eq!(path("a/firmware/x", &mut out), "data/link/a/firmware/x");
}

#[test]
fn a_put_is_finished_in_its_own_place() {
    assert_eq!(part("firmware/rtw88/rtw8821c_fw.bin"), "data/firmware/link-part.tmp");
    assert_eq!(part("rel/MANIFEST"), "data/link/link-part.tmp");
}

#[test]
fn the_place_itself_and_moves_between_places_are_refused() {
    assert!(!changeable("firmware"));
    assert!(changeable("firmware/rtw88"));
    assert!(one_place("firmware/a.bin", "firmware/rtw88/a.bin"));
    assert!(one_place("x.bin", "y/x.bin"));
    assert!(!one_place("x.bin", "firmware/x.bin"), "the two are separate clients");
    assert!(!one_place("firmware/x.bin", "x.bin"));
    assert!(!one_place("firmware", "old-firmware"));
}

#[test]
fn listings_leave_out_the_part_files_and_a_shadowed_directory() {
    assert!(hidden(".", false, false) && hidden("..", true, true));
    assert!(hidden("link-part.tmp", true, false));
    assert!(!hidden("link-part.tmp", false, false), "only at a place's top");
    assert!(hidden("firmware", true, true), "data/link/firmware/ is not listed where firmware/ is data/firmware/");
    assert!(!hidden("firmware", true, false));
}
