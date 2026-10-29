// nnpkg against NRL's pinned conformance bundle (900-NRL-0025) and nrl-lab's writer (262-APP-0112).
use nnpkg::{encode, json, model, package, program::Prepared, sha256, Refusal};
use nrl_runtime::{Budget, Checkpoint, Control, Error, Program, Session, Status, Usage};
use std::path::PathBuf;

const MODEL_ID: &str = "1b23aaf4baa75fa146ee8618ea1f7e0e64699c1ea2ee7cf27eeb8bc83f0c1bee";

// The bundle's directory: NRL's in the tree, or NNPKG_FIXTURE.
fn fixture() -> PathBuf {
    std::env::var_os("NNPKG_FIXTURE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../nrl/tests/fixtures/native-conformance-v0"))
}
fn read(name: &str) -> Vec<u8> { std::fs::read(fixture().join(name)).unwrap_or_else(|e| panic!("{name}: {e}")) }
fn f64le(bytes: &[u8]) -> Vec<f64> { bytes.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect() }
fn id() -> [u8; 32] { let mut id = [0; 32]; for i in 0..32 { id[i] = u8::from_str_radix(&MODEL_ID[2 * i..2 * i + 2], 16).unwrap(); } id }
fn hex(d: &[u8; 32]) -> String { d.iter().map(|b| format!("{b:02x}")).collect() }

struct Files { manifest: Vec<u8>, objects: Vec<(String, Vec<u8>)> }
fn files() -> Files {
    let objects = (0..7).map(|i| { let name = package::object_file(i); let bytes = read(&format!("model/{name}")); (name, bytes) }).collect();
    Files { manifest: read("model/manifest.json"), objects }
}
fn load(f: &Files, id: &[u8; 32]) -> Result<package::Package, Refusal> {
    package::load(&f.manifest, id, |name| f.objects.iter().find(|(n, _)| n == name).map(|(_, b)| b.as_slice()))
}

#[test]
fn the_encoding_vector_comes_back_to_its_bits_and_bytes() {
    let bytes = read("object-encoding.json");
    assert_eq!(hex(&sha256::digest(&bytes)), "31505851b9d9a72ffbfdd1685c2d8e194245fc974c4d4f887ffedd5500ec04ca");
    let data = model::object(&json::parse(&bytes).unwrap()).unwrap();
    let model::ObjectData::Affine(a) = &data else { panic!("not affine") };
    let bits: serde_json::Value = serde_json::from_slice(&read("encoding-bits.json")).unwrap();
    let want: Vec<u64> = bits["weights_f64_bits_hex"].as_array().unwrap().iter().map(|h| u64::from_str_radix(h.as_str().unwrap(), 16).unwrap()).collect();
    assert_eq!(a.weights.iter().map(|w| w.to_bits()).collect::<Vec<_>>(), want, "subnormals and -0.0 read back to their bits");
    assert_eq!(a.bias[0].to_bits(), 0);
    assert_eq!(encode::object_bytes(&data), bytes, "the profile's bytes");
}

#[test]
fn floats_and_strings_are_written_as_serde_json_writes_them() {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for i in 0..200_000u64 {
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        let v = match i % 3 { 0 => f64::from_bits(x), 1 => f64::from_bits(x & 0x000F_FFFF_FFFF_FFFF), _ => (x % 2_000_000) as f64 / [1.0, 10.0, 1e3, 7.0, 1e6][(x >> 60) as usize % 5] };
        if !v.is_finite() { continue; }
        let mut out = Vec::new();
        encode::pretty(&mut out, &encode::Json::Float(v), 0);
        assert_eq!(String::from_utf8(out).unwrap(), serde_json::to_string(&v).unwrap(), "{:016x}", v.to_bits());
    }
    for s in ["", "plain", "quote \" and \\ slash /", "\u{0}\u{1}\u{8}\t\n\u{b}\u{c}\r\u{1f}\u{7f}", "Кириллица ✓ 𝄞"] {
        let mut out = Vec::new();
        encode::pretty(&mut out, &encode::Json::Str(s), 0);
        assert_eq!(String::from_utf8(out).unwrap(), serde_json::to_string(s).unwrap());
    }
}

