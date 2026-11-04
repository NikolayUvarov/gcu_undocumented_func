//! Host tests of `timed`'s SNTP (libmind/src/sntp.rs, 111-NET-0057): the codec, the choice of agreeing servers, the
//! floor and the step bound.
#[path = "../libmind/src/sntp.rs"]
#[allow(dead_code)]
mod sntp;
use sntp::*;

const NONCE: u64 = 0x1234_5678_9abc_def1;
// 2026-11-04 12:00:00 UTC.
const NOW: i64 = 1_793_793_600_000;

fn answer(leap_version_mode: u8, stratum: u8, origin: u64, receive_ms: i64, transmit_ms: i64) -> [u8; PACKET] {
    let mut p = [0u8; PACKET];
    p[0] = leap_version_mode;
    p[1] = stratum;
    p[24..32].copy_from_slice(&origin.to_be_bytes());
    p[32..40].copy_from_slice(&timestamp(receive_ms).to_be_bytes());
    p[40..48].copy_from_slice(&timestamp(transmit_ms).to_be_bytes());
    p
}

#[test]
fn the_request_is_version_4_client_mode_with_the_nonce() {
    let p = request(NONCE);
    assert_eq!(p[0], 0x23);
    assert!(p[1..40].iter().all(|&b| b == 0));
    assert_eq!(&p[40..48], &NONCE.to_be_bytes());
}

#[test]
fn timestamps_round_trip_across_eras() {
    for ms in [0, 1, 999, NOW, NOW + 123, 2_085_978_496_000 - 1000, 2_085_978_496_000, 2_085_978_496_000 + 86_400_123, 4_000_000_000_000] {
        let back = unix_ms(timestamp(ms));
        assert!((back - ms).abs() <= 1, "{ms} came back as {back}");
    }
    // 2036-02-07 06:28:16 UTC is era 1's second 0.
    assert_eq!(timestamp(2_085_978_496_000) >> 32, 0);
    assert_eq!(unix_ms(0), 2_085_978_496_000);
}

#[test]
fn a_servers_answer_is_read() {
    let got = parse(&answer(0x24, 2, NONCE, NOW, NOW + 1), NONCE).unwrap();
    assert_eq!(got.stratum, 2);
    assert_eq!(unix_ms(got.receive), NOW);
    assert_eq!(unix_ms(got.transmit), NOW + 1);
    // Version 3 too, and a leap second announced.
    assert!(parse(&answer(0x1c, 1, NONCE, NOW, NOW), NONCE).is_ok());
    assert!(parse(&answer(0x64, 1, NONCE, NOW, NOW), NONCE).is_ok());
}

#[test]
fn every_field_that_makes_an_answer_wrong_is_refused() {
    let good = answer(0x24, 2, NONCE, NOW, NOW);
    assert_eq!(parse(&good[..47], NONCE), Err(Refused::Short));
    assert_eq!(parse(&answer(0x23, 2, NONCE, NOW, NOW), NONCE), Err(Refused::NotAServer));
    assert_eq!(parse(&answer(0x25, 2, NONCE, NOW, NOW), NONCE), Err(Refused::NotAServer));
    assert_eq!(parse(&answer(0x14, 2, NONCE, NOW, NOW), NONCE), Err(Refused::Version(2)));
    assert_eq!(parse(&answer(0x2c, 2, NONCE, NOW, NOW), NONCE), Err(Refused::Version(5)));
    assert_eq!(parse(&answer(0xe4, 2, NONCE, NOW, NOW), NONCE), Err(Refused::Unsynchronised));
    assert_eq!(parse(&answer(0x24, 0, NONCE, NOW, NOW), NONCE), Err(Refused::Stratum(0)));
    assert_eq!(parse(&answer(0x24, 16, NONCE, NOW, NOW), NONCE), Err(Refused::Stratum(16)));
    // A wrong origin time: an answer to another request, or a forged one that did not see ours.
    assert_eq!(parse(&good, NONCE ^ 1), Err(Refused::NotOurs));
    assert_eq!(parse(&answer(0x24, 2, 0, NOW, NOW), NONCE), Err(Refused::NotOurs));
    let mut empty = good;
    empty[40..48].fill(0);
    assert_eq!(parse(&empty, NONCE), Err(Refused::NoTime));
    let mut empty = good;
    empty[32..40].fill(0);
    assert_eq!(parse(&empty, NONCE), Err(Refused::NoTime));
    // Longer packets (extension fields) are read by their first 48 bytes.
    let mut long = [0u8; 68];
    long[..48].copy_from_slice(&good);
    assert!(parse(&long, NONCE).is_ok());
}

