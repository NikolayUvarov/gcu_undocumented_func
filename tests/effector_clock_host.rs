//! Host tests of the agent's rule for the clock from the pinned server's `Date` (801-EFF-0040).
#[path = "../effector/src/clock.rs"] mod clock;
use clock::*;

#[test]
fn the_clock_is_set_when_more_than_five_minutes_off_within_the_floor_and_2099() {
    let (floor, server) = (1_790_000_000, 1_795_000_000);
    // Within five minutes, either way: left to timed or the user.
    assert_eq!(to_set(server, Some(server + OFF_S), floor), None);
    assert_eq!(to_set(server, Some(server - OFF_S), floor), None);
    // More: set, ahead or behind, or years behind (2000-01-01).
    assert_eq!(to_set(server, Some(server + OFF_S + 1), floor), Some(server));
    assert_eq!(to_set(server, Some(server - 3 * 3600), floor), Some(server));
    assert_eq!(to_set(server, Some(946_684_800), floor), Some(server));
    // A device clock that cannot be read is set.
    assert_eq!(to_set(server, None, floor), Some(server));
    // A Date before the build's commit time, or past 2099, is not used.
    assert_eq!(to_set(floor - 1, Some(946_684_800), floor), None);
    assert_eq!(to_set(floor, Some(946_684_800), floor), Some(floor));
    assert_eq!(to_set(LAST_UNIX + 1, Some(server), floor), None);
    assert_eq!(to_set(LAST_UNIX, Some(server), floor), Some(LAST_UNIX));
}
