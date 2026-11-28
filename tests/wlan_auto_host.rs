//! The station's automatic join (550-NET-0041): its schedule of tries and its choice among remembered networks.
#[path = "../wlan/src/auto.rs"]
mod auto;
use auto::{choose, Auto, FIRST_MS, LATER_MS};

#[test]
fn tries_every_ten_seconds_for_two_minutes_then_every_minute() {
    let mut a = Auto::new();
    assert!(a.due(0));
    let (mut now, mut waits) = (5_000, Vec::new());
    for _ in 0..16 {
        assert!(a.due(now));
        a.begin(now);
        let wait = a.ended(now + 2_000);
        assert!(!a.due(now + 2_000 + wait - 1));
        waits.push(wait);
        now += 2_000 + wait;
    }
    // Tries end 2 s after they begin: 10 waits of 10 s fit the first two minutes, then a minute each.
    assert_eq!(&waits[..10], &[FIRST_MS; 10]);
    assert!(waits[10..].iter().all(|&w| w == LATER_MS), "{waits:?}");
    assert_eq!(a.tries, 16);
}

#[test]
fn leave_stops_it_until_a_join_and_a_join_starts_a_new_search() {
    let mut a = Auto::new();
    a.begin(0);
    a.ended(1_000);
    a.left();
    assert!(!a.due(1_000_000));
    a.joined();
    assert!(a.on && a.due(0) && a.tries == 0);
    // Nothing remembered: looked at again after a first wait, no try counted.
    a.idle(500);
    assert!(!a.due(500 + FIRST_MS - 1) && a.due(500 + FIRST_MS) && a.tries == 0);
}

#[test]
fn chooses_the_network_last_joined_else_the_strongest_remembered() {
    let heard: [(&[u8], u8); 4] = [(b"cafe", 40), (b"home", 70), (b"office", 55), (b"", 30)];
    let kept = |s: &[u8]| s == b"home" || s == b"office";
    assert_eq!(choose(heard.iter().copied(), kept, b""), Some(&b"office"[..]));
    assert_eq!(choose(heard.iter().copied(), kept, b"home"), Some(&b"home"[..]));
    // A previous network not heard, or not remembered (forgotten), is no reason to wait.
    assert_eq!(choose(heard.iter().copied(), kept, b"gone"), Some(&b"office"[..]));
    assert_eq!(choose(heard.iter().copied(), |s: &[u8]| s == b"office", b"home"), Some(&b"office"[..]));
    // An unknown signal ranks last; nothing remembered heard, nothing chosen; a hidden network is never chosen.
    assert_eq!(choose([(&b"a"[..], 0u8), (&b"b"[..], 90)].into_iter(), |_| true, b""), Some(&b"b"[..]));
    assert_eq!(choose(heard.iter().copied(), |_| false, b""), None);
    assert_eq!(choose([(&b""[..], 20u8)].into_iter(), |_| true, b""), None);
}

#[test]
fn an_access_point_set_aside_waits_its_time_and_a_join_lets_all_go() {
    use auto::{SetAside, SET_ASIDE, SET_ASIDE_MS};
    let (a, b) = ([1; 6], [2; 6]);
    let mut set = SetAside::new();
    let mut out = [[0; 6]; SET_ASIDE];
    set.add(a, 1_000);
    let n = set.at(1_000, &mut out);
    assert_eq!(&out[..n], &[a]);
    // Set aside again later: its time starts again, and it is held once.
    set.add(b, 2_000);
    set.add(a, 3_000);
    let n = set.at(3_000, &mut out);
    assert_eq!(&out[..n], &[a, b]);
    // B's time ends first; A's a moment after.
    let n = set.at(2_000 + SET_ASIDE_MS, &mut out);
    assert_eq!(&out[..n], &[a]);
    assert_eq!(set.at(3_000 + SET_ASIDE_MS, &mut out), 0);
    // When full, a new one takes the oldest one's place.
    for i in 0..SET_ASIDE as u8 { set.add([10 + i; 6], 10_000 + i as u64); }
    set.add([99; 6], 20_000);
    let n = set.at(20_000, &mut out);
    assert_eq!(n, SET_ASIDE);
    assert!(!out[..n].contains(&[10; 6]) && out[..n].contains(&[99; 6]));
    // A join lets every one go.
    set.clear();
    assert_eq!(set.at(20_000, &mut out), 0);
}