#[test]
fn every_object_file_is_in_the_writer_profile() {
    let f = files();
    let manifest = package::manifest(&f.manifest, &id()).unwrap();
    for ((key, record), (name, bytes)) in manifest.objects.iter().zip(&f.objects) {
        assert_eq!(&record.file, name, "{key}");
        let (data, summary) = package::object(bytes, record).unwrap_or_else(|e| panic!("{key}: {e:?}"));
        assert_eq!(summary, record.parameters);
        assert_eq!(&encode::object_bytes(&data), bytes);
    }
}

#[test]
fn the_pinned_model_loads_with_its_id_and_layout() {
    let p = load(&files(), &id()).unwrap();
    assert_eq!(hex(&p.id), MODEL_ID);
    assert_eq!(hex(&package::model_id(&p.manifest)), MODEL_ID);
    assert_eq!((p.input_width, p.output_width, p.workspace_values, p.work_per_row), (3, 2, 77, 631));
    assert_eq!(p.objects.len(), 7);
    assert_eq!(p.bytes, read("model/manifest.json").len() + files().objects.iter().map(|(_, b)| b.len()).sum::<usize>());
}

// NRL's fake clock: a checkpoint's tick is its count; it cancels or goes back at a given one.
#[derive(Default)]
struct Clock { calls: u64, cancel_at: Option<u64>, reverse_at: Option<u64> }
impl Control for Clock {
    fn checkpoint(&mut self) -> Checkpoint {
        self.calls += 1;
        Checkpoint { tick: if self.reverse_at == Some(self.calls) { 0 } else { self.calls }, cancelled: self.cancel_at == Some(self.calls) }
    }
}
fn budget() -> Budget { Budget { max_rows: 256, max_work_units: 256 * 631, max_workspace_values: 77, max_output_values: 512, max_requests: 16, deadline_tick: u64::MAX } }
fn usage(rows: u64, work_units: u64, requests: u64) -> Usage { Usage { rows, work_units, requests } }
fn close(actual: &[f64], reference: &[f64]) {
    assert_eq!(actual.len(), reference.len());
    for (a, r) in actual.iter().zip(reference) { assert!(a.is_finite() && (a - r).abs() <= 1e-12 + 1e-10 * r.abs(), "{a} against {r}"); }
}
fn attempt(s: &mut Session, owner: u64, p: &Program, input: &[f64], workspace: usize, clock: &mut Clock, expected: nrl_runtime::Result<()>) -> Vec<f64> {
    let mut output = vec![123.0; input.len() / 3 * 2];
    let mut work = vec![456.0; workspace];
    let result = s.execute(owner, p, input, &mut output, &mut work, clock);
    assert_eq!(result, expected);
    assert!(work.iter().all(|v| v.to_bits() == 0), "the workspace cleared");
    if result.is_err() { assert!(output.iter().all(|v| v.to_bits() == 0), "a failed output cleared"); }
    output
}