#[test]
fn the_offset_and_round_trip_of_an_answer() {
    // The clock is 10 s behind; the request takes 20 ms each way, and the server 2 ms to answer.
    let sent = NOW - 10_000;
    let got = Answer { stratum: 2, receive: timestamp(NOW + 20), transmit: timestamp(NOW + 22) };
    let s = sample(&got, sent, sent + 42);
    assert!((s.offset_ms - 10_000).abs() <= 1, "{s:?}");
    assert!((s.delay_ms - 40).abs() <= 1, "{s:?}");
    // A server's time that runs ahead of the answer's arrival gives no negative round trip.
    let odd = Answer { stratum: 2, receive: timestamp(NOW), transmit: timestamp(NOW + 500) };
    assert_eq!(sample(&odd, NOW, NOW + 10).delay_ms, 0);
}

#[test]
fn servers_that_agree_are_followed() {
    let s = |offset_ms, delay_ms| Sample { offset_ms, delay_ms };
    assert_eq!(choose(&[]), Err(NoChoice::TooFew));
    assert_eq!(choose(&[s(5000, 10)]), Err(NoChoice::TooFew));
    // Two that agree within a second: the middle of the two.
    assert_eq!(choose(&[s(5000, 10), s(5400, 30)]), Ok(Choice { offset_ms: 5200, agreed: 0b11 }));
    // Two that do not.
    assert_eq!(choose(&[s(5000, 10), s(8000, 10)]), Err(NoChoice::NoneAgree));
    // Round trips widen the agreement: 1.5 s apart, with 1.2 s of round trips between them.
    assert!(choose(&[s(0, 600), s(1500, 600)]).is_ok());
    assert!(choose(&[s(0, 100), s(1500, 100)]).is_err());
    // One server disagreeing with two others is left out.
    assert_eq!(choose(&[s(3_600_000, 10), s(5000, 10), s(5100, 10)]), Ok(Choice { offset_ms: 5050, agreed: 0b110 }));
    // Three of four agree: their median.
    assert_eq!(choose(&[s(100, 5), s(-200_000, 5), s(300, 5), s(200, 5)]), Ok(Choice { offset_ms: 200, agreed: 0b1101 }));
    // Two against two: no majority.
    assert_eq!(choose(&[s(0, 5), s(100, 5), s(50_000, 5), s(50_100, 5)]), Err(NoChoice::Split));
    // A chain (the first agrees with the second, the second with the third, not the first with the third).
    assert_eq!(choose(&[s(0, 0), s(900, 0), s(1800, 0)]), Err(NoChoice::Split));
}

#[test]
fn the_floor_and_the_step_bound() {
    let floor = NOW - 86_400_000;
    // Before the floor: never, not even at the first set.
    assert_eq!(decide(NOW, floor - 1000, floor, true, true), Decision::BeforeFloor);
    assert_eq!(decide(NOW, floor, floor, false, false), Decision::Set);
    // A small correction is taken as it comes.
    assert_eq!(decide(NOW, NOW + 5000, floor, false, false), Decision::Set);
    assert_eq!(decide(NOW, NOW - STEP_MS, floor - STEP_MS, false, false), Decision::Set);
    // More than 24 hours: at the first set since boot, or with the user's yes.
    let far = NOW + STEP_MS + 1000;
    assert_eq!(decide(NOW, far, floor, false, false), Decision::StepTooLarge);
    assert_eq!(decide(NOW, far, floor, true, false), Decision::Set);
    assert_eq!(decide(NOW, far, floor, false, true), Decision::Set);
    // From a clock years behind, at boot.
    assert_eq!(decide(rtc_ms(0, 0), NOW, floor, true, false), Decision::Set);
    // Past 2099, which the rtc service does not keep.
    assert_eq!(decide(NOW, rtc_ms(36_525, 0), floor, true, true), Decision::OutOfRange);
}

#[test]
fn the_rtc_services_day_and_second() {
    assert_eq!(rtc_parts(946_684_800_000), Some((0, 0)));
    assert_eq!(rtc_ms(0, 0), 946_684_800_000);
    let (date, seconds) = rtc_parts(NOW).unwrap();
    assert_eq!((date, seconds), (9804, 43_200));
    assert_eq!(rtc_ms(date, seconds), NOW);
    // Rounded to the nearest second.
    assert_eq!(rtc_parts(NOW + 499), Some((9804, 43_200)));
    assert_eq!(rtc_parts(NOW + 500), Some((9804, 43_201)));
    assert_eq!(rtc_parts(946_684_800_000 - 1000), None);
    assert_eq!(rtc_parts(rtc_ms(36_524, 86_399)), Some((36_524, 86_399)));
    assert_eq!(rtc_parts(rtc_ms(36_525, 0)), None);
}

