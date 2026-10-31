//! Differential host checks of live NRL exports against APP's portable nnpkg implementation.
use crate::{model, research, runtime_bridge, storage, Result, MAX_ARTIFACT_BYTES};
use nnpkg::{package, program, service};
use nrl_runtime::{Budget, Checkpoint, Control, Session, Status, Usage};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

const IPC_BYTES: usize = 65536;
const OWNER: u16 = 41;
fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("portable import differential: {message}"))
    }
}
fn native<T, E: std::fmt::Debug>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|e| format!("APP portable path: {e:?}"))
}
#[derive(Default)]
struct Clock(u64);
impl Control for Clock {
    fn checkpoint(&mut self) -> Checkpoint {
        self.0 += 1;
        Checkpoint {
            tick: self.0,
            cancelled: false,
        }
    }
}
fn hex(id: &[u8; 32]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

/// Verify current standalone writer output through the portable parser, views and service, in process on the host.
pub fn check(directory: &Path, input: &[f64]) -> Result<Value> {
    let host = model::load(directory, None)?;
    let prepared = runtime_bridge::Prepared::new(&host)?;
    let nodes = prepared.operations()?;
    let host_program = prepared.program(&nodes)?;
    let host_layout = native(host_program.layout())?;
    require(
        !input.is_empty() && input.len() % host_program.input_width == 0,
        "input shape",
    )?;
    crate::research::finite(input)?;
    let rows = input.len() / host_program.input_width;
    let output_values = rows
        .checked_mul(host_layout.output_width)
        .ok_or("output overflow")?;
    require(
        input.len().checked_mul(8).is_some_and(|b| b <= IPC_BYTES)
            && output_values.checked_mul(8).is_some_and(|b| b <= IPC_BYTES),
        "nn.wit payload limit (65536 bytes)",
    )?;
    require(
        rows <= host_program.max_batch_rows
            && rows
                .checked_mul(host_layout.values_per_row)
                .is_some_and(|n| n <= host_program.max_batch_values),
        "model batch bound",
    )?;
    let expected = host.forward(input)?;
    let manifest =
        storage::read_regular(&directory.join("manifest.json"), model::MAX_MANIFEST_BYTES)?;
    let files: BTreeMap<_, _> = host
        .manifest()
        .objects
        .values()
        .map(|entry| {
            Ok((
                entry.file.clone(),
                storage::read_regular(&directory.join(&entry.file), MAX_ARTIFACT_BYTES)?,
            ))
        })
        .collect::<Result<_>>()?;
    let id = prepared.model_id();
    let direct = native(package::load(&manifest, &id, |name| {
        files.get(name).map(Vec::as_slice)
    }))?;
    let manifest_answer = native(package::parse_manifest(&manifest))?;
    let checked_manifest = native(package::read_manifest(&manifest_answer, &id))?;
    let mut objects = Vec::new();
    let mut object_hashes = BTreeMap::new();
    for (key, entry) in &checked_manifest.objects {
        let raw = files.get(&entry.file).ok_or("missing exporter object")?;
        native(package::vet_file(raw, entry))?;
        let answer = native(package::parse_object(raw))?;
        let checked = native(package::read_object(&answer, raw, entry))?;
        require(
            nnpkg::encode::object_bytes(&checked) == *raw,
            "portable writer differs",
        )?;
        objects.push((key.clone(), checked));
        object_hashes.insert(key.clone(), storage::hash(raw));
    }
    let bytes = manifest.len() + files.values().map(Vec::len).sum::<usize>();
    let parsed = native(package::assemble(checked_manifest, objects, id, bytes))?;
    let description = parsed.describe();
    require(
        direct.describe() == description,
        "direct/worker parser descriptions differ",
    )?;
    let worker = native(package::Package::from_description(&description))?;
    require(
        worker.describe() == description,
        "job description round trip differs",
    )?;
    require(
        worker.id == id && hex(&id) == host.model_id(),
        "portable model identity differs",
    )?;
    let portable = native(program::Prepared::new(&worker))?;
    let portable_nodes = native(portable.operations())?;
    let portable_program = native(portable.program(&portable_nodes))?;
    let layout = native(portable_program.layout())?;
    require(
        layout.values_per_row == host_layout.values_per_row
            && layout.scratch_values == host_layout.scratch_values
            && layout.output_width == host_layout.output_width
            && layout.work_units_per_row == host_layout.work_units_per_row
            && portable_program.output == host_program.output,
        "portable runtime layout differs",
    )?;
    let workspace_values = layout.values_per_row + layout.scratch_values;
    let work = layout
        .work_units_per_row
        .checked_mul(rows as u64)
        .ok_or("work overflow")?;
    let budget = Budget {
        max_rows: rows as u64,
        max_work_units: work,
        max_workspace_values: workspace_values,
        max_output_values: output_values,
        max_requests: 1,
        deadline_tick: u64::MAX,
    };
    let expected_usage = Usage {
        rows: rows as u64,
        work_units: work,
        requests: 1,
    };
    let mut checkpoints = Vec::new();
    for program in [&host_program, &portable_program] {
        let mut session = native(Session::new(OWNER as u64, id, budget))?;
        let mut workspace = vec![123.0; workspace_values];
        let mut output = vec![456.0; output_values];
        let mut control = Clock::default();
        native(session.execute(
            OWNER as u64,
            program,
            input,
            &mut output,
            &mut workspace,
            &mut control,
        ))?;
        crate::close(&output, &expected)?;
        require(
            workspace.iter().all(|x| x.to_bits() == 0),
            "workspace not cleared",
        )?;
        require(
            session.usage() == expected_usage && session.status() == Status::Ready,
            "runtime charges/state differ",
        )?;
        checkpoints.push(control.0);
    }
    require(
        checkpoints[0] == checkpoints[1],
        "portable checkpoint count differs",
    )?;
    let mut service = service::Service::new(128 << 20, 1);
    native(service.insert(OWNER, parsed))?;
    let memory = ((workspace_values + output_values) as u64)
        .checked_mul(8)
        .ok_or("reservation overflow")?;
    let session = native(service.open(
        OWNER,
        &id,
        service::Budget {
            rows: rows as u64,
            work,
            jobs: 1,
            memory,
            deadline_ms: 0,
        },
        0,
    ))?;
    let payload: Vec<_> = input.iter().flat_map(|v| v.to_le_bytes()).collect();
    let job = native(service.submit(OWNER, session, rows as u32, &payload))?;
    require(
        service.take(OWNER, job) == Err(service::Error::Pending),
        "job is not deferred",
    )?;
    let run = service.start().ok_or("no queued portable job")?;
    require(run.job == job, "wrong job started")?;
    let mut clock = Clock::default();
    let outcome = service::execute(&worker, &run, input, &mut clock);
    native(outcome.result)?;
    require(
        outcome.usage == expected_usage,
        "service worker charges differ",
    )?;
    service.finish(job, outcome);
    let output = native(service.take(OWNER, job))?;
    crate::close(&output, &expected)?;
    let usage = native(service.usage(OWNER, session, clock.0))?;
    require(
        (usage.rows, usage.work, usage.jobs, usage.ended) == (rows as u64, work, 1, false),
        "service usage differs",
    )?;
    require(
        service.take(OWNER, job) == Err(service::Error::NotFound),
        "output can be taken twice",
    )?;
    native(service.close(OWNER, session))?;
    native(service.unload(OWNER, &id))?;
    require(
        service.used == 0
            && service.models.is_empty()
            && service.sessions.is_empty()
            && service.jobs.is_empty()
            && service.workers == 0,
        "portable state not released",
    )?;
    Ok(
        json!({"schema":"nrl.portable-import-check/0", "model_id":host.model_id(),
        "execution":"host process using APP nnpkg portable code; no native IPC/worker/VFS claim",
        "native_execution":false, "reference":"NRL host model.forward; not a new independent numerical oracle",
        "input_sha256":storage::hash(&payload), "manifest_sha256":storage::hash(&manifest),
        "object_sha256":object_hashes, "description_sha256":storage::hash(&description),
        "rows":rows, "input_width":host_program.input_width,"output_width":layout.output_width,
        "output":output,"workspace_values":workspace_values,"work_units":work,
        "requests":1,"checkpoints":checkpoints[0],"service_state_released":true,
        "absolute_tolerance":1e-12,"relative_tolerance":1e-10}),
    )
}

pub fn check_to_file(directory: &Path, input_path: &Path, output: &Path) -> Result<Value> {
    let input: Vec<f64> = serde_json::from_value(crate::import_json::parse(
        &storage::read_regular(input_path, MAX_ARTIFACT_BYTES)?,
        MAX_ARTIFACT_BYTES,
    )?)
    .map_err(|e| e.to_string())?;
    let report = check(directory, &input)?;
    research::save(output, &report)?;
    Ok(report)
}
