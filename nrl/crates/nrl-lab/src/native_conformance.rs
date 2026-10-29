//! Pinned host fixture and oracle for APP's future native conformance harness.
use crate::{
    import_json,
    model::{self, AffineData, Definition, Model, ObjectData},
    research,
    runtime_bridge::Prepared,
    storage, Result, MAX_ARTIFACT_BYTES,
};
use nrl_runtime::{Budget, Checkpoint, Control, Error, Program, Session, Status, Usage};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

pub const MODEL_ID: &str = "1b23aaf4baa75fa146ee8618ea1f7e0e64699c1ea2ee7cf27eeb8bc83f0c1bee";
pub const OBJECT_ENCODING: &str = "nrl.serde-json-pretty-object/0";
const DEFINITION: &[u8] = include_bytes!("../../../tests/fixtures/model-v0/definition.json");
const INPUT: &[u8] = include_bytes!("../../../tests/fixtures/model-v0/input.json");
const ORACLE: &[u8] = include_bytes!("../../../tests/fixtures/model-v0/expected.json");
const LICENSE: &[u8] = include_bytes!("../../../../LICENSE-MIT");
const SOURCE_COMMIT: &str = "11547f61a75d6d1efb5c001439cfccb7cd3d3189";

fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("native conformance: {message}"))
    }
}
fn pin(bytes: &[u8], expected: &str) -> Result<()> {
    require(
        storage::hash(bytes) == expected,
        "source fixture SHA-256 changed",
    )
}
fn pinned() -> Result<(Model, Vec<f64>, Vec<f64>)> {
    pin(
        DEFINITION,
        "fdd5bb55fdaec6c736043f3656658625f5a0a35750a6077ceb27bbf88b1b7879",
    )?;
    pin(
        INPUT,
        "26c04d62e66b2ca926f710fdd41dff8f6de43802c27496593676c4702bc383f6",
    )?;
    pin(
        ORACLE,
        "8414e5703e2ad1dc48fb9aa8beb33f22ce58535190ba2c6fa27906d947220fc3",
    )?;
    let model = Model::from_definition(Definition::parse(DEFINITION)?)?;
    require(model.model_id() == MODEL_ID, "pinned model ID changed")?;
    let input: Vec<f64> = serde_json::from_slice(INPUT).map_err(|e| e.to_string())?;
    let oracle = import_json::parse(ORACLE, MAX_ARTIFACT_BYTES)?;
    let expected =
        serde_json::from_value(oracle["original_output"].clone()).map_err(|e| e.to_string())?;
    Ok((model, input, expected))
}
fn f64le(values: &[f64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn edge_values() -> Vec<f64> {
    vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::MIN_POSITIVE,
        f64::MAX,
        1e20,
        1e-20,
        1e-5,
        1e-6,
        1e16,
        1e17,
        f64::from_bits(0x3ff0000000000001),
    ]
}
fn encoding_vector() -> ObjectData {
    let weights = edge_values();
    ObjectData::Affine(AffineData {
        schema: "nrl.affine/0".into(),
        shape: [weights.len(), 1],
        weights,
        bias: vec![0.0],
    })
}

/// Check the existing writer subset; ordinary model-v0 loading remains more permissive.
pub fn check_object_encoding(bytes: &[u8]) -> Result<()> {
    let data: ObjectData = serde_json::from_value(import_json::parse(bytes, MAX_ARTIFACT_BYTES)?)
        .map_err(|e| e.to_string())?;
    data.summary()?;
    require(
        research::bytes(&data)? == bytes,
        "object is outside the pinned writer encoding",
    )
}

fn files() -> Result<BTreeMap<String, Vec<u8>>> {
    let (model, input, expected) = pinned()?;
    let mut files = BTreeMap::new();
    files.insert(
        "model/manifest.json".into(),
        research::bytes(model.manifest())?,
    );
    for (name, record) in &model.manifest().objects {
        let data = model.object(name).ok_or("missing pinned object")?;
        let bytes = research::bytes(data)?;
        require(
            bytes.len() == record.bytes && storage::hash(&bytes) == record.sha256,
            "object writer changed",
        )?;
        check_object_encoding(&bytes)?;
        files.insert(format!("model/{}", record.file), bytes);
    }
    files.insert("input.f64le".into(), f64le(&input));
    files.insert("expected.f64le".into(), f64le(&expected));
    files.insert(
        "numeric-input.f64le".into(),
        f64le(&[f64::MAX, -f64::MAX, f64::MAX]),
    );
    let long: Vec<_> = (0..256)
        .flat_map(|i| input[3 * (i % 3)..3 * (i % 3) + 3].iter().copied())
        .collect();
    files.insert("long-input.f64le".into(), f64le(&long));
    files.insert("reference.json".into(), ORACLE.to_vec());
    files.insert("LICENSE".into(), LICENSE.to_vec());
    let encoding = research::bytes(&encoding_vector())?;
    pin(
        &encoding,
        "31505851b9d9a72ffbfdd1685c2d8e194245fc974c4d4f887ffedd5500ec04ca",
    )?;
    files.insert("object-encoding.json".into(), encoding);
    let bits: Vec<_> = edge_values()
        .iter()
        .map(|v| format!("{:016x}", v.to_bits()))
        .collect();
    files.insert(
        "encoding-bits.json".into(),
        research::bytes(&json!({
            "schema": "nrl.object-encoding-vector/0", "encoding": OBJECT_ENCODING,
            "weights_f64_bits_hex": bits, "bias_f64_bits_hex": ["0000000000000000"]
        }))?,
    );
    let inventory: BTreeMap<_, _> = files
        .iter()
        .map(|(path, b)| {
            (
                path.clone(),
                json!({"bytes": b.len(), "sha256": storage::hash(b)}),
            )
        })
        .collect();
    let index = json!({
        "schema": "nrl.native-conformance/0", "model_id": MODEL_ID,
        "licence": "MIT", "source_commit": SOURCE_COMMIT,
        "source_files": {
            "nrl/tests/fixtures/model-v0/definition.json": storage::hash(DEFINITION),
            "nrl/tests/fixtures/model-v0/input.json": storage::hash(INPUT),
            "nrl/tests/fixtures/model-v0/expected.json": storage::hash(ORACLE)
        },
        "reference": {"producer": "independent explicit PyTorch", "torch": "2.10.0+cpu",
            "generator": "nrl/tools/generate_model_fixtures.py", "field": "original_output",
            "finite_required": true, "absolute_tolerance": 1e-12, "relative_tolerance": 1e-10,
            "rule": "abs(actual-reference) <= absolute_tolerance + relative_tolerance*abs(reference)"},
        "encoding": {"object": OBJECT_ENCODING, "serde_json": "1.0.151", "zmij": "1.0.23",
            "float_roundtrip": true, "client_payload": "IEEE-754 binary64 little endian, row major"},
        "geometry": {"input_width": 3, "output_width": 2, "rows": 3, "objects": 7,
            "nodes": 15, "numeric_values": 287, "max_batch_rows": 256,
            "max_batch_activation_values": 65536, "long_rows": 256},
        "runtime": {"version": 0, "values_per_row": 47, "scratch_values": 30,
            "workspace_values": 77, "workspace_bytes": 616, "work_units_per_row": 631,
            "three_row_work_units": 1893, "three_row_checkpoints": 50,
            "partial_interrupt_checkpoint": 20, "partial_interrupt_rows": 2,
            "partial_interrupt_work_units": 646, "numeric_failure_work_units": 39},
        "native_execution": false, "files": inventory
    });
    files.insert("bundle.json".into(), research::bytes(&index)?);
    Ok(files)
}

/// Create a new immutable bundle; never overwrite a previous export.
pub fn make(directory: &Path) -> Result<()> {
    let files = files()?;
    fs::create_dir(directory).map_err(|e| e.to_string())?;
    fs::create_dir(directory.join("model")).map_err(|e| e.to_string())?;
    for (path, bytes) in files
        .iter()
        .filter(|(path, _)| path.as_str() != "bundle.json")
    {
        storage::write_new(&directory.join(path), bytes)?;
    }
    storage::write_new(&directory.join("bundle.json"), &files["bundle.json"])
}
fn regular_directory(path: &Path) -> Result<()> {
    require(
        fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_dir(),
        "expected directory, not symlink",
    )
}
fn names(directory: &Path) -> Result<Vec<String>> {
    let mut names: Vec<_> = fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .map(|entry| {
            entry.map_err(|e| e.to_string()).and_then(|entry| {
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "non-UTF8 fixture filename".into())
            })
        })
        .collect::<Result<_>>()?;
    names.sort();
    Ok(names)
}

