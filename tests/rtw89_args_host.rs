#[path = "../rtw89_wifi/src/args.rs"] mod args;
use args::{parse, Platform};

#[test]
fn the_index_alone_still_starts_with_an_unknown_platform() {
    assert_eq!(parse("3"), Some((3, Platform::Unknown(None))));
    assert_eq!(parse(" 12 "), Some((12, Platform::Unknown(None))));
}

#[test]
fn init_words_after_the_index_are_read() {
    assert_eq!(parse("3 platform=absent"), Some((3, Platform::Absent)));
    assert_eq!(parse("3 pci=0000:03:00.0 platform=absent"), Some((3, Platform::Absent)));
    assert_eq!(parse("3 platform=unknown:ssdt-checksum"), Some((3, Platform::Unknown(Some("ssdt-checksum")))));
    assert_eq!(parse("3 platform=limits:30,24,-2,0"), Some((3, Platform::Limits { per_ntx: [30, 24], per_path: [-2, 0] })));
}

#[test]
fn a_malformed_or_repeated_word_is_unknown_never_absent() {
    for word in ["platform=", "platform=ABSENT", "platform=absent2", "platform=limits:30,24,0", "platform=limits:30,24,0,0,0",
        "platform=limits:64,0,0,0", "platform=limits:a,b,c,d", "platform=unknown:"] {
        assert_eq!(parse(&format!("3 {word}")), Some((3, Platform::Unknown(None))), "{word}");
    }
    let long = format!("3 platform=unknown:{}", "x".repeat(49));
    assert_eq!(parse(&long), Some((3, Platform::Unknown(None))));
    assert_eq!(parse("3 platform=absent platform=absent"), Some((3, Platform::Unknown(None))));
    assert_eq!(parse("3 platform=unknown:a platform=absent"), Some((3, Platform::Unknown(None))));
}

#[test]
fn no_index_means_no_start() {
    for args in ["", "platform=absent", "x 3", "-1"] { assert_eq!(parse(args), None, "{args}"); }
}
