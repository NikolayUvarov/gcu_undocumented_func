use crate::{
    model::{Model, ObjectData, Operation},
    research, Result, RuntimeBlock,
};
use nrl_runtime::{Budget, Checkpoint, Control, Kernel, Program, Session};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path, time::Instant};
enum View<'a> {
    Affine(nrl_repr::affine::Affine<'a>, usize, usize, u64),
    Gated(RuntimeBlock<'a>, usize, usize, u64),
}
impl<'a> View<'a> {
    fn new(data: &'a ObjectData) -> Result<Self> {
        let s = data.summary()?;
        Ok(match data {
            ObjectData::Affine(a) => Self::Affine(
                a.view()?,
                s.input_width,
                s.output_width,
                s.numeric_values as u64 + s.output_width as u64,
            ),
            ObjectData::Gated(a) => Self::Gated(
                a.block()?,
                s.input_width,
                s.output_width,
                s.numeric_values as u64
                    + 16 * s.channels.unwrap_or(0) as u64
                    + s.output_width as u64,
            ),
        })
    }
}
impl Kernel for View<'_> {
    fn input_width(&self) -> usize {
        match self {
            Self::Affine(_, n, _, _) | Self::Gated(_, n, _, _) => *n,
        }
    }
    fn output_width(&self) -> usize {
        match self {
            Self::Affine(_, _, n, _) | Self::Gated(_, _, n, _) => *n,
        }
    }
    fn scratch_values(&self) -> usize {
        match self {
            Self::Affine(..) => 0,
            Self::Gated(b, ..) => b.scratch_len(),
        }
    }
    fn work_units(&self) -> u64 {
        match self {
            Self::Affine(_, _, _, n) | Self::Gated(_, _, _, n) => *n,
        }
    }
    fn forward(&self, x: &[f64], y: &mut [f64], scratch: &mut [f64]) -> nrl_core::Result<()> {
        match self {
            Self::Affine(a, ..) => a.forward(x, y),
            Self::Gated(b, ..) => b.forward(x, y, scratch),
        }
    }
}
pub struct Prepared<'a> {
    model: &'a Model,
    views: Vec<View<'a>>,
    indices: BTreeMap<String, usize>,
    model_id: [u8; 32],
}
impl<'a> Prepared<'a> {
    pub fn new(model: &'a Model) -> Result<Self> {
        model.info()?;
        let mut views = Vec::new();
        let mut indices = BTreeMap::new();
        for node in &model.manifest().descriptor.configuration.nodes {
            let name = match &node.op {
                Operation::Affine { object, .. } | Operation::Gated { object, .. } => Some(object),
                _ => None,
            };
            if let Some(name) = name {
                if !indices.contains_key(name) {
                    let data = model
                        .object(name)
                        .ok_or("missing verified runtime object")?;
                    indices.insert(name.clone(), views.len());
                    views.push(View::new(data)?);
                }
            }
        }
        let mut model_id = [0; 32];
        for (i, value) in model_id.iter_mut().enumerate() {
            *value = u8::from_str_radix(&model.model_id()[2 * i..2 * i + 2], 16)
                .map_err(|_| "invalid model ID")?;
        }
        Ok(Self {
            model,
            views,
            indices,
            model_id,
        })
    }
    pub fn model_id(&self) -> [u8; 32] {
        self.model_id
    }
    pub fn operations(&self) -> Result<Vec<nrl_runtime::Operation<'_>>> {
        use nrl_runtime::Operation as Op;
        let graph = &self.model.manifest().descriptor.configuration;
        let mut ids = BTreeMap::from([("input".to_owned(), 0)]);
        let mut result = Vec::new();
        for (node_index, node) in graph.nodes.iter().enumerate() {
            let get = |s: &str| {
                ids.get(s)
                    .copied()
                    .ok_or("runtime graph input is not preceding")
            };
            let op = match &node.op {
                Operation::Affine { input, object } | Operation::Gated { input, object } => {
                    Op::Kernel {
                        input: get(input)?,
                        kernel: &self.views[self.indices[object]],
                    }
                }
                Operation::Silu { input } => Op::Silu { input: get(input)? },
                Operation::Relu { input } => Op::Relu { input: get(input)? },
                Operation::Add { left, right } => Op::Add {
                    left: get(left)?,
                    right: get(right)?,
                },
            };
            result.push(op);
            ids.insert(node.id.clone(), node_index + 1);
        }
        Ok(result)
    }
    pub fn program<'b>(&'b self, nodes: &'b [nrl_runtime::Operation<'b>]) -> Result<Program<'b>> {
        let graph = &self.model.manifest().descriptor.configuration;
        let output = if graph.output == "input" {
            0
        } else {
            graph
                .nodes
                .iter()
                .position(|n| n.id == graph.output)
                .ok_or("runtime graph output missing")?
                + 1
        };
        let p = Program {
            version: nrl_runtime::VERSION,
            model_id: self.model_id,
            input_width: graph.input_width,
            max_batch_rows: self.model.manifest().execution.max_batch_rows,
            max_batch_values: self.model.manifest().execution.max_activation_values,
            nodes,
            output,
        };
        p.layout().map_err(|e| format!("runtime plan: {e:?}"))?;
        Ok(p)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub owner: u64,
    pub max_rows: u64,
    pub max_work_units: u64,
    pub max_workspace_values: usize,
    pub max_output_values: usize,
    pub max_requests: u64,
    pub deadline_duration_ns: u64,
}
impl Policy {
    pub fn load(path: &Path) -> Result<Self> {
        let p: Self = research::parse(&research::read(path)?, "nrl.runtime-policy/0")?;
        p.budget()?;
        Ok(p)
    }
    pub fn budget(&self) -> Result<Budget> {
        if self.schema != "nrl.runtime-policy/0"
            || self.deadline_duration_ns == 0
            || self.deadline_duration_ns > 60_000_000_000
        {
            return Err("unsupported runtime policy/deadline".into());
        }
        let b = Budget {
            max_rows: self.max_rows,
            max_work_units: self.max_work_units,
            max_workspace_values: self.max_workspace_values,
            max_output_values: self.max_output_values,
            max_requests: self.max_requests,
            deadline_tick: self.deadline_duration_ns,
        };
        Session::new(self.owner, [0; 32], b).map_err(|e| format!("runtime budget: {e:?}"))?;
        Ok(b)
    }
}
struct HostClock {
    start: Instant,
    calls: u64,
}
impl Control for HostClock {
    fn checkpoint(&mut self) -> Checkpoint {
        self.calls += 1;
        Checkpoint {
            tick: self.start.elapsed().as_nanos().min(u64::MAX as u128) as u64,
            cancelled: false,
        }
    }
}
pub fn run(model: &Model, input: &[f64], policy: &Policy) -> Result<serde_json::Value> {
    let prepared = Prepared::new(model)?;
    let nodes = prepared.operations()?;
    let program = prepared.program(&nodes)?;
    let layout = program
        .layout()
        .map_err(|e| format!("runtime layout: {e:?}"))?;
    if input.is_empty() || input.len() % program.input_width != 0 {
        return Err("runtime input shape".into());
    }
    let rows = input.len() / program.input_width;
    let budget = policy.budget()?;
    let output_count = rows
        .checked_mul(layout.output_width)
        .ok_or("runtime output limit")?;
    let workspace_count = layout.values_per_row + layout.scratch_values;
    if output_count > budget.max_output_values || workspace_count > budget.max_workspace_values {
        return Err("runtime buffers exceed policy before allocation".into());
    }
    let mut output = vec![0.0; output_count];
    let mut workspace = vec![0.0; workspace_count];
    let mut session = Session::new(policy.owner, prepared.model_id(), budget)
        .map_err(|e| format!("runtime session: {e:?}"))?;
    let mut control = HostClock {
        start: Instant::now(),
        calls: 0,
    };
    session
        .execute(
            policy.owner,
            &program,
            input,
            &mut output,
            &mut workspace,
            &mut control,
        )
        .map_err(|e| format!("runtime execution: {e:?}"))?;
    let usage = session.usage();
    Ok(
        serde_json::json!({"schema":"nrl.runtime-report/0","model_id":model.model_id(),"policy":policy,"output":output,"workspace_values":workspace_count,"workspace_cleared":workspace.iter().all(|v|*v==0.0),"usage":{"rows":usage.rows,"work_units":usage.work_units,"requests":usage.requests},"control_checkpoints":control.calls,"elapsed_seconds":control.start.elapsed().as_secs_f64(),"executor":"nrl-runtime/0; no_std/no_alloc; caller-owned row workspace","work_policy":"stored numeric values + 16 per gated channel + output width; one unit per non-kernel scalar output","owner_scope":"trusted host adapter supplies owner; no capability-authentication claim","native_execution":false}),
    )
}
pub fn run_to_file(
    model: &Model,
    input: &[f64],
    policy: &Policy,
    path: &Path,
) -> Result<serde_json::Value> {
    let report = run(model, input, policy)?;
    research::save(path, &report)?;
    Ok(report)
}