/// Verify against pinned source bytes, not a caller-controlled manifest's claims.
pub fn check(directory: &Path) -> Result<Value> {
    let expected = files()?;
    regular_directory(directory)?;
    regular_directory(&directory.join("model"))?;
    let mut top: Vec<_> = expected
        .keys()
        .filter(|p| !p.contains('/'))
        .cloned()
        .collect();
    top.push("model".into());
    top.sort();
    require(names(directory)? == top, "unexpected top-level inventory")?;
    let model_names: Vec<_> = expected
        .keys()
        .filter_map(|p| p.strip_prefix("model/").map(str::to_owned))
        .collect();
    require(
        names(&directory.join("model"))? == model_names,
        "unexpected model inventory",
    )?;
    for (path, bytes) in expected {
        require(
            storage::read_regular(&directory.join(&path), bytes.len())? == bytes,
            &format!("pinned bytes differ: {path}"),
        )?;
    }
    let model = model::load(&directory.join("model"), None)?;
    require(model.model_id() == MODEL_ID, "loaded model ID differs")?;
    let (_, input, reference) = pinned()?;
    verify_runtime(&model, &input, &reference)
}

#[derive(Default)]
struct Clock {
    calls: u64,
    cancel_at: Option<u64>,
    reverse_at: Option<u64>,
}
impl Control for Clock {
    fn checkpoint(&mut self) -> Checkpoint {
        self.calls += 1;
        Checkpoint {
            tick: if self.reverse_at == Some(self.calls) {
                0
            } else {
                self.calls
            },
            cancelled: self.cancel_at == Some(self.calls),
        }
    }
}
fn budget() -> Budget {
    Budget {
        max_rows: 256,
        max_work_units: 256 * 631,
        max_workspace_values: 77,
        max_output_values: 512,
        max_requests: 16,
        deadline_tick: u64::MAX,
    }
}
fn usage(rows: u64, work_units: u64, requests: u64) -> Usage {
    Usage {
        rows,
        work_units,
        requests,
    }
}
fn session(program: &Program<'_>, b: Budget) -> Result<Session> {
    Session::new(41, program.model_id, b).map_err(|e| format!("session: {e:?}"))
}
fn state(s: &Session, status: Status, usage: Usage) -> Result<()> {
    require(
        s.status() == status && s.usage() == usage,
        "session status/charges differ",
    )
}
struct Attempt {
    output: Vec<f64>,
    result: nrl_runtime::Result<()>,
}
fn attempt(
    s: &mut Session,
    owner: u64,
    p: &Program<'_>,
    input: &[f64],
    workspace_values: usize,
    control: &mut Clock,
    expected: nrl_runtime::Result<()>,
) -> Result<Attempt> {
    let original: Vec<_> = input.iter().map(|v| v.to_bits()).collect();
    let mut output = vec![123.0; input.len() / p.input_width * 2];
    let mut workspace = vec![456.0; workspace_values];
    let result = s.execute(owner, p, input, &mut output, &mut workspace, control);
    require(
        result == expected,
        &format!("execution: expected {expected:?}, got {result:?}"),
    )?;
    require(
        workspace.iter().all(|v| v.to_bits() == 0),
        "workspace not cleared",
    )?;
    if result.is_err() {
        require(
            output.iter().all(|v| v.to_bits() == 0),
            "failed output not cleared",
        )?;
    }
    require(
        input.iter().map(|v| v.to_bits()).eq(original),
        "input changed",
    )?;
    Ok(Attempt { output, result })
}

