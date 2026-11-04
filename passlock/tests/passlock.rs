//! Host tests of the passphrase's file (308-NET-0055): Argon2id against RFC 9106's own vector (section 5.3), the file
//! rebuilt here from its description, the seal and the opening, a wrong passphrase, the parameters read back, every
//! byte changed, truncation, another version, the memory wiped after each derivation, and the wait after wrong tries.
//! Evidence of these runs on the host, not a proof (MC-12.2).
use argon2::{Algorithm, Argon2, Version};
use chacha20poly1305::aead::AeadInPlace;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use passlock::{Block, Params, Refused, Tries, HEADER, LEN, MAGIC, SEED, VERSION};
use std::time::Instant;

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

fn memory(blocks: usize) -> Vec<Block> { vec![Block::default(); blocks] }

// Whether every byte of `blocks` is zero: Block shows no words, so they are read as bytes.
fn wiped(blocks: &[Block]) -> bool {
    let bytes = unsafe { std::slice::from_raw_parts(blocks.as_ptr() as *const u8, std::mem::size_of_val(blocks)) };
    bytes.iter().all(|&b| b == 0)
}

const SMALL: Params = Params { memory_kib: 64, passes: 2, lanes: 1 };
const PASSPHRASE: &[u8] = b"correct horse battery staple";
const SALT: [u8; 16] = [0x5a; 16];
const NONCE: [u8; 12] = [0xc3; 12];

fn seed() -> [u8; SEED] { core::array::from_fn(|i| 0x80 ^ i as u8) }

fn sealed(params: Params) -> [u8; LEN] {
    passlock::seal(PASSPHRASE, &SALT, &NONCE, params, &seed(), &mut memory(params.blocks().unwrap())).unwrap()
}

#[test]
fn argon2id_rfc9106() {
    // RFC 9106, section 5.3: Argon2id, version 0x13, 32 KiB, 3 passes, 4 lanes, a 32-byte tag.
    let tag = hex("0d 64 0d f5 8d 78 76 6c 08 c0 37 a3 4a 8b 53 c9 d0 1e f0 45 2d 75 b6 5e b5 25 20 e9 6b 01 e6 59");
    let params = Params { memory_kib: 32, passes: 3, lanes: 4 };
    assert_eq!(params.blocks(), Some(32));
    let mut blocks = memory(32);
    let mut out = [0u8; 32];
    passlock::argon2id(&[0x01; 32], &[0x02; 16], &[0x03; 8], &[0x04; 12], params, &mut blocks, &mut out).unwrap();
    assert_eq!(out.to_vec(), tag);
    assert!(wiped(&blocks), "Argon2id's memory left unwiped");
}

#[test]
fn file_rebuilt_from_its_description() {
    // MAGIC, the version, memory, passes and lanes (u32, little-endian), salt, nonce; then the seed under
    // ChaCha20-Poly1305 keyed by Argon2id(passphrase, salt), the header as associated data.
    let params = Params { memory_kib: 72, passes: 3, lanes: 2 };
    let file = sealed(params);
    let mut header = MAGIC.to_vec();
    header.push(1);
    for word in [72u32, 3, 2] { header.extend_from_slice(&word.to_le_bytes()); }
    header.extend_from_slice(&SALT);
    header.extend_from_slice(&NONCE);
    assert_eq!(header.len(), HEADER);
    let mut key = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2::Params::new(72, 3, 2, Some(32)).unwrap())
        .hash_password_into_with_memory(PASSPHRASE, &SALT, &mut key, memory(72)).unwrap();
    let mut body = seed().to_vec();
    let tag = ChaCha20Poly1305::new(&key.into()).encrypt_in_place_detached(Nonce::from_slice(&NONCE), &header, &mut body).unwrap();
    let expected = [header, body, tag.to_vec()].concat();
    assert_eq!(expected.len(), LEN);
    assert_eq!(file.to_vec(), expected);
}

