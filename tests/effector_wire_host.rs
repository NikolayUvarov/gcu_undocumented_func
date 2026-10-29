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
    let h = Heartbeat { agent_id: "mind_test_0123456789abcdef", hostname: "mind-test", arch: "x64", agent_version: "0.1.0", stopping: false, agent_time: None, busy: None, load: None };
    let body = core::str::from_utf8(heartbeat(&mut buf, &h).unwrap()).unwrap().to_owned();
    // docs/effector/protocol.md, "A minimal heartbeat is", and 1.2's renewable_execution_timeout (the fixed mode).
    assert_eq!(body, r#"{"agent_id":"mind_test_0123456789abcdef","hostname":"mind-test","os_version":"mindcore","arch":"x64","run_mode":"service","username":"effector","agent_version":"0.1.0","programs":[],"system_state":"running","busy":false,"max_concurrent_commands":1,"occupied_queues":0,"free_slots":1,"config_auto_update_enabled":false,"transport_mode":"https_only","transport":"https","renewable_execution_timeout":false}"#);
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

#[test]
fn an_artifact_goes_inline_as_base64() {
    let mut buf = [0u8; 2048];
    let a = Artifact { name: "screenshot.png", content_type: "image/png", size: 5, sha256: &"0f".repeat(32), truncated: false, picture: Some((1280, 800)) };
    let prefix = artifact_ack(&mut buf, "20261022180615.107104433_000178", &a).unwrap().to_vec();
    let mut data = [0u8; 8];
    let n = base64(b"\x89PNG!", &mut data);
    assert_eq!((&data[..n], base64_len(5)), (&b"iVBORyE="[..], 8));
    let body = [&prefix[..], &data[..n], ARTIFACT_SUFFIX].concat();
    let body = String::from_utf8(body).unwrap();
    // The result is a string holding the JSON, as every other result (contract 1.0); the data is the artifact's last member.
    assert_eq!(body, format!(r#"{{"command_id":"20261022180615.107104433_000178","result":"{{\"status\":\"completed\",\"exit_code\":0,\"stdout\":\"\",\"stderr\":\"\",\"truncated\":false,\"run_as\":\"effector\",\"artifact\":{{\"name\":\"screenshot.png\",\"content_type\":\"image/png\",\"encoding\":\"base64\",\"size\":5,\"sha256\":\"{}\",\"metadata\":{{\"width\":1280,\"height\":800}},\"data\":\"iVBORyE=\"}}}}"}}"#, "0f".repeat(32)));
    parsed(&body);
    // A log: no picture, cut to the policy's bound.
    let log = Artifact { name: "boot.log", content_type: "text/plain; charset=utf-8", truncated: true, picture: None, ..a };
    let prefix = String::from_utf8(artifact_ack(&mut buf, "c1", &log).unwrap().to_vec()).unwrap();
    assert!(prefix.contains(r#"\"truncated\":true,"#) && !prefix.contains("metadata") && prefix.ends_with(r#"\"data\":\""#), "{prefix}");
    let mut small = [0u8; 64];
    assert!(artifact_ack(&mut small, "c1", &a).is_err(), "a body that does not fit is an error, never cut");
}

#[test]
fn base64_of_every_tail() {
    let mut out = [0u8; 16];
    for (input, expected) in [(&b""[..], ""), (b"f", "Zg=="), (b"fo", "Zm8="), (b"foo", "Zm9v"), (b"foob", "Zm9vYg=="), (b"\xff\xfe\xfd\x00", "//79AA==")] {
        let n = base64(input, &mut out);
        assert_eq!((std::str::from_utf8(&out[..n]).unwrap(), base64_len(input.len() as u64)), (expected, n as u64));
    }
}

#[test]
fn a_delivery_and_a_log_source_are_read() {
    let sha = "ab".repeat(32);
    let text = format!(r#"{{"path":"data/inbox/a.bin","allowed_directories":["data/inbox"],"max_bytes":100,"timeout_seconds":60,"artifact_id":"x","size":5,"sha256":"{sha}","overwrite":true}}"#);
    let owned = |v: &[Value<'_>]| deliver(v).map(|d| (d.path.to_owned(), d.size, d.sha256.to_owned(), d.overwrite));
    assert_eq!(with_values(&text, owned), Ok(("data/inbox/a.bin".to_owned(), 5, sha.clone(), true)));
    let plain = format!(r#"{{"path":"data/a","size":0,"sha256":"{sha}"}}"#);
    assert_eq!(with_values(&plain, |v| deliver(v).map(|d| d.overwrite)), Ok(false));
    for (bad, why) in [
        (text.replace(r#""size":5"#, r#""size":500"#), Error::Bounds),
        (text.replace(r#""size":5,"#, ""), Error::Missing),
        (text.replace(r#""size":5"#, r#""size":"5""#), Error::Type),
        (text.replace(&sha, &"AB".repeat(32)), Error::Type),
        (text.replace(r#""overwrite":true"#, r#""overwrite":1"#), Error::Type),
        (text.replace(r#""path":"data/inbox/a.bin""#, r#""path":"""#), Error::Bounds),
    ] {
        assert_eq!(with_values(&bad, |v| deliver(v).err()), Some(why), "{bad}");
    }
    assert_eq!(with_values(r#"{"path":"boot","timeout_seconds":30}"#, |v| log_path(v).map(str::to_owned)), Ok("boot".to_owned()));
    assert_eq!(with_values(r#"{"path":5}"#, |v| log_path(v).err()), Some(Error::Type));
}

#[test]
fn a_held_command_and_the_load_are_in_the_heartbeat() {
    let mut buf = [0u8; 1536];
    let h = Heartbeat { agent_id: "mind-test", hostname: "mind-test", arch: "x64", agent_version: "0.1.0", stopping: false, agent_time: None,
                        busy: Some(Busy { since: "2026-10-23T02:28:56Z", command: "cmd-1", action: "screenshot" }), load: Load::new(150, 8) };
    let body = core::str::from_utf8(heartbeat(&mut buf, &h).unwrap()).unwrap().to_owned();
    // While an answer waits for the server, the one slot is occupied: idle reports would close the command as unconfirmed.
    assert!(body.contains(r#""busy":true,"busy_since":"2026-10-23T02:28:56Z","executing_commands":[{"command_id":"cmd-1","action":"screenshot","started_at":"2026-10-23T02:28:56Z"}],"max_concurrent_commands":1,"occupied_queues":1,"free_slots":0,"#), "{body}");
    // 100 x 1.5 / 8 cores = 18.75%: the server's Linux reading, its default threshold 100.
    assert!(body.ends_with(r#""renewable_execution_timeout":false,"load":{"load_percent":18.750,"load_average":1.500,"cpu_cores":8}}"#), "{body}");
    parsed(&body);
    assert_eq!(Load::new(150, 0), None, "no cores, no load");
    assert_eq!(with_values(r#"{"path":"x","timeout_seconds":600}"#, |v| timeout_seconds(v)), Ok(Some(600)));
    assert_eq!(with_values(r#"{"path":"x"}"#, |v| timeout_seconds(v)), Ok(None));
    assert_eq!(with_values(r#"{"timeout_seconds":0}"#, |v| timeout_seconds(v)), Err(Error::Bounds));
    assert_eq!(with_values(r#"{"timeout_seconds":"5"}"#, |v| timeout_seconds(v)), Err(Error::Type));
}

#[test]
fn an_exec_payload_gives_its_command() {
    let text = r#"{"command":"reboot","run_as":"service","session":"service","timeout_seconds":300}"#;
    assert_eq!(with_values(text, |v| exec_command(v).map(str::to_owned)), Ok("reboot".to_owned()));
    assert_eq!(with_values(r#"{"run_as":"service"}"#, |v| exec_command(v).err()), Some(Error::Missing));
}

#[test]
fn a_completed_result_says_what_was_done() {
    let mut buf = [0u8; 512];
    let json = completed_result(&mut buf, "delivered data/inbox/d.txt 68000 4e0c").unwrap();
    assert_eq!(core::str::from_utf8(json).unwrap(),
        r#"{"status":"completed","exit_code":0,"stdout":"delivered data/inbox/d.txt 68000 4e0c","stderr":"","truncated":false,"run_as":"effector"}"#);
    let json = completed_result(&mut buf, "a \"quoted\" name").unwrap();
    assert!(core::str::from_utf8(json).unwrap().contains(r#""stdout":"a \"quoted\" name""#));
    assert!(completed_result(&mut [0u8; 32], "too long for the buffer").is_err());
    // A command that exits 1 fails, as the other agents report it (750-EFF-0022).
    let json = exec_result(&mut buf, 1, "unhealthy: quarantined: wlan", "exit status 1").unwrap();
    assert_eq!(core::str::from_utf8(json).unwrap(),
        r#"{"status":"failed","exit_code":1,"stdout":"unhealthy: quarantined: wlan","stderr":"","truncated":false,"run_as":"effector","error":"exit status 1"}"#);
}
