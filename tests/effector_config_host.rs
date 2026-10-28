//! The Effector agent's configuration, token and agent ID files (750-EFF-0006).
#![allow(dead_code)]
#[path = "../effector/src/config.rs"] mod config;
use config::*;

const LEAF: &str = "aa00000000000000000000000000000000000000000000000000000000000001";
const SPKI: &str = "BB00000000000000000000000000000000000000000000000000000000000002";

fn kind(text: &str) -> (u16, ErrorKind) { let e = Config::parse(text.as_bytes()).expect_err("refused"); (e.line, e.kind) }

#[test]
fn the_fixtures_configuration() {
    let c = Config::parse(format!("# QEMU's host\neffector-config 1\nserver https://10.0.2.2:8443/effector\npin leaf:{LEAF}\n").as_bytes()).unwrap();
    assert_eq!((c.host(), c.port(), c.base_path(), c.tls_name(), c.address(), c.hostname()), ("10.0.2.2", 8443, "/effector", "10.0.2.2", Some(0x0a00_0202), "mindcore"));
    assert_eq!(c.pins().count(), 1);
}

#[test]
fn a_named_server_with_two_pins() {
    let c = Config::parse(format!("effector-config 1\r\nserver https://hesere.uz/effector/\r\npin leaf:{LEAF}\r\npin spki:{SPKI}\r\naddress 203.0.113.7\r\nhostname mind-lab-1\r\n").as_bytes()).unwrap();
    assert_eq!((c.host(), c.port(), c.base_path(), c.tls_name(), c.address(), c.hostname()), ("hesere.uz", 443, "/effector", "hesere.uz", Some(0xcb00_7107), "mind-lab-1"));
    let kinds: Vec<_> = c.pins().map(|p| p.kind).collect();
    assert_eq!(kinds, [PinKind::Leaf, PinKind::Spki]);
    assert_eq!(c.pins().nth(1).unwrap().digest[0], 0xbb);
    let c = Config::parse(format!("effector-config 1\nserver https://10.0.2.2\npin leaf:{LEAF}\nname localhost\n").as_bytes()).unwrap();
    assert_eq!((c.base_path(), c.tls_name(), c.port()), ("", "localhost", 443));
}

#[test]
fn what_is_refused() {
    for (text, line, k) in [
        ("server https://10.0.2.2/\n".to_owned(), 1, ErrorKind::Version),
        ("effector-config 2\n".to_owned(), 1, ErrorKind::Version),
        ("".to_owned(), 0, ErrorKind::Version),
        ("effector-config 1\n".to_owned(), 0, ErrorKind::Missing),
        (format!("effector-config 1\nserver https://10.0.2.2/\n"), 0, ErrorKind::Missing),
        (format!("effector-config 1\nserver https://hesere.uz/e\npin leaf:{LEAF}\n"), 0, ErrorKind::Missing),
        (format!("effector-config 1\nserver http://10.0.2.2/\npin leaf:{LEAF}\n"), 2, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://user:pw@10.0.2.2/\npin leaf:{LEAF}\n"), 2, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2/e?x=1\npin leaf:{LEAF}\n"), 2, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2/a/../b\npin leaf:{LEAF}\n"), 2, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2:0/\npin leaf:{LEAF}\n"), 2, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2:70000/\npin leaf:{LEAF}\n"), 2, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2/\nserver https://10.0.2.3/\npin leaf:{LEAF}\n"), 3, ErrorKind::Duplicate),
        (format!("effector-config 1\nserver https://10.0.2.2/\npin sha1:{LEAF}\n"), 3, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2/\npin leaf:1234\n"), 3, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2/\npin leaf:{LEAF}\npin leaf:{LEAF}\n"), 4, ErrorKind::Duplicate),
        (format!("effector-config 1\nserver https://10.0.2.2/\npin leaf:{LEAF}\npin spki:{SPKI}\npin spki:{LEAF}\n"), 5, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2/\npin leaf:{LEAF}\naddress 10.0.2.256\n"), 4, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2/\npin leaf:{LEAF}\naddress 010.0.2.2\n"), 4, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2/\npin leaf:{LEAF}\ntoken secret\n"), 4, ErrorKind::Malformed), // only `token none`
        (format!("effector-config 1\nserver https://10.0.2.2/\npin leaf:{LEAF}\nhostname two words\n"), 4, ErrorKind::Malformed),
        (format!("effector-config 1\nserver https://10.0.2.2/\npin leaf:{LEAF}\nname bad_name\n"), 4, ErrorKind::Malformed),
    ] {
        assert_eq!(kind(&text), (line, k), "{text:?}");
    }
    assert_eq!(Config::parse(&vec![b'#'; FILE_MAX + 1]).unwrap_err().kind, ErrorKind::TooLarge);
}