#[test]
fn sealed_then_opened() {
    let file = sealed(SMALL);
    assert!(!file.windows(SEED).any(|w| w == seed()), "the seed in the clear");
    let mut blocks = memory(SMALL.blocks().unwrap());
    assert_eq!(passlock::open(&file, PASSPHRASE, &mut blocks), Ok(seed()));
    assert!(wiped(&blocks));
    // Another salt or nonce, another file; the same seed opens from each.
    let other = passlock::seal(PASSPHRASE, &[0x11; 16], &[0x22; 12], SMALL, &seed(), &mut blocks).unwrap();
    assert!(wiped(&blocks));
    assert_ne!(other[HEADER..], file[HEADER..]);
    assert_eq!(passlock::open(&other, PASSPHRASE, &mut blocks), Ok(seed()));
}

#[test]
fn wrong_passphrase_refused() {
    let file = sealed(SMALL);
    let mut blocks = memory(SMALL.blocks().unwrap());
    for wrong in [&b"correct horse battery stapl"[..], b"correct horse battery staplf", b"correct horse battery staple ", b"Correct horse battery staple", b"12345678"] {
        assert_eq!(passlock::open(&file, wrong, &mut blocks), Err(Refused::Wrong), "{:?}", String::from_utf8_lossy(wrong));
        assert!(wiped(&blocks));
    }
}

#[test]
fn parameters_read_back() {
    for params in [Params::DEFAULT, SMALL, Params { memory_kib: 96, passes: 1, lanes: 3 }] {
        let started = Instant::now();
        let file = sealed(params);
        if params == Params::DEFAULT { eprintln!("Argon2id at {:?}: {:?} on this host (this build's profile)", params, started.elapsed()); }
        let header = passlock::header(&file).unwrap();
        assert_eq!((header.params, header.salt, header.nonce), (params, SALT, NONCE));
        assert_eq!(&file[..9], &[MAGIC.as_slice(), &[VERSION]].concat()[..]);
    }
    assert_eq!(Params::DEFAULT, Params { memory_kib: 9216, passes: 4, lanes: 1 });
    assert_eq!(Params::DEFAULT.blocks(), Some(9216));
}

#[test]
fn parameters_out_of_bounds_refused() {
    for params in [Params { memory_kib: 7, passes: 1, lanes: 1 }, Params { memory_kib: 15, passes: 1, lanes: 2 }, Params { memory_kib: 64, passes: 0, lanes: 1 },
                   Params { memory_kib: 64, passes: 1, lanes: 0 }, Params { memory_kib: 64, passes: Params::PASSES_MAX + 1, lanes: 1 },
                   Params { memory_kib: 256, passes: 1, lanes: Params::LANES_MAX + 1 }, Params { memory_kib: Params::MEMORY_KIB_MAX + 1, passes: 1, lanes: 1 }] {
        assert_eq!(params.blocks(), None, "{params:?}");
        assert_eq!(passlock::seal(PASSPHRASE, &SALT, &NONCE, params, &seed(), &mut memory(64)), Err(Refused::Damaged), "{params:?}");
    }
    assert!(Params { memory_kib: Params::MEMORY_KIB_MAX, passes: Params::PASSES_MAX, lanes: Params::LANES_MAX }.blocks().is_some());
}

#[test]
fn every_byte_changed_refused() {
    let file = sealed(SMALL);
    let mut blocks = memory(1024);
    for at in 0..LEN {
        for bit in [0x01u8, 0x80] {
            let mut changed = file;
            changed[at] ^= bit;
            let opened = passlock::open(&changed, PASSPHRASE, &mut blocks);
            match at {
                0..=7 => assert_eq!(opened, Err(Refused::Damaged), "magic byte {at}"),
                8 => assert_eq!(opened, Err(Refused::Version(VERSION ^ bit))),
                // A parameter changed: out of bounds, more memory than given, or another key.
                9..=20 => assert!(matches!(opened, Err(Refused::Damaged | Refused::Memory | Refused::Wrong)), "parameter byte {at}: {opened:?}"),
                _ => assert_eq!(opened, Err(Refused::Wrong), "byte {at}"),
            }
            assert!(wiped(&blocks));
        }
    }
    assert_eq!(passlock::open(&file, PASSPHRASE, &mut blocks), Ok(seed()));
}

#[test]
fn truncated_or_longer_refused() {
    let file = sealed(SMALL);
    let mut blocks = memory(SMALL.blocks().unwrap());
    for n in 0..LEN {
        assert_eq!(passlock::open(&file[..n], PASSPHRASE, &mut blocks), Err(Refused::Damaged), "{n} bytes");
        assert_eq!(passlock::header(&file[..n]), Err(Refused::Damaged), "{n} bytes");
    }
    assert_eq!(passlock::open(&[file.as_slice(), &[0]].concat(), PASSPHRASE, &mut blocks), Err(Refused::Damaged));
}

