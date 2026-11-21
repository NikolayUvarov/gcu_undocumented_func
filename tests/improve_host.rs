//! Host tests of `improve`'s words on MIND Core (improve/app/src/words.rs, 100003-APP-0151): a command line split,
//! quoted words whole; the hub's configuration read and written back; addresses, hex and durations.
#![allow(dead_code)]
extern crate alloc;
#[path = "../improve/app/src/words.rs"]
mod words;

use words::*;

#[test]
fn a_command_line_into_words() {
    assert_eq!(split(r#"new fault "Fan noise" "It is \"loud\" at idle""#).unwrap(), ["new", "fault", "Fan noise", "It is \"loud\" at idle"]);
    assert_eq!(split("  list  ").unwrap(), ["list"]);
    assert_eq!(split(r#"reply 3 "a\\b" --answer 2"#).unwrap(), ["reply", "3", "a\\b", "--answer", "2"]);
    assert_eq!(split(r#""""#).unwrap(), [""]);
    assert_eq!(split("Тише вентилятор").unwrap(), ["Тише", "вентилятор"]);
    assert_eq!(split(r#"new fault 'It said "hi"' 'a\b'"#).unwrap(), ["new", "fault", "It said \"hi\"", "a\\b"], "single quotes keep the text as written");
    assert!(split("'open").is_err());
    assert!(split(r#"new "open"#).is_err(), "an open quote");
    assert!(split(r#""a"b"#).is_err(), "a quote run into a word");
}

#[test]
fn the_hubs_configuration() {
    let pin = "ab".repeat(32);
    let c = config(&format!("# comment\nhub hub.example.org 203.0.113.5 8443\npin {pin}\n")).unwrap();
    assert_eq!((c.name.as_str(), c.address, c.port, c.pins.len()), ("hub.example.org", Some(0xCB00_7105), 8443, 1));
    assert_eq!(config(&config_text(&c)).unwrap(), c, "written back as read");
    let dns = config(&format!("hub hub.example.org dns 443\npin {pin}\npin {}\n", "cd".repeat(32))).unwrap();
    assert_eq!((dns.address, dns.pins.len()), (None, 2));
    assert!(config_text(&dns).contains("hub hub.example.org dns 443"));
    for wrong in [format!("hub h 1.2.3 443\npin {pin}"), format!("hub h 1.2.3.4 0\npin {pin}"), "hub h 1.2.3.4 443\n".into(), format!("hub h_x 1.2.3.4 443\npin {pin}"),
                  format!("hub h 1.2.3.4 443\npin {}", "AB".repeat(32)), format!("hub h 1.2.3.4 443\npin {pin}\npin {pin}\npin {pin}"), "other line".into()] {
        assert!(config(&wrong).is_err(), "{wrong}");
    }
}

#[test]
fn addresses_hex_and_durations() {
    assert_eq!(ipv4("10.0.2.2"), Some(0x0A00_0202));
    assert_eq!(ipv4_text(0x0A00_0202), "10.0.2.2");
    assert_eq!(ipv4("10.0.2"), None);
    assert_eq!(ipv4("10.0.2.256"), None);
    assert_eq!(hex32(&"0f".repeat(32)), Some([0x0f; 32]));
    assert_eq!(hex32(&"0F".repeat(32)), None, "lower case alone");
    assert_eq!(hex32("0f"), None);
    assert_eq!((duration(5), duration(61), duration(7322)), ("5 s".into(), "2 min".into(), "2 h 2 min".into()));
}
