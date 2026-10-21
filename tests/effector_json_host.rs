//! The Effector agent's bounded JSON writer (750-EFF-0004); every text it writes is read back by `mind::json::walk`.
#![allow(dead_code)]
#[path = "../effector/src/json.rs"] mod writer;
#[path = "../libmind/src/json.rs"] mod json;
use writer::{Error, Writer, DEPTH};

fn walked(text: &[u8]) -> Vec<(json::Kind, Option<u16>, String, String)> {
    let mut out = Vec::new();
    json::walk(text, &mut |_, v| { out.push((v.kind, v.parent, String::from_utf8(v.key.to_vec()).unwrap(), String::from_utf8(v.text.to_vec()).unwrap())); Ok(()) })
        .unwrap_or_else(|_| panic!("not valid JSON: {}", String::from_utf8_lossy(text)));
    out
}

#[test]
fn a_heartbeat_shaped_object() {
    let mut buf = [0u8; 512];
    let mut w = Writer::new(&mut buf);
    w.object().unwrap();
    w.field_str("agent_id", "mind_test_0123").unwrap();
    w.field_str("os_version", "mindcore").unwrap();
    w.field_u64("max_concurrent_commands", 1).unwrap();
    w.field_bool("busy", false).unwrap();
    w.key("programs").unwrap(); w.array().unwrap(); w.end().unwrap();
    w.key("executing_commands").unwrap(); w.array().unwrap();
    w.object().unwrap(); w.field_str("command_id", "a.b_1").unwrap(); w.end().unwrap();
    w.end().unwrap();
    w.end().unwrap();
    let text = w.finish().unwrap();
    assert_eq!(core::str::from_utf8(text).unwrap(), r#"{"agent_id":"mind_test_0123","os_version":"mindcore","max_concurrent_commands":1,"busy":false,"programs":[],"executing_commands":[{"command_id":"a.b_1"}]}"#);
    assert_eq!(walked(text).len(), 9);
}

#[test]
fn strings_are_escaped_and_read_back_unchanged() {
    let tricky = "quote \" backslash \\ newline \n tab \t cr \r bell \u{7} del \u{7f} cyrillic \u{0436} emoji \u{1f600}";
    let mut buf = [0u8; 256];
    let mut w = Writer::new(&mut buf);
    w.string(tricky).unwrap();
    let text = w.finish().unwrap();
    assert!(text.iter().all(|&b| b >= 0x20), "no raw control byte: {:?}", text);
    let v = walked(text);
    assert_eq!(v[0].3, tricky);
}

#[test]
fn a_result_inside_an_ack_is_a_string() {
    let mut inner = [0u8; 256];
    let mut w = Writer::new(&mut inner);
    w.object().unwrap(); w.field_str("status", "denied").unwrap(); w.field_u64("exit_code", 1).unwrap(); w.end().unwrap();
    let inner = core::str::from_utf8(w.finish().unwrap()).unwrap().to_owned();
    let mut outer = [0u8; 256];
    let mut w = Writer::new(&mut outer);
    w.object().unwrap(); w.field_str("command_id", "c-1").unwrap(); w.field_str("result", &inner).unwrap(); w.end().unwrap();
    let text = w.finish().unwrap();
    assert_eq!(core::str::from_utf8(text).unwrap(), r#"{"command_id":"c-1","result":"{\"status\":\"denied\",\"exit_code\":1}"}"#);
    let v = walked(text);
    assert_eq!(v[2].3, inner);
}

#[test]
fn numbers_booleans_and_null() {
    let mut buf = [0u8; 128];
    let mut w = Writer::new(&mut buf);
    w.array().unwrap();
    for n in [0u64, 9, 10, u64::MAX] { w.unsigned(n).unwrap(); }
    for n in [-1i64, i64::MIN, 7] { w.signed(n).unwrap(); }
    w.boolean(true).unwrap(); w.null().unwrap();
    w.end().unwrap();
    assert_eq!(core::str::from_utf8(w.finish().unwrap()).unwrap(), "[0,9,10,18446744073709551615,-1,-9223372036854775808,7,true,null]");
}

#[test]
fn a_body_that_does_not_fit_is_an_error_not_a_cut() {
    let mut buf = [0u8; 16];
    let mut w = Writer::new(&mut buf);
    w.object().unwrap();
    assert_eq!(w.field_str("agent_id", "much too long for sixteen bytes"), Err(Error::Full));
}

#[test]
fn order_and_nesting_are_checked() {
    let mut buf = [0u8; 128];
    let mut w = Writer::new(&mut buf);
    w.object().unwrap();
    assert_eq!(w.string("value without a key"), Err(Error::Order));
    w.key("k").unwrap();
    assert_eq!(w.key("again"), Err(Error::Order));
    assert_eq!(w.end(), Err(Error::Order), "a key without its value");
    w.unsigned(1).unwrap();
    w.end().unwrap();
    assert_eq!(w.unsigned(2), Err(Error::Order), "nothing after the outermost value");

    let mut buf = [0u8; 128];
    let mut w = Writer::new(&mut buf);
    for _ in 0..DEPTH { w.array().unwrap(); }
    assert_eq!(w.array(), Err(Error::Nesting));
    let mut buf = [0u8; 8];
    let mut w = Writer::new(&mut buf);
    w.array().unwrap();
    assert_eq!(w.finish().err(), Some(Error::Order), "an unclosed array is not finished");
    let mut buf = [0u8; 8];
    let w = Writer::new(&mut buf);
    assert_eq!(w.finish().err(), Some(Error::Order), "nothing written is not finished");
}