#[test]
fn the_runtime_scenarios_charge_as_nrl_pinned_them() {
    let package = load(&files(), &id()).unwrap();
    let prepared = Prepared::new(&package).unwrap();
    let nodes = prepared.operations().unwrap();
    let p = prepared.program(&nodes).unwrap();
    let input = f64le(&read("input.f64le"));
    let reference = f64le(&read("expected.f64le"));
    let session = |b: Budget| Session::new(41, p.model_id, b).unwrap();
    let state = |s: &Session, status: Status, u: Usage| { assert_eq!((s.status(), s.usage()), (status, u)); };

    let mut s = session(budget());
    let mut clock = Clock::default();
    close(&attempt(&mut s, 41, &p, &input, 77, &mut clock, Ok(())), &reference);
    assert_eq!(clock.calls, 50);
    state(&s, Status::Ready, usage(3, 1893, 1));

    let mut s = session(budget());
    attempt(&mut s, 42, &p, &input, 77, &mut Clock::default(), Err(Error::Owner));
    attempt(&mut s, 41, &p, &[f64::NAN, 0.0, 0.0], 77, &mut Clock::default(), Err(Error::Numeric));
    state(&s, Status::Ready, usage(0, 0, 0));

    let mut s = session(Budget { max_work_units: 1892, ..budget() });
    attempt(&mut s, 41, &p, &input, 77, &mut Clock::default(), Err(Error::Limit));
    state(&s, Status::Ready, usage(0, 0, 0));
    close(&attempt(&mut s, 41, &p, &input[..3], 77, &mut Clock::default(), Ok(())), &reference[..2]);

    for (name, b) in [("rows", Budget { max_rows: 3, ..budget() }), ("work", Budget { max_work_units: 1893, ..budget() }), ("requests", Budget { max_requests: 2, ..budget() })] {
        let mut s = session(b);
        let mut c = Clock::default();
        attempt(&mut s, 41, &p, &input[..6], 77, &mut c, Ok(()));
        if name != "requests" { attempt(&mut s, 41, &p, &input[..6], 77, &mut c, Err(Error::Limit)); state(&s, Status::Ready, usage(2, 1262, 1)); }
        attempt(&mut s, 41, &p, &input[..3], 77, &mut c, Ok(()));
        attempt(&mut s, 41, &p, &input[..3], 77, &mut c, Err(Error::Limit));
        state(&s, Status::Ready, usage(3, 1893, 2));
    }
    for (workspace, x) in [(76, input.clone()), (77, vec![0.2; 257 * 3])] {
        let mut s = session(budget());
        attempt(&mut s, 41, &p, &x, workspace, &mut Clock::default(), Err(Error::Limit));
        state(&s, Status::Ready, usage(0, 0, 0));
    }
    for (error, status) in [(Error::Cancelled, Status::Cancelled), (Error::Deadline, Status::Cancelled), (Error::Clock, Status::Failed)] {
        let mut b = budget();
        let mut c = Clock::default();
        match error { Error::Cancelled => c.cancel_at = Some(20), Error::Deadline => b.deadline_tick = 20, _ => c.reverse_at = Some(20) }
        let mut s = session(b);
        attempt(&mut s, 41, &p, &input, 77, &mut c, Err(error));
        state(&s, status, usage(2, 646, 1));
        attempt(&mut s, 41, &p, &input, 77, &mut Clock::default(), Err(Error::Closed));
        state(&s, status, usage(2, 646, 1));
    }
    let mut s = session(budget());
    attempt(&mut s, 41, &p, &f64le(&read("numeric-input.f64le")), 77, &mut Clock::default(), Err(Error::Numeric));
    state(&s, Status::Failed, usage(1, 39, 1));
    let mut fresh = session(budget());
    close(&attempt(&mut fresh, 41, &p, &input, 77, &mut Clock::default(), Ok(())), &reference);
    state(&fresh, Status::Ready, usage(3, 1893, 1));

    let mut s = session(Budget { deadline_tick: 0, ..budget() });
    attempt(&mut s, 41, &p, &input, 77, &mut Clock::default(), Err(Error::Deadline));
    state(&s, Status::Cancelled, usage(0, 0, 0));

    let long = f64le(&read("long-input.f64le"));
    assert_eq!(long.len(), 256 * 3);
    let mut s = session(Budget { max_output_values: 512, ..budget() });
    attempt(&mut s, 41, &p, &long, 77, &mut Clock::default(), Ok(()));
    state(&s, Status::Ready, usage(256, 256 * 631, 1));
}

// The manifest with `edit` made to its JSON, and its new ID.
fn edited(f: &Files, edit: impl FnOnce(&mut serde_json::Value)) -> (Vec<u8>, [u8; 32]) {
    let mut value: serde_json::Value = serde_json::from_slice(&f.manifest).unwrap();
    edit(&mut value);
    let bytes = serde_json::to_vec_pretty(&value).unwrap();
    let m = model::manifest(&json::parse(&bytes).unwrap()).unwrap();
    (bytes, package::model_id(&m))
}

