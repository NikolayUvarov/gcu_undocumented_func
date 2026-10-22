//! The agent's wire adapter for contract 1.0 (750-EFF-0006): bodies it writes, answers it reads from `parse`'s values.
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"] mod sha256;
#[path = "../effector/src/json.rs"] mod json;
#[path = "../effector/src/session.rs"] mod session;
#[path = "../effector/src/wire.rs"] mod wire;
#[path = "../libmind/src/json.rs"] mod reader;
use wire::*;

/// What `parse.json` would answer for `text`, as owned strings.
fn parsed(text: &str) -> Vec<(Kind, Option<u16>, String, String)> {
    let mut out = Vec::new();
    reader::walk(text.as_bytes(), &mut |_, v| {
        let kind = match v.kind {
            reader::Kind::Object => Kind::Object, reader::Kind::Array => Kind::Array, reader::Kind::String => Kind::String,
            reader::Kind::Number => Kind::Number, reader::Kind::True => Kind::True, reader::Kind::False => Kind::False, reader::Kind::Null => Kind::Null,
        };
        out.push((kind, v.parent, String::from_utf8(v.key.to_vec()).unwrap(), String::from_utf8(v.text.to_vec()).unwrap()));
        Ok(())
    }).expect("valid JSON");
    out
}

fn with_values<R>(text: &str, f: impl FnOnce(&[Value<'_>]) -> R) -> R {
    let owned = parsed(text);
    let values: Vec<Value<'_>> = owned.iter().map(|(k, p, key, t)| Value { kind: *k, parent: *p, key, text: t }).collect();
    f(&values)
}

#[test]
fn the_heartbeat_is_the_contracts_minimal_one() {
    let mut buf = [0u8; 1024];
    let h = Heartbeat { agent_id: "mind_test_0123456789abcdef", hostname: "mind-test", arch: "x64", agent_version: "0.1.0", stopping: false, agent_time: None };
    let body = core::str::from_utf8(heartbeat(&mut buf, &h).unwrap()).unwrap().to_owned();
    // docs/effector/protocol.md, "A minimal heartbeat is".
    assert_eq!(body, r#"{"agent_id":"mind_test_0123456789abcdef","hostname":"mind-test","os_version":"mindcore","arch":"x64","run_mode":"service","username":"effector","agent_version":"0.1.0","programs":[],"system_state":"running","busy":false,"max_concurrent_commands":1,"occupied_queues":0,"free_slots":1,"config_auto_update_enabled":false,"transport_mode":"https_only","transport":"https"}"#);
    let mut buf = [0u8; 1024];
    let stopping = Heartbeat { stopping: true, agent_time: Some("2026-10-21T07:06:26Z"), ..h };
    let body = core::str::from_utf8(heartbeat(&mut buf, &stopping).unwrap()).unwrap().to_owned();
    assert!(body.contains(r#""system_state":"shutting_down""#) && body.ends_with(r#""agent_time":"2026-10-21T07:06:26Z"}"#), "{body}");
    let mut small = [0u8; 64];
    assert_eq!(heartbeat(&mut small, &h).err(), Some(Error::Write));
}

#[test]
fn the_ack_carries_the_result_as_a_string() {
    let mut buf = [0u8; 512];
    let body = ack(&mut buf, "20261021064900.000000001_000042", session::Final::Denied.result_json()).unwrap();
    let text = core::str::from_utf8(body).unwrap().to_owned();
    assert!(text.starts_with(r#"{"command_id":"20261021064900.000000001_000042","result":"{\"status\":\"denied\""#), "{text}");
    with_values(&text, |v| { assert_eq!(v[2].text, session::Final::Denied.result_json()); });
}

#[test]
fn ok_answers() {
    assert_eq!(with_values(r#"{"status":"ok"}"#, |v| ok_answer(v)), Ok(()));
    assert_eq!(with_values(r#"{"status":"ok","extra":[1,2,{"x":null}]}"#, |v| ok_answer(v)), Ok(()), "unknown fields are skipped");
    assert_eq!(with_values(r#"{"status":"error"}"#, |v| ok_answer(v)), Err(Error::Shape));
    assert_eq!(with_values(r#"{"state":"ok"}"#, |v| ok_answer(v)), Err(Error::Missing));
    assert_eq!(with_values(r#"{"status":1}"#, |v| ok_answer(v)), Err(Error::Type));
    assert_eq!(with_values(r#"{"status":"ok","status":"ok"}"#, |v| ok_answer(v)), Err(Error::Duplicate));
    assert_eq!(with_values(r#"["status","ok"]"#, |v| ok_answer(v)), Err(Error::Shape));
}

#[test]
fn a_poll_with_commands_from_the_real_server_and_the_fixture() {
    // The fixture's command (tests/effector_server.py) and the server's (server/commands.go: times, initiator, reason).
    let text = r#"{"commands":[
        {"id":"cmd-1","agent_id":"mind-test","action":"collect_logs","program_name":"","new_hostport":"{\"path\":\"log:/boot0001.log\"}","status":"pending"},
        {"id":"20261021064900.000000001_000042","operation":"restart_service","agent_id":"mind-test","program_name":"sysmon","action":"restart_service","new_hostport":"{\"timeout_seconds\":180}","status":"sent","created_at":"2026-10-21T06:49:00Z","expires_at":"2026-10-21T07:04:00.123456789+03:00","sent_at":"2026-10-21T06:49:01Z","acked_at":"0001-01-01T00:00:00Z","initiator":"operator","reason":"restart after update","delivery":{"transport":"https","peer":"10.0.2.15:51000"}}
    ],"parallel_limit":1}"#;
    with_values(text, |v| {
        let c = commands(v).unwrap();
        assert_eq!(c.count, 2);
        assert!(!c.activation_required);
        let a = c.list[0].unwrap();
        assert_eq!((a.command.id, a.command.action, a.command.payload, a.command.expires_unix), ("cmd-1", "collect_logs", r#"{"path":"log:/boot0001.log"}"#, None));
        let b = c.list[1].unwrap();
        assert_eq!((b.command.id, b.command.program, b.initiator, b.reason, b.operation), ("20261021064900.000000001_000042", "sysmon", "operator", "restart after update", "restart_service"));
        assert_eq!(b.command.expires_unix, Some(1_792_555_440), "07:04:00+03:00 is 04:04:00Z, fractions dropped");
    });
}

#[test]
fn an_empty_poll_and_activation() {
    with_values(r#"{"commands":null,"parallel_limit":1}"#, |v| assert_eq!(commands(v).unwrap().count, 0));
    with_values(r#"{"commands":[]}"#, |v| assert_eq!(commands(v).unwrap().count, 0));
    with_values(r#"{"commands":[],"activation_required":true}"#, |v| assert!(commands(v).unwrap().activation_required));
    assert_eq!(with_values(r#"{"commands":[],"activation_required":"yes"}"#, |v| commands(v).err()), Some(Error::Type));
}

#[test]
fn malformed_polls_are_refused() {
    let base = r#""agent_id":"a","action":"x","program_name":"","new_hostport":"""#;
    for (text, err) in [
        (r#"{"parallel_limit":1}"#.to_owned(), Error::Missing),
        (r#"{"commands":{}}"#.to_owned(), Error::Type),
        (r#"{"commands":[1]}"#.to_owned(), Error::Shape),
        (format!(r#"{{"commands":[{{{base}}}]}}"#), Error::Missing),
        (format!(r#"{{"commands":[{{"id":7,{base}}}]}}"#), Error::Type),
        (format!(r#"{{"commands":[{{"id":"a","id":"b",{base}}}]}}"#), Error::Duplicate),
        (format!(r#"{{"commands":[{{"id":"a",{base},"expires_at":"tomorrow"}}]}}"#), Error::Type),
        (r#"{"commands":[],"commands":null}"#.to_owned(), Error::Duplicate),
    ] {
        assert_eq!(with_values(&text, |v| commands(v).err()), Some(err), "{text}");
    }
    let many: Vec<String> = (0..=COMMANDS_MAX).map(|i| format!(r#"{{"id":"c{i}",{base}}}"#)).collect();
    let text = format!(r#"{{"commands":[{}]}}"#, many.join(","));
    assert_eq!(with_values(&text, |v| commands(v).err()), Some(Error::Bounds));
}

#[test]
fn the_contracts_value_bound_is_kept() {
    // The root, "status" and "x" are three values: the array fills the answer to exactly the bound.
    let items: Vec<String> = (0..VALUES_MAX - 3).map(|i| i.to_string()).collect();
    let text = format!(r#"{{"status":"ok","x":[{}]}}"#, items.join(","));
    assert_eq!(VALUES_MAX, 1024, "contract 1.0");
    assert_eq!(with_values(&text, |v| { assert_eq!(v.len(), VALUES_MAX); ok_answer(v) }), Ok(()));
    // One more is refused (parse refuses it too, so the values are built by hand).
    let mut over = vec![Value { kind: Kind::Object, parent: None, key: "", text: "" }];
    over.extend((0..VALUES_MAX).map(|_| Value { kind: Kind::Null, parent: Some(0), key: "k", text: "" }));
    assert_eq!(ok_answer(&over), Err(Error::Bounds));
}

#[test]
fn rfc3339_times() {
    assert_eq!(rfc3339("1970-01-01T00:00:00Z"), Some(Some(0)));
    assert_eq!(rfc3339("2026-10-21T07:06:26Z"), Some(Some(1_792_566_386)));
    assert_eq!(rfc3339("2026-10-21T10:06:26+03:00"), Some(Some(1_792_566_386)));
    assert_eq!(rfc3339("2024-02-29T12:00:00.5-01:30"), Some(Some(1_709_213_400)));
    assert_eq!(rfc3339("0001-01-01T00:00:00Z"), Some(None), "Go's zero time is no expiry");
    for bad in ["2026-10-21 07:06:26Z", "2026-13-01T00:00:00Z", "2026-10-21T07:06:26", "2026-10-21T07:06:26+3:00", "1969-12-31T23:59:59Z", "2026-10-21T07:06:26.Z", ""] {
        assert_eq!(rfc3339(bad), None, "{bad}");
    }
    let mut out = [0u8; 20];
    assert_eq!(rfc3339_utc(1_792_566_386, &mut out), "2026-10-21T07:06:26Z");
    assert_eq!(rfc3339_utc(1_709_208_000, &mut out), "2024-02-29T12:00:00Z");
    for t in [0u64, 951_782_400, 4_102_444_800] {
        let s = rfc3339_utc(t, &mut out).to_owned();
        assert_eq!(rfc3339(&s), Some(Some(t)), "{s}");
    }
}

#[test]
fn a_collect_file_payload() {
    let c = with_values(r#"{"kind":"collect_file","path":"log:/boot0001.log","max_bytes":1024,"timeout_seconds":60}"#, |v| collect(v).map(|c| (c.path.to_string(), c.max_bytes)).unwrap());
    assert_eq!(c, ("log:/boot0001.log".to_string(), Some(1024)));
    assert_eq!(with_values(r#"{"path":"data/reports/r.txt"}"#, |v| collect(v).unwrap().max_bytes), None);
    assert_eq!(with_values(r#"{"max_bytes":5}"#, |v| collect(v).err()), Some(Error::Missing));
    assert_eq!(with_values(r#"{"path":"a","max_bytes":"5"}"#, |v| collect(v).err()), Some(Error::Type));
    assert_eq!(with_values(r#"{"path":"","max_bytes":5}"#, |v| collect(v).err()), Some(Error::Bounds));
    assert_eq!(with_values(r#"{"path":"a","path":"b"}"#, |v| collect(v).err()), Some(Error::Duplicate));
}