#[test]
fn another_version_refused_by_its_number() {
    let file = sealed(SMALL);
    let mut blocks = memory(SMALL.blocks().unwrap());
    for version in [0u8, 2, 0xff] {
        let mut other = file.to_vec();
        other[8] = version;
        assert_eq!(passlock::header(&other), Err(Refused::Version(version)));
        assert_eq!(passlock::open(&other, PASSPHRASE, &mut blocks), Err(Refused::Version(version)));
        // A later version may be longer or shorter: still refused by its number.
        for len in [9, 64, LEN + 32] {
            other.resize(len, 0);
            assert_eq!(passlock::open(&other, PASSPHRASE, &mut blocks), Err(Refused::Version(version)), "{len} bytes");
        }
    }
}

#[test]
fn passphrase_lengths() {
    let mut blocks = memory(SMALL.blocks().unwrap());
    for len in [0, 7, 129] {
        assert_eq!(passlock::seal(&vec![b'a'; len], &SALT, &NONCE, SMALL, &seed(), &mut blocks), Err(Refused::Passphrase), "{len}");
    }
    for len in [8, 128] {
        let phrase = vec![b'a'; len];
        let file = passlock::seal(&phrase, &SALT, &NONCE, SMALL, &seed(), &mut blocks).unwrap();
        assert_eq!(passlock::open(&file, &phrase, &mut blocks), Ok(seed()), "{len}");
        assert_eq!(passlock::open(&file, &phrase[1..], &mut blocks), if len == 8 { Err(Refused::Passphrase) } else { Err(Refused::Wrong) }, "{len}");
    }
    // Refused before any derivation: the memory is not written.
    let file = sealed(SMALL);
    let mut marked = memory(SMALL.blocks().unwrap());
    marked[0] = written_block();
    assert_eq!(passlock::open(&file, b"short", &mut marked), Err(Refused::Passphrase));
    assert!(!wiped(&marked[..1]));
}

// A block with a byte that is not zero, made by Argon2id itself (no other way to write one).
fn written_block() -> Block {
    let mut blocks = memory(8);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2::Params::new(8, 1, 1, Some(32)).unwrap()).fill_memory(b"password", b"saltsalt", &mut blocks).unwrap();
    blocks[0]
}

#[test]
fn too_little_memory_refused() {
    let file = sealed(SMALL);
    let mut blocks = memory(SMALL.blocks().unwrap() - 1);
    assert_eq!(passlock::open(&file, PASSPHRASE, &mut blocks), Err(Refused::Memory));
    assert_eq!(passlock::seal(PASSPHRASE, &SALT, &NONCE, SMALL, &seed(), &mut blocks), Err(Refused::Memory));
}

#[test]
fn wrong_tries_wait() {
    // None for the first two in a row, then 1 s doubling up to 60 s.
    assert_eq!((0..=11).map(passlock::wait_ms).collect::<Vec<_>>(), [0, 0, 0, 1000, 2000, 4000, 8000, 16000, 32000, 60000, 60000, 60000]);
    assert_eq!(passlock::wait_ms(u32::MAX), 60_000);
    let mut tries = Tries::default();
    assert_eq!((tries.wrong(0), tries.early(0)), (0, None));
    assert_eq!((tries.wrong(10), tries.early(10)), (0, None));
    assert_eq!(tries.wrong(20), 1000);
    assert_eq!((tries.early(20), tries.early(1019), tries.early(1020)), (Some(1000), Some(1), None));
    // A try refused early changes nothing; the next wrong one doubles the wait.
    assert_eq!(tries.wrong(1020), 2000);
    assert_eq!((tries.in_a_row(), tries.early(3019), tries.early(3020)), (4, Some(1), None));
    tries.opened();
    assert_eq!((tries.in_a_row(), tries.early(3020)), (0, None));
    assert_eq!((tries.wrong(3020), tries.early(3020)), (0, None));
    // A clock near its end does not wrap.
    let mut late = Tries::default();
    for _ in 0..3 { late.wrong(u64::MAX - 10); }
    assert_eq!(late.early(u64::MAX - 10), Some(10));
}
