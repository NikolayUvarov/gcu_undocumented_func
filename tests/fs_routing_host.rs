// 311-KRN-0190: mind::fs asks vfs_server for its four volumes and storefs for every other, and nobody for a store volume
// when the program holds no storefs client; storefs refuses the same four names.
#[path = "../libmind/src/fs/volumes.rs"]
mod volumes;
use volumes::{again, on_vfs, route, VFS_VOLUMES};

const VFS: usize = 3;
const STORE: usize = 42;
const FILE: usize = 7;

#[test]
fn vfs_servers_four_volumes_go_to_it_in_any_case() {
    for v in ["", "ram", "RAM", "log", "Log", "models", "MODELS"] {
        assert!(on_vfs(v), "{v}");
        assert_eq!(route(v, VFS, VFS, Some(STORE)), Some(VFS), "{v}");
        assert_eq!(route(v, VFS, VFS, None), Some(VFS), "{v}");
    }
}

#[test]
fn another_volume_goes_to_storefs_or_to_nobody() {
    for v in ["notes", "rams", "data", "fs", "Photos-2026"] {
        assert!(!on_vfs(v), "{v}");
        assert_eq!(route(v, VFS, VFS, Some(STORE)), Some(STORE), "{v}");
        assert_eq!(route(v, VFS, VFS, None), None, "{v}: nothing is asked without storefs's client");
    }
}

#[test]
fn a_lent_client_takes_vfs_servers_volumes_and_storefs_the_others_when_held() {
    // 311-KRN-0203: a program the shell lent its files client (fm, get -o) reaches a store volume through the storefs
    // client it holds too; without one, the lent client is asked, as before.
    for v in ["", "ram", "log", "models"] {
        assert_eq!(route(v, FILE, VFS, Some(STORE)), Some(FILE), "{v}");
        assert_eq!(route(v, FILE, VFS, None), Some(FILE), "{v}");
    }
    for v in ["notes", "data"] {
        assert_eq!(route(v, FILE, VFS, Some(STORE)), Some(STORE), "{v}");
        assert_eq!(route(v, FILE, VFS, None), Some(FILE), "{v}");
    }
}

#[test]
fn storefs_refuses_the_names_vfs_server_holds() {
    // storefs/src/fs.rs (STO's) names them apart from the boot disk's empty name, which its rule of 1 to 16 bytes refuses.
    let source = include_str!("../storefs/src/fs.rs");
    let line = source.lines().find(|l| l.starts_with("pub const VFS_VOLUMES")).expect("storefs's VFS_VOLUMES");
    let named: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
    let ours: Vec<&str> = VFS_VOLUMES.iter().copied().filter(|v| !v.is_empty()).collect();
    assert_eq!(named, ours);
}

// 311-KRN-0207: a kept root that answers invalid (its service restarted) is opened again once, and the call asked once
// more; a second invalid, and one with no kept root, come back as they are; any other answer is not asked again.
#[derive(Clone, Copy, Debug, PartialEq)]
enum E { Invalid, NotFound }

#[test]
fn a_stale_root_is_opened_again_once() {
    let (mut calls, mut forgotten) = (0, 0);
    let got = again(E::Invalid, || { calls += 1; if calls == 1 { Err(E::Invalid) } else { Ok(7) } }, || { forgotten += 1; true });
    assert_eq!((got, calls, forgotten), (Ok(7), 2, 1));
}

#[test]
fn a_second_invalid_and_other_answers_are_kept() {
    let (mut calls, mut forgotten) = (0, 0);
    let got: Result<u8, E> = again(E::Invalid, || { calls += 1; Err(E::Invalid) }, || { forgotten += 1; true });
    assert_eq!((got, calls, forgotten), (Err(E::Invalid), 2, 1));
    // No kept root to drop: the first answer stands.
    let mut calls = 0;
    let got: Result<u8, E> = again(E::Invalid, || { calls += 1; Err(E::Invalid) }, || false);
    assert_eq!((got, calls), (Err(E::Invalid), 1));
    // Not invalid: never asked again, nothing dropped.
    let (mut calls, mut forgotten) = (0, 0);
    let got: Result<u8, E> = again(E::Invalid, || { calls += 1; Err(E::NotFound) }, || { forgotten += 1; true });
    assert_eq!((got, calls, forgotten), (Err(E::NotFound), 1, 0));
    let got: Result<u8, E> = again(E::Invalid, || Ok(1), || panic!("an answer that is not invalid drops nothing"));
    assert_eq!(got, Ok(1));
}
