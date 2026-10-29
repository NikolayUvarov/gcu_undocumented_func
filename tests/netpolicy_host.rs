//! Host tests of the network policy broker's client rights (libmind/src/network.rs, 800-NET-0038): a launcher's client
//! (link's in its developer mode) may launch but not change the policy; the shell's, with no badge, may.
#[path = "../libmind/src/network.rs"]
#[allow(dead_code)]
mod network;
use network::{may_change_policy, BADGE_LAUNCH};

#[test]
fn a_launchers_client_may_not_change_the_policy() {
    assert!(may_change_policy(0));
    assert!(!may_change_policy(BADGE_LAUNCH));
    // A badge this version does not know changes nothing either.
    assert!(!may_change_policy(BADGE_LAUNCH | 0x8000));
    assert!(!may_change_policy(0x8000));
}

#[test]
fn which_addresses_are_public() {
    use network::public;
    let ip = |a: u8, b: u8, c: u8, d: u8| u32::from_be_bytes([a, b, c, d]);
    for public_one in [ip(1, 1, 1, 1), ip(8, 8, 8, 8), ip(11, 0, 2, 80), ip(93, 184, 216, 34), ip(100, 63, 255, 255), ip(100, 128, 0, 0),
                       ip(172, 15, 255, 255), ip(172, 32, 0, 0), ip(192, 0, 1, 1), ip(192, 167, 1, 1), ip(198, 17, 0, 1), ip(198, 20, 0, 1), ip(223, 255, 255, 255)] {
        assert!(public(public_one), "{:#x}", public_one);
    }
    for other in [0, ip(0, 1, 2, 3), ip(10, 0, 2, 2), ip(100, 64, 0, 1), ip(100, 127, 255, 255), ip(127, 0, 0, 1), ip(169, 254, 1, 1),
                  ip(172, 16, 0, 1), ip(172, 31, 255, 255), ip(192, 0, 0, 9), ip(192, 0, 2, 1), ip(192, 168, 1, 1), ip(198, 18, 0, 1),
                  ip(198, 19, 255, 255), ip(198, 51, 100, 7), ip(203, 0, 113, 9), ip(224, 0, 0, 1), ip(239, 255, 255, 250), ip(240, 0, 0, 1), u32::MAX] {
        assert!(!public(other), "{:#x}", other);
    }
}

#[test]
fn browsing_lines() {
    use network::browse_line;
    let (program, ports, count, seconds, bytes) = browse_line("browse www public tcp 80,443 3600 1000000").unwrap();
    assert_eq!((program, &ports[..count], seconds, bytes), ("www", &[80u16, 443][..], Some(3600), Some(1000000)));
    let (_, ports, count, seconds, bytes) = browse_line("BROWSE www PUBLIC TCP 443").unwrap();
    assert_eq!((&ports[..count], seconds, bytes), (&[443u16][..], None, None));
    for refused in ["browse www public udp 53", "browse www private tcp 80", "browse www public tcp 0", "browse www public tcp 80,80",
                    "browse www public tcp 1,2,3,4,5", "browse www public tcp 80 soon", "browse www public tcp 80 1 2 3", "browse",
                    "browse thisnameistoolongforone public tcp 80", "www public tcp 80", "browse www public tcp"] {
        assert!(browse_line(refused).is_none(), "{}", refused);
    }
}