fn verify_runtime(model: &Model, input: &[f64], reference: &[f64]) -> Result<Value> {
    let prepared = Prepared::new(model)?;
    let nodes = prepared.operations()?;
    let p = prepared.program(&nodes)?;
    let l = p.layout().map_err(|e| format!("layout: {e:?}"))?;
    require(
        l.values_per_row == 47
            && l.scratch_values == 30
            && l.output_width == 2
            && l.work_units_per_row == 631,
        &format!("pinned layout changed: {l:?}"),
    )?;
    let mut cases = Vec::new();
    let mut record = |name: &str, s: &Session, result: nrl_runtime::Result<()>| {
        let u = s.usage();
        cases.push(json!({"case": name, "result": format!("{result:?}"),
            "status": format!("{:?}", s.status()), "rows": u.rows,
            "work_units": u.work_units, "requests": u.requests}));
    };
    let mut s = session(&p, budget())?;
    let mut clock = Clock::default();
    let a = attempt(&mut s, 41, &p, input, 77, &mut clock, Ok(()))?;
    crate::close(&a.output, reference)?;
    require(clock.calls == 50, "pinned checkpoint count changed")?;
    state(&s, Status::Ready, usage(3, 1893, 1))?;
    record("success", &s, a.result);

    let mut s = session(&p, budget())?;
    for (name, owner, error) in [
        ("wrong-owner", 42, Error::Owner),
        ("nonfinite-input", 41, Error::Numeric),
    ] {
        let x = if owner == 41 {
            vec![f64::NAN, 0.0, 0.0]
        } else {
            input.to_vec()
        };
        let a = attempt(&mut s, owner, &p, &x, 77, &mut Clock::default(), Err(error))?;
        state(&s, Status::Ready, usage(0, 0, 0))?;
        record(name, &s, a.result);
    }
    let mut wrong = session(&p, budget())?;
    let mut other = prepared.program(&nodes)?;
    other.model_id[0] ^= 1;
    let a = attempt(
        &mut wrong,
        41,
        &other,
        input,
        77,
        &mut Clock::default(),
        Err(Error::Model),
    )?;
    state(&wrong, Status::Ready, usage(0, 0, 0))?;
    record("wrong-model", &wrong, a.result);
    crate::close(
        &attempt(&mut s, 41, &p, input, 77, &mut Clock::default(), Ok(()))?.output,
        reference,
    )?;

    let mut b = budget();
    b.max_work_units = 1892;
    let mut s = session(&p, b)?;
    let a = attempt(
        &mut s,
        41,
        &p,
        input,
        77,
        &mut Clock::default(),
        Err(Error::Limit),
    )?;
    state(&s, Status::Ready, usage(0, 0, 0))?;
    record("whole-request-work-preflight", &s, a.result);
    crate::close(
        &attempt(
            &mut s,
            41,
            &p,
            &input[..3],
            77,
            &mut Clock::default(),
            Ok(()),
        )?
        .output,
        &reference[..2],
    )?;

    for (name, b) in [
        (
            "cumulative-rows",
            Budget {
                max_rows: 3,
                ..budget()
            },
        ),
        (
            "cumulative-work",
            Budget {
                max_work_units: 1893,
                ..budget()
            },
        ),
        (
            "request-limit",
            Budget {
                max_requests: 2,
                ..budget()
            },
        ),
    ] {
        let mut s = session(&p, b)?;
        let mut c = Clock::default();
        attempt(&mut s, 41, &p, &input[..6], 77, &mut c, Ok(()))?;
        if name != "request-limit" {
            attempt(&mut s, 41, &p, &input[..6], 77, &mut c, Err(Error::Limit))?;
            state(&s, Status::Ready, usage(2, 1262, 1))?;
        }
        attempt(&mut s, 41, &p, &input[..3], 77, &mut c, Ok(()))?;
        let a = attempt(&mut s, 41, &p, &input[..3], 77, &mut c, Err(Error::Limit))?;
        state(&s, Status::Ready, usage(3, 1893, 2))?;
        record(name, &s, a.result);
    }
    for (name, workspace, x) in [
        ("workspace-short", 76, input.to_vec()),
        ("batch-bound", 77, vec![0.2; 257 * 3]),
    ] {
        let mut s = session(&p, budget())?;
        let a = attempt(
            &mut s,
            41,
            &p,
            &x,
            workspace,
            &mut Clock::default(),
            Err(Error::Limit),
        )?;
        state(&s, Status::Ready, usage(0, 0, 0))?;
        record(name, &s, a.result);
    }
    for (name, error, status) in [
        ("partial-cancel", Error::Cancelled, Status::Cancelled),
        ("partial-deadline", Error::Deadline, Status::Cancelled),
        ("partial-clock-reversal", Error::Clock, Status::Failed),
    ] {
        let mut b = budget();
        let mut c = Clock::default();
        match error {
            Error::Cancelled => c.cancel_at = Some(20),
            Error::Deadline => b.deadline_tick = 20,
            Error::Clock => c.reverse_at = Some(20),
            _ => unreachable!(),
        }
        let mut s = session(&p, b)?;
        let a = attempt(&mut s, 41, &p, input, 77, &mut c, Err(error))?;
        state(&s, status, usage(2, 646, 1))?;
        record(name, &s, a.result);
        attempt(
            &mut s,
            41,
            &p,
            input,
            77,
            &mut Clock::default(),
            Err(Error::Closed),
        )?;
        state(&s, status, usage(2, 646, 1))?;
        let mut fresh = session(&p, budget())?;
        crate::close(
            &attempt(&mut fresh, 41, &p, input, 77, &mut Clock::default(), Ok(()))?.output,
            reference,
        )?;
    }
    let mut s = session(&p, budget())?;
    let a = attempt(
        &mut s,
        41,
        &p,
        &[f64::MAX, -f64::MAX, f64::MAX],
        77,
        &mut Clock::default(),
        Err(Error::Numeric),
    )?;
    state(&s, Status::Failed, usage(1, 39, 1))?;
    record("finite-input-numeric-poison", &s, a.result);
    attempt(
        &mut s,
        41,
        &p,
        input,
        77,
        &mut Clock::default(),
        Err(Error::Closed),
    )?;
    state(&s, Status::Failed, usage(1, 39, 1))?;
    let mut fresh = session(&p, budget())?;
    crate::close(
        &attempt(&mut fresh, 41, &p, input, 77, &mut Clock::default(), Ok(()))?.output,
        reference,
    )?;
    record("fresh-session-after-poison", &fresh, Ok(()));
    let mut s = session(
        &p,
        Budget {
            deadline_tick: 0,
            ..budget()
        },
    )?;
    let a = attempt(
        &mut s,
        41,
        &p,
        input,
        77,
        &mut Clock::default(),
        Err(Error::Deadline),
    )?;
    state(&s, Status::Cancelled, usage(0, 0, 0))?;
    record("zero-deadline-is-immediate", &s, a.result);
    Ok(
        json!({"schema": "nrl.native-conformance-result/0", "model_id": MODEL_ID,
        "native_execution": false, "host_scenarios": cases, "workspace_cleared": true,
        "failed_output_cleared": true, "independent_reference_matched": true}),
    )
}