#[test]
fn the_floor_file() {
    let mut out = [0u8; 16];
    let n = floor_text(NOW + 999, &mut out);
    assert_eq!(&out[..n], b"1793793600\n");
    assert_eq!(parse_floor(&out[..n]), Some(NOW));
    assert_eq!(parse_floor(b"1793793600"), Some(NOW));
    assert_eq!(floor_text(0, &mut out), 2);
    for bad in [&b""[..], b"\n", b"-5\n", b"12a\n", b"1793793600\n\n", b" 1793793600\n", b"1234567890123\n"] {
        assert_eq!(parse_floor(bad), None, "{bad:?}");
    }
}

// Seeded fuzzing of the answer's parser and of the choice (111-NET-0057): random and mutated packets never panic, and
// an answer taken always keeps every rule; a run is evidence of the inputs it made, not a proof (MC-12.2).
// MIND_FUZZ_SEED and MIND_FUZZ_ITERATIONS override the fixed seed and count.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) }
}

fn fuzz_setup() -> (Rng, u64) {
    let seed: u64 = std::env::var("MIND_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(111_0057);
    let iterations = std::env::var("MIND_FUZZ_ITERATIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(200_000);
    (Rng(seed | 1), iterations)
}

#[test]
fn fuzzed_answers_never_panic_and_a_taken_one_keeps_every_rule() {
    let (mut rng, iterations) = fuzz_setup();
    let good = answer(0x24, 2, NONCE, NOW, NOW + 1);
    let mut taken = 0;
    for _ in 0..iterations {
        let mut packet = [0u8; 96];
        let length = (rng.next() % 97) as usize;
        if rng.next() % 2 == 0 {
            packet[..48].copy_from_slice(&good);
            for _ in 0..1 + rng.next() % 4 { packet[(rng.next() % 48) as usize] ^= 1 << (rng.next() % 8); }
        } else {
            for byte in packet.iter_mut() { *byte = rng.next() as u8; }
        }
        let nonce = if rng.next() % 4 == 0 { rng.next() } else { NONCE };
        if let Ok(got) = parse(&packet[..length], nonce) {
            taken += 1;
            assert!(length >= PACKET);
            assert_eq!(packet[0] & 7, 4);
            assert!((3..=4).contains(&(packet[0] >> 3 & 7)) && packet[0] >> 6 != 3);
            assert!((1..=15).contains(&got.stratum));
            assert_eq!(&packet[24..32], &nonce.to_be_bytes());
            assert!(got.receive != 0 && got.transmit != 0);
            let _ = sample(&got, NOW, NOW + (rng.next() % 10_000) as i64);
        }
    }
    assert!(taken > 0, "no fuzzed answer was taken: the mutations never kept a valid one");
}

#[test]
fn fuzzed_samples_are_chosen_only_where_every_member_agrees() {
    let (mut rng, iterations) = fuzz_setup();
    for _ in 0..iterations / 10 {
        let n = (rng.next() % 9) as usize;
        let mut samples = [Sample { offset_ms: 0, delay_ms: 0 }; 8];
        for s in samples.iter_mut().take(n) {
            let spread = [2_000i64, 20_000, 1 << 40][(rng.next() % 3) as usize];
            *s = Sample { offset_ms: (rng.next() as i64 % spread), delay_ms: (rng.next() % 3_000) as i64 };
        }
        if let Ok(choice) = choose(&samples[..n]) {
            let members: Vec<_> = (0..n).filter(|i| choice.agreed >> i & 1 == 1).collect();
            assert!(members.len() >= 2);
            for &i in &members {
                for &j in &members {
                    let (a, b) = (samples[i], samples[j]);
                    assert!((a.offset_ms - b.offset_ms).abs() <= AGREE_MS + (a.delay_ms + b.delay_ms) / 2);
                }
            }
            let (low, high) = members.iter().fold((i64::MAX, i64::MIN), |(l, h), &i| (l.min(samples[i].offset_ms), h.max(samples[i].offset_ms)));
            assert!(low <= choice.offset_ms && choice.offset_ms <= high);
        }
    }
}
