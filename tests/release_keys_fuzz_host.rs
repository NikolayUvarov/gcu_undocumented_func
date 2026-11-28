// A fuzzer of the update's key file and of the signature lines (351-ASR-0017, for 351-UPD-0026): libmind's
// parse_keys and parse_channel, with a fixed seed, from the files release.py makes. A finding fails the test with the
// input that made it. Evidence for these parsers on the host only (MC-12.2: a fuzzer finds, it does not prove).
#[path = "../libmind/src/release.rs"]
#[allow(dead_code)]
mod release;

use release::{channel_is, encode_channel, encode_keys, keys_is, parse_channel, parse_keys, signed_by, Signature, Signed, KEYS_MAX, SIGNATURES_MAX};
use std::process::Command;

fn python(code: &str) -> Vec<u8> {
    let out = Command::new("python3").args(["-c", &format!("import sys; sys.path.insert(0, 'scripts')\n{}", code)]).output().expect("python3");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

// Key files as release.py makes them, signed by `roots` (Python expressions over release.TEST_ROOT_SEEDS).
fn keys_file(roots: &str) -> Vec<u8> {
    python(&format!("import release, datetime\nnow = datetime.datetime(2026, 11, 13, tzinfo=datetime.timezone.utc)\n\
                     sys.stdout.buffer.write(release.test_keys_file(now, roots=[{roots}]))"))
}

// A channel file as release.py publishes it, signed by `seeds`.
fn channel_file(seeds: &str) -> Vec<u8> {
    python(&format!("import release\nbody = release.channel_bytes('stable', 7, 5, '2026-11-08T00:00:00Z', {{'x86_64': 'ab' * 32, 'aarch64': '0c' * 32}})\n\
                     sys.stdout.buffer.write(body + release.signature_lines(body, [{seeds}]))"))
}

// xorshift64*: the same inputs on every run.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) }
    fn below(&mut self, n: usize) -> usize { if n == 0 { 0 } else { (self.next() % n as u64) as usize } }
}

// Bytes the formats are made of, so that a mutation often stays near a form the parsers take.
const INTERESTING: &[u8] = b"0123456789abcdefABCDEF\",:{}[]\n \r\x00\x80\xffed25519TZ-";

fn mutate(rng: &mut Rng, seeds: &[Vec<u8>], input: &[u8]) -> Vec<u8> {
    let mut v = input.to_vec();
    for _ in 0..1 + rng.below(4) {
        let at = rng.below(v.len() + 1);
        match rng.below(10) {
            0 | 1 if !v.is_empty() => { let i = at.min(v.len() - 1); v[i] = INTERESTING[rng.below(INTERESTING.len())]; }
            2 if !v.is_empty() => { let i = at.min(v.len() - 1); v[i] ^= 1 << rng.below(8); }
            3 => v.insert(at, INTERESTING[rng.below(INTERESTING.len())]),
            4 if !v.is_empty() => { let n = 1 + rng.below(8); let end = (at + n).min(v.len()); v.drain(at.min(v.len())..end); }
            5 if !v.is_empty() => { // a range of the input repeated where it is
                let from = rng.below(v.len()); let n = (1 + rng.below(200)).min(v.len() - from);
                let part: Vec<u8> = v[from..from + n].to_vec(); v.splice(at..at, part);
            }
            6 => { // a range of another seed put in place of one of this input
                let other = &seeds[rng.below(seeds.len())];
                let from = rng.below(other.len()); let n = (1 + rng.below(200)).min(other.len() - from);
                let end = (at + rng.below(200)).min(v.len());
                v.splice(at..end, other[from..from + n].iter().copied());
            }
            7 => v.truncate(at),
            8 => { // a signature line repeated or dropped
                let lines: Vec<usize> = v.iter().enumerate().filter(|(_, &b)| b == b'\n').map(|(i, _)| i + 1).collect();
                if lines.len() >= 2 {
                    let k = rng.below(lines.len() - 1);
                    let (start, end) = (lines[k], lines[k + 1]);
                    if rng.below(2) == 0 { let line = v[start..end].to_vec(); v.splice(end..end, line); } else { v.drain(start..end); }
                }
            }
            _ if !v.is_empty() => { // a decimal digit run made longer, to reach the counts' and versions' edges
                let i = at.min(v.len() - 1);
                if v[i].is_ascii_digit() { let n = 1 + rng.below(20); v.splice(i..i, std::iter::repeat(b'9').take(n)); }
            }
            _ => {}
        }
    }
    if rng.below(64) == 0 { v.resize(KEYS_MAX + rng.below(3), b'\n'); }
    v
}

// Whether `signed_by` counted exactly the distinct keys of `keys` that signed, under a check that passes the pairs in
// `good` alone.
fn signed_by_is_a_count(keys: &[[u8; 32]], threshold: u8, signed: &Signed, good: &dyn Fn(&[u8; 32], &[u8; 64]) -> bool) -> bool {
    let mut distinct: Vec<[u8; 32]> = Vec::new();
    for key in keys {
        if distinct.contains(key) { continue; }
        if signed.signatures().iter().any(|s| s.key == *key && good(&s.key, &s.value)) { distinct.push(*key); }
    }
    let expected = threshold > 0 && distinct.len() >= threshold as usize;
    signed_by(keys, threshold, b"body", signed, &|k: &[u8; 32], _: &[u8], v: &[u8; 64]| good(k, v)) == expected
}