#[test]
fn a_package_is_refused_as_the_design_says() {
    let f = files();
    // A wrong ID.
    let mut wrong = id();
    wrong[0] ^= 1;
    assert!(matches!(load(&f, &wrong), Err(Refusal::Digest(_))));
    // One changed byte of an object file.
    let mut changed = Files { manifest: f.manifest.clone(), objects: f.objects.clone() };
    let at = changed.objects[5].1.iter().position(|&b| b == b'3').unwrap();
    changed.objects[5].1[at] = b'4';
    assert!(matches!(load(&changed, &id()), Err(Refusal::Digest(_))));
    // The same values in another valid layout, its hash in a manifest of its own: not in the writer profile.
    let compact: serde_json::Value = serde_json::from_slice(&f.objects[5].1).unwrap();
    let compact = serde_json::to_vec(&compact).unwrap();
    let digest = hex(&sha256::digest(&compact));
    let (manifest, new_id) = edited(&f, |m| { m["objects"]["embed"]["sha256"] = digest.clone().into(); m["objects"]["embed"]["bytes"] = compact.len().into(); });
    let mut other = Files { manifest, objects: f.objects.clone() };
    other.objects[5].1 = compact;
    assert_eq!(load(&other, &new_id).err(), Some(Refusal::Malformed("AN OBJECT FILE NOT IN THE WRITER PROFILE nrl.serde-json-pretty-object/0")));
    // An overlay.
    let (manifest, new_id) = edited(&f, |m| { m["kind"] = "overlay".into(); m["base"] = serde_json::json!({"model_id": MODEL_ID}); });
    assert!(matches!(load(&Files { manifest, objects: f.objects.clone() }, &new_id), Err(Refusal::Unsupported(_))));
    // An unknown field, a duplicate key.
    let mut unknown: serde_json::Value = serde_json::from_slice(&f.manifest).unwrap();
    unknown["execution"]["threads"] = 4.into();
    assert_eq!(package::manifest(&serde_json::to_vec_pretty(&unknown).unwrap(), &id()).err(), Some(Refusal::Malformed("AN UNKNOWN FIELD")));
    let mut duplicate = f.manifest.clone();
    let at = duplicate.windows(9).position(|w| w == b"\"history\"").unwrap();
    duplicate.splice(at..at, b"\"history\": [],\n  ".iter().copied());
    assert_eq!(package::manifest(&duplicate, &id()).err(), Some(Refusal::Malformed("A DUPLICATE KEY")));
    // A missing object file.
    let missing = Files { manifest: f.manifest.clone(), objects: f.objects[..6].to_vec() };
    assert!(matches!(load(&missing, &id()), Err(Refusal::NotFound(_))));
    // The manifest's own layout is free: compact, it keeps its ID.
    let compact: serde_json::Value = serde_json::from_slice(&f.manifest).unwrap();
    assert!(load(&Files { manifest: serde_json::to_vec(&compact).unwrap(), objects: f.objects.clone() }, &id()).is_ok());
}

#[test]
fn the_reader_is_as_strict_as_serde_json() {
    for good in ["{}", "[]", " [1, -0, 0.5, 1e+20, -1E-6] ", "\"\\u00e9\\ud834\\udd1e\\/\"", "{\"a\": {\"b\": [null, true, false]}}"] {
        assert!(json::parse(good.as_bytes()).is_ok(), "{good}");
        assert!(serde_json::from_str::<serde_json::Value>(good).is_ok(), "{good}");
    }
    for bad in ["", "[1,]", "{\"a\":1,}", "01", "1.", ".5", "+1", "1e", "[1 2]", "\"\\ud834\"", "\"\\udd1e\"", "\"a\u{1}\"", "\"\\x\"", "{\"a\":1} x", "nul", "[\"\\u12\"]", "\u{feff}{}"] {
        assert!(json::parse(bad.as_bytes()).is_err(), "{bad:?}");
        assert!(serde_json::from_str::<serde_json::Value>(bad).is_err(), "{bad:?}");
    }
    assert_eq!(json::parse(b"{\"a\":1,\"a\":2}"), Err("A DUPLICATE KEY"));
    let deep = "[".repeat(40) + &"]".repeat(40);
    assert_eq!(json::parse(deep.as_bytes()), Err("NESTED TOO DEEP"));
    assert!(json::parse(&[b'"', 0xff, b'"']).is_err());
}
