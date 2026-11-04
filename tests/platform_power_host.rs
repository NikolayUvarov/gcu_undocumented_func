//! Host tests of the platform's transmit power policy (libmind/src/platform_power.rs, 213-KRN-0122): every AML table
//! read whole and checked, and the five SAR names searched for; an error never reads as an absence.
#[path = "../libmind/src/platform_power.rs"]
mod platform_power;
use platform_power::*;

// A table with its header: signature, length, revision, checksum, OEM fields, and the body.
fn table(signature: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut t = Vec::new();
    t.extend_from_slice(signature);
    t.extend_from_slice(&((36 + body.len()) as u32).to_le_bytes());
    t.push(2);
    t.push(0);
    t.extend_from_slice(b"MINDOS");
    t.extend_from_slice(b"MINDTEST");
    t.extend_from_slice(&1u32.to_le_bytes());
    t.extend_from_slice(b"MIND");
    t.extend_from_slice(&1u32.to_le_bytes());
    t.extend_from_slice(body);
    let sum = t.iter().fold(0u8, |s, &b| s.wrapping_add(b));
    t[9] = 0u8.wrapping_sub(sum);
    t
}

// AML of a scope with a device and a method, none of the five names: `Scope(\_SB) { Device(PCI0) { Name(_HID, …) } }`.
const PLAIN: &[u8] = b"\x10\x15\\_SB_\x5b\x82\x0fPCI0\x08_HID\x0c\x41\xd0\x0a\x08";

fn word(policy: Policy) -> String {
    let mut out = [0u8; 48];
    let n = policy.word(&mut out);
    String::from_utf8(out[..n].to_vec()).unwrap()
}

#[test]
fn tables_without_the_names_are_a_checked_absence() {
    let dsdt = table(b"DSDT", PLAIN);
    let ssdt = table(b"SSDT", b"\x10\x09\\_PR_\x08CPU0\x01");
    let apic = table(b"APIC", b"WRDS in a table that holds no AML");
    let got = search([dsdt.as_slice(), apic.as_slice(), ssdt.as_slice()]);
    assert_eq!(got, Policy::Absent { tables: 2 });
    assert_eq!(word(got), "platform=absent");
}

#[test]
fn a_name_anywhere_in_aml_is_unknown_until_its_aml_is_read() {
    for name in NAMES {
        // In a device's scope, in another scope, or only referred to: each is unknown, not limits, not absent.
        let mut body = PLAIN.to_vec();
        body.extend_from_slice(b"\x08");
        body.extend_from_slice(name);
        body.extend_from_slice(b"\x12\x03\x01\x0a\x01");
        let ssdt = table(b"SSDT", &body);
        let got = search([table(b"DSDT", PLAIN).as_slice(), ssdt.as_slice()]);
        assert_eq!(got, Policy::Unknown(Why::Named(*b"SSDT", *name)), "{}", String::from_utf8_lossy(&name[..]));
        assert_eq!(word(got), format!("platform=unknown:{}-in-SSDT", String::from_utf8_lossy(&name[..])));
    }
}

#[test]
fn a_table_not_read_whole_is_never_an_absence() {
    let dsdt = table(b"DSDT", PLAIN);
    // Shorter than its stated length.
    let cut = &dsdt[..dsdt.len() - 1];
    assert_eq!(search([cut]), Policy::Unknown(Why::Truncated(*b"DSDT", 0)));
    // Shorter than a header.
    assert_eq!(search([&dsdt[..20]]), Policy::Unknown(Why::Truncated(*b"DSDT", 0)));
    // A length below the header's.
    let mut small = dsdt.clone();
    small[4..8].copy_from_slice(&10u32.to_le_bytes());
    assert_eq!(search([small.as_slice()]), Policy::Unknown(Why::Truncated(*b"DSDT", 0)));
    // A byte changed: its checksum fails, even where a name would hide.
    let mut bad = table(b"SSDT", PLAIN);
    let last = bad.len() - 1;
    bad[last] ^= 0x40;
    assert_eq!(search([dsdt.as_slice(), bad.as_slice()]), Policy::Unknown(Why::Checksum(*b"SSDT", 1)));
    assert_eq!(word(search([dsdt.as_slice(), bad.as_slice()])), "platform=unknown:checksum-SSDT");
    // A table longer than its length is read to its length only.
    let mut long = dsdt.clone();
    long.extend_from_slice(b"WRDS");
    assert_eq!(search([long.as_slice()]), Policy::Absent { tables: 1 });
}

#[test]
fn no_dsdt_or_too_many_tables_is_unknown() {
    assert_eq!(search([table(b"SSDT", PLAIN).as_slice()]), Policy::Unknown(Why::NoDsdt));
    assert_eq!(search(std::iter::empty::<&[u8]>()), Policy::Unknown(Why::NoDsdt));
    assert_eq!(word(Policy::Unknown(Why::NoDsdt)), "platform=unknown:no-dsdt");
    let ssdt = table(b"SSDT", PLAIN);
    let dsdt = table(b"DSDT", PLAIN);
    let many: Vec<&[u8]> = std::iter::once(dsdt.as_slice()).chain(std::iter::repeat(ssdt.as_slice()).take(TABLES_MAX)).collect();
    assert_eq!(search(many), Policy::Unknown(Why::TooMuch));
    assert_eq!(word(Policy::Unknown(Why::TooMuch)), "platform=unknown:too-many-tables");
}

#[test]
fn a_signature_that_is_not_text_is_named_safely() {
    let mut odd = table(b"DSDT", PLAIN);
    odd.truncate(30);
    odd[0] = 0xFF;
    // Not a DSDT or an SSDT by its signature: skipped, and with no DSDT the answer is unknown.
    assert_eq!(search([odd.as_slice()]), Policy::Unknown(Why::NoDsdt));
}

#[test]
fn a_table_that_could_not_be_mapped_is_unknown() {
    let dsdt = table(b"DSDT", PLAIN);
    let mut search = Search::new();
    search.feed(&dsdt);
    search.unreadable();
    assert_eq!(search.finish(), Policy::Unknown(Why::Unreadable(1)));
    assert_eq!(word(Policy::Unknown(Why::Unreadable(1))), "platform=unknown:unreadable-table");
}