#[test]
fn the_token_file() {
    assert_eq!(token(b"mind-effector-test-token\n"), Some("mind-effector-test-token"));
    assert_eq!(token(b"abc\r\n"), Some("abc"));
    for bad in [&b""[..], b"\n", b"two words\n", b"a\nb\n", &[b'x'; TOKEN_MAX + 1], b"\xff\xfe"] {
        assert_eq!(token(bad), None, "{bad:?}");
    }
}

#[test]
fn the_agent_id() {
    let mut out = [0u8; 21];
    let id = new_agent_id(0x0123_4567_89ab_cdef, &mut out).to_owned();
    assert_eq!(id, "mind_0123456789abcdef");
    assert_eq!(agent_id(format!("{id}\n").as_bytes()), Some(id.as_str()));
    for bad in [&b""[..], b"mind.x", b"has space", &[b'a'; 65]] { assert_eq!(agent_id(bad), None); }
}

#[test]
fn a_server_that_asks_no_token() {
    let base = format!("effector-config 1\nserver https://hesere.uz/effector\npin leaf:{LEAF}\naddress 37.140.216.52\n");
    assert!(!Config::parse(base.as_bytes()).unwrap().no_token(), "a token is needed unless the owner says otherwise");
    assert!(Config::parse(format!("{base}token none\n").as_bytes()).unwrap().no_token());
    // Only `none`: a token never sits in the configuration, which is not secret storage.
    assert_eq!(kind(&format!("{base}token 0123456789abcdef\n")), (5, ErrorKind::Malformed));
    assert_eq!(kind(&format!("{base}token none\ntoken none\n")), (6, ErrorKind::Duplicate));
}

#[test]
fn the_machine_print_rounds_the_memory_to_a_power_of_two() {
    let mut out = [0u8; PRINT_MAX];
    assert_eq!(print("Intel(R) N100", 16_719_953_920, &mut out), "Intel(R) N100, 16 GiB", "the DEXP: 15.6 GiB");
    assert_eq!(print("Intel(R) N100", 16_100_000_000, &mut out), "Intel(R) N100, 16 GiB", "a larger reserve gives the same");
    assert_eq!(print("", 8_100_000_000, &mut out), "8 GiB", "no processor name where user code cannot read it");
    assert_eq!(print("x", 12 << 30, &mut out), "x, 16 GiB");
    assert_eq!(print("x", 0, &mut out), "x, 0 GiB");
}

#[test]
fn the_machine_record_tells_a_copied_identity() {
    let here = "Intel(R) N100, 16 GiB";
    let mut out = [0u8; RECORD_MAX];
    assert_eq!(machine(None, here), Machine::Unrecorded);
    assert_eq!(machine(Some(b"\xff\n"), here), Machine::Unrecorded, "unreadable: the identity is kept");
    assert_eq!(machine(Some(record(here, "", &mut out)), here), Machine::This { foreign: "" });
    let mut out = [0u8; RECORD_MAX];
    let copied = record(here, "mind-macbook-pro", &mut out).to_vec();
    assert_eq!(copied, b"Intel(R) N100, 16 GiB\nmind-macbook-pro\n");
    assert_eq!(machine(Some(&copied), here), Machine::This { foreign: "mind-macbook-pro" });
    assert_eq!(machine(Some(b"Intel(R) Core(TM) i7-4870HQ CPU @ 2.50GHz, 16 GiB\r\n"), here), Machine::Other { print: "Intel(R) Core(TM) i7-4870HQ CPU @ 2.50GHz, 16 GiB" });
    let mut out = [0u8; RECORD_MAX];
    assert_eq!(record(here, "no spaces allowed", &mut out), b"Intel(R) N100, 16 GiB\n", "a malformed hostname is not kept");
}

#[test]
fn a_hostname_of_the_machines_own() {
    let mut out = [0u8; NAME_MAX];
    assert_eq!(own_hostname("Intel(R) N100", &mut out), "mind-n100");
    assert_eq!(own_hostname("Intel(R) Core(TM) i7-4870HQ CPU @ 2.50GHz", &mut out), "mind-core-i7-4870hq");
    assert_eq!(own_hostname("AMD Ryzen 7 5800X 8-Core Processor", &mut out), "mind-ryzen-7-5800x-8-core");
    assert_eq!(own_hostname("", &mut out), "mindcore");
    assert_eq!(own_hostname("Intel(R) CPU", &mut out), "mindcore");
}
