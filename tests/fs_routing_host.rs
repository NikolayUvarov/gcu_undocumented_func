// 311-KRN-0190: mind::fs asks vfs_server for its four volumes and storefs for every other, and nobody for a store volume
// when the program holds no storefs client; storefs refuses the same four names.
#[path = "../libmind/src/fs/volumes.rs"]
mod volumes;
use volumes::{on_vfs, route, VFS_VOLUMES};

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
fn a_lent_client_takes_every_volume() {
    for v in ["", "ram", "notes"] {
        assert_eq!(route(v, FILE, VFS, Some(STORE)), Some(FILE), "{v}");
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
