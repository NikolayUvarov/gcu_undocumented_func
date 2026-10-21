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