// The properties a taken key file must have (351-UPD-0026): it encodes again to exactly its body, keys_is holds for it,
// and its signatures count each key once and no key outside the set given.
fn check_keys(input: &[u8]) {
    let Ok((keys, signed)) = parse_keys(input) else { return };
    let mut body = [0u8; KEYS_MAX];
    let n = encode_keys(&keys, &mut body).unwrap_or_else(|| panic!("taken but not encoded: {:?}", String::from_utf8_lossy(input)));
    assert_eq!(&body[..n], &input[..signed.body], "taken, encoded otherwise: {:?}", String::from_utf8_lossy(input));
    assert!(keys_is(input, &keys, &signed), "taken, keys_is false: {:?}", String::from_utf8_lossy(input));
    assert!(signed.count >= 1 && signed.count <= SIGNATURES_MAX);
    check_counting(&keys.root().keys().to_vec(), keys.root().threshold, &signed);
}

fn check_channel(input: &[u8]) {
    let Ok((channel, signed)) = parse_channel(input) else { return };
    let mut body = [0u8; KEYS_MAX];
    let n = encode_channel(&channel, &mut body).unwrap_or_else(|| panic!("taken but not encoded: {:?}", String::from_utf8_lossy(input)));
    assert_eq!(&body[..n], &input[..signed.body], "taken, encoded otherwise: {:?}", String::from_utf8_lossy(input));
    assert!(channel_is(input, &channel, &signed), "taken, channel_is false: {:?}", String::from_utf8_lossy(input));
}

// signed_by over the signers named, keys outside them, and repeats, under checks that pass all, none, or some.
fn check_counting(root: &[[u8; 32]], threshold: u8, signed: &Signed) {
    let signers: Vec<[u8; 32]> = signed.signatures().iter().map(|s| s.key).collect();
    let outsider = [0x5au8; 32];
    let sets: [Vec<[u8; 32]>; 4] = [root.to_vec(), signers.clone(), [signers.clone(), signers.clone()].concat(), [signers.clone(), vec![outsider], root.to_vec()].concat()];
    let all = |_: &[u8; 32], _: &[u8; 64]| true;
    let none = |_: &[u8; 32], _: &[u8; 64]| false;
    let even = |_: &[u8; 32], v: &[u8; 64]| v[0] % 2 == 0;
    for keys in &sets {
        for t in [0u8, 1, 2, threshold, keys.len() as u8, keys.len() as u8 + 1] {
            for good in [&all as &dyn Fn(&[u8; 32], &[u8; 64]) -> bool, &none, &even] {
                assert!(signed_by_is_a_count(keys, t, signed, good), "signed_by miscounted: {} keys, threshold {}", keys.len(), t);
            }
        }
    }
}

#[test]
fn the_seeds_are_taken_as_they_are() {
    for roots in ["release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[1]", "release.TEST_ROOT_SEEDS[0]",
                  "release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[1]"] {
        let file = keys_file(roots);
        assert!(parse_keys(&file).is_ok(), "{}", roots);
        check_keys(&file);
    }
    let file = channel_file("release.TEST_RELEASE_SEED");
    assert!(parse_channel(&file).is_ok());
    check_channel(&file);
}

#[test]
fn mutated_key_files_and_channels_never_break_the_parsers_properties() {
    let keys = [keys_file("release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[1]"), keys_file("release.TEST_ROOT_SEEDS[0]"),
                keys_file("release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[0], release.TEST_ROOT_SEEDS[1]")];
    let channels = [channel_file("release.TEST_RELEASE_SEED"), channel_file("release.TEST_RELEASE_SEED, release.TEST_ROOT_SEEDS[0]")];
    let seeds: Vec<Vec<u8>> = keys.iter().chain(channels.iter()).cloned().collect();
    let mut rng = Rng(0x3517_0017_0026_0001);
    let (mut taken_keys, mut taken_channels) = (0u32, 0u32);
    // Mutations of mutations: a quarter start again from a seed.
    let mut current = seeds[0].clone();
    for i in 0..400_000u32 {
        if i % 4 == 0 { current = seeds[rng.below(seeds.len())].clone(); }
        let input = mutate(&mut rng, &seeds, &current);
        if parse_keys(&input).is_ok() { taken_keys += 1; }
        if parse_channel(&input).is_ok() { taken_channels += 1; }
        check_keys(&input);
        check_channel(&input);
        if input.len() <= KEYS_MAX + 2 { current = input; }
    }
    // The mutations reach taken files too, or the properties above would be checked on nothing.
    assert!(taken_keys > 100 && taken_channels > 100, "taken: {} key files, {} channels", taken_keys, taken_channels);
}

#[test]
fn signed_by_counts_each_key_once_on_made_up_signature_sets() {
    let mut rng = Rng(0x0026_5163_0017_0002);
    let pool: Vec<[u8; 32]> = (0..6u8).map(|i| [i * 37 + 1; 32]).collect();
    for _ in 0..20_000 {
        let count = 1 + rng.below(SIGNATURES_MAX);
        let mut signed = Signed { body: 4, signatures: [Signature { key: [0; 32], value: [0; 64] }; SIGNATURES_MAX], count };
        for s in signed.signatures.iter_mut().take(count) { *s = Signature { key: pool[rng.below(pool.len())], value: [rng.below(256) as u8; 64] }; }
        let keys: Vec<[u8; 32]> = (0..rng.below(7)).map(|_| pool[rng.below(pool.len())]).collect();
        check_counting(&keys, rng.below(5) as u8, &signed);
    }
}
