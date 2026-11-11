//! Seeded fuzzing of httpd's request head, path and range parsers, as 500-ASR-0001's harness runs the IDL decoders:
//! random and mutated input must never panic, an accepted path names only the release layout, and a range stays inside
//! its file. A run is evidence of the executions it made, not a proof (MC-12.2). MIND_FUZZ_SEED and
//! MIND_FUZZ_ITERATIONS override the fixed seed and count.

use httpd::{head, layout, range};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: usize) -> usize { (self.next() % n.max(1) as u64) as usize }
}

fn env(name: &str, default: u64) -> u64 { std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default) }

const SEEDS: &[&[u8]] = &[
    b"GET /blobs/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef HTTP/1.1\r\nHost: x\r\nRange: bytes=10-\r\n\r\n",
    b"HEAD /releases/6/x86_64/MANIFEST.SIG HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    b"GET /channels/stable HTTP/1.0\nConnection: keep-alive\nIf-None-Match: \"1-2\"\n\n",
    b"GET /channels/../blobs?x=%2e HTTP/1.1\r\nHost: a\r\nRange: bytes=-5,1-2\r\n\r\n",
];
const PIECES: &[&[u8]] = &[b"/", b"..", b".", b"%", b"?", b"\\", b"\r\n", b"\n", b":", b" ", b"\t", b"\0", b"blobs", b"releases", b"channels",
    b"x86_64", b"aarch64", b"MANIFEST", b"-", b",", b"=", b"bytes", b"Range", b"Host", b"\xff\xfe", b"\xc3\xa9", b"99999999999999999999"];

fn mutate(rng: &mut Rng, input: &mut Vec<u8>) {
    for _ in 0..1 + rng.below(4) {
        let at = rng.below(input.len() + 1);
        match rng.below(5) {
            0 if !input.is_empty() => { let i = rng.below(input.len()); input[i] = rng.next() as u8; }
            1 if !input.is_empty() => { let end = (at + rng.below(8)).min(input.len()); input.drain(at.min(end)..end); }
            2 => { let piece = PIECES[rng.below(PIECES.len())]; input.splice(at..at, piece.iter().copied()); }
            3 => { let n = rng.below(16); input.splice(at..at, (0..n).map(|_| rng.next() as u8)); }
            _ => { let n = rng.below(input.len() + 1); let copy = input[..n].to_vec(); input.extend_from_slice(&copy); input.truncate(head::HEAD_MAX + 64); }
        }
    }
}

// What `resolve` accepts: one of the layout's three shapes, no name empty or starting with a dot.
fn layout_shaped(path: &str) -> bool {
    let names: Vec<&str> = path.split('/').collect();
    !names.iter().any(|n| n.is_empty() || n.starts_with('.'))
        && path.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'))
        && matches!(names.as_slice(), ["blobs", _] | ["releases", _, "x86_64" | "aarch64", "MANIFEST" | "MANIFEST.SIG"] | ["channels", _])
}

#[test]
fn the_parsers_on_random_and_mutated_input() {
    let mut rng = Rng(env("MIND_FUZZ_SEED", 0x6d69_6e64_6874_7470) | 1);
    let iterations = env("MIND_FUZZ_ITERATIONS", 200_000);
    let (mut parsed, mut resolved, mut parts) = (0u64, 0u64, 0u64);
    for i in 0..iterations {
        let mut input = if i % 8 == 0 { (0..rng.below(300)).map(|_| rng.next() as u8).collect() } else { SEEDS[rng.below(SEEDS.len())].to_vec() };
        mutate(&mut rng, &mut input);
        if let Some(end) = head::end(&input) { assert!(end <= input.len()); }
        let Ok(request) = head::parse(&input) else { continue };
        parsed += 1;
        let _ = request.keep_alive();
        if let Ok((path, _)) = layout::resolve(&request.target) {
            resolved += 1;
            assert!(layout_shaped(&path), "accepted outside the layout: {path:?} from {:?}", request.target);
        }
        let len = rng.next() % 1_000_000;
        match range::asked(request.header("range"), len) {
            range::Asked::Part(first, last) => { parts += 1; assert!(first <= last && last < len, "{first}-{last} of {len}"); }
            range::Asked::Whole | range::Asked::Unsatisfiable => {}
        }
    }
    // The run reached past the parsers' first refusals.
    assert!(parsed > iterations / 20 && resolved > 0 && parts > 0, "parsed {parsed}, resolved {resolved}, ranges {parts}");
    println!("httpd fuzz: {iterations} inputs, {parsed} heads parsed, {resolved} paths resolved, {parts} ranges");
}
