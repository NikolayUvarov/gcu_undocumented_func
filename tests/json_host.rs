//! Host tests of `mind::json::walk` (750-NET-0021), what the parser service's `json` call runs: a document's values in
//! order with their parents, keys and texts, and every bound refused (values, depth, key and string length).
#[allow(dead_code)]
#[path = "../libmind/src/json.rs"]
mod json;

use json::{walk, Kind, Malformed, KEY_MAX, TEXT_MAX, VALUES_MAX, WALK_DEPTH};

/// The values as (kind, parent, key, text).
fn values(text: &[u8]) -> Result<Vec<(Kind, Option<u16>, String, String)>, Malformed> {
    let mut out = Vec::new();
    let count = walk(text, &mut |index, v| {
        assert_eq!(index as usize, out.len());
        out.push((v.kind, v.parent, String::from_utf8(v.key.to_vec()).unwrap(), String::from_utf8(v.text.to_vec()).unwrap()));
        Ok(())
    })?;
    assert_eq!(count as usize, out.len());
    Ok(out)
}

#[test]
fn a_command_in_document_order() {
    let command = br#" {"id": 42, "kind": "exec", "argv": ["ls", "-l\u00e9"], "env": {}, "timeout": -1.5e3, "ok": true, "none": null} "#;
    let v = values(command).unwrap();
    let s = |x: &str| x.to_string();
    assert_eq!(v, vec![
        (Kind::Object, None, s(""), s("")),
        (Kind::Number, Some(0), s("id"), s("42")),
        (Kind::String, Some(0), s("kind"), s("exec")),
        (Kind::Array, Some(0), s("argv"), s("")),
        (Kind::String, Some(3), s(""), s("ls")),
        (Kind::String, Some(3), s(""), s("-l\u{e9}")),
        (Kind::Object, Some(0), s("env"), s("")),
        (Kind::Number, Some(0), s("timeout"), s("-1.5e3")),
        (Kind::True, Some(0), s("ok"), s("")),
        (Kind::Null, Some(0), s("none"), s("")),
    ]);
    // A scalar alone, and nested containers closing in turn.
    assert_eq!(values(b"\"just this\"").unwrap(), vec![(Kind::String, None, s(""), s("just this"))]);
    let nested = values(br#"[[1, [2]], {"a": {"b": []}}, 3]"#).unwrap();
    let parents: Vec<_> = nested.iter().map(|v| v.1).collect();
    assert_eq!(parents, vec![None, Some(0), Some(1), Some(1), Some(3), Some(0), Some(5), Some(6), Some(0)]);
}

#[test]
fn malformed_texts_are_refused() {
    for bad in [&b""[..], b"{", b"[1,]", b"{\"a\" 1}", b"{\"a\":1,}", b"01", b"1.", b"\"\\x\"", b"\"\x01\"", b"[1] 2", b"tru", b"\"\\ud800\"", b"\xff"] {
        assert_eq!(values(bad), Err(Malformed), "{:?}", String::from_utf8_lossy(bad));
    }
}

#[test]
fn every_bound_is_refused_past_it() {
    // Nesting: WALK_DEPTH containers are read, one more is not.
    let deep = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
    assert!(values(deep(WALK_DEPTH).as_bytes()).is_ok());
    assert_eq!(values(deep(WALK_DEPTH + 1).as_bytes()), Err(Malformed));
    // Values: VALUES_MAX in all (the array and its items), not one more.
    let many = |n: usize| format!("[{}]", vec!["0"; n].join(","));
    assert!(values(many(VALUES_MAX - 1).as_bytes()).is_ok());
    assert_eq!(values(many(VALUES_MAX).as_bytes()), Err(Malformed));
    // A key of KEY_MAX bytes, a string of TEXT_MAX bytes once decoded (an escape counts as what it decodes to).
    let key = |n: usize| format!("{{\"{}\": 1}}", "k".repeat(n));
    assert!(values(key(KEY_MAX).as_bytes()).is_ok());
    assert_eq!(values(key(KEY_MAX + 1).as_bytes()), Err(Malformed));
    let text = |n: usize| format!("\"{}\"", "\\u0041".repeat(n));
    assert!(values(text(TEXT_MAX).as_bytes()).is_ok());
    assert_eq!(values(text(TEXT_MAX + 1).as_bytes()), Err(Malformed));
    // The visitor may refuse too, and the walk stops there.
    let mut seen = 0;
    assert_eq!(walk(b"[1,2,3]", &mut |_, _| { seen += 1; if seen == 2 { Err(Malformed) } else { Ok(()) } }), Err(Malformed));
    assert_eq!(seen, 2);
}
