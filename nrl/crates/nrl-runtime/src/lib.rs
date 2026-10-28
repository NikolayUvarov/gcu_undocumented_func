#![no_std]
#![forbid(unsafe_code)]

use nrl_core::finite;
pub const VERSION: u32 = 0;
pub const MAX_NODES: usize = 64;
pub const MAX_WIDTH: usize = 4096;
pub const MAX_WORKSPACE: usize = 1 << 20;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Profile,
    Graph,
    Shape,
    Limit,
    Owner,
    Model,
    Closed,
    Cancelled,
    Deadline,
    Clock,
    Numeric,
}
pub type Result<T> = core::result::Result<T, Error>;
pub trait Kernel {
    fn input_width(&self) -> usize;
    fn output_width(&self) -> usize;
    fn scratch_values(&self) -> usize;
    fn work_units(&self) -> u64;
    fn forward(
        &self,
        input: &[f64],
        output: &mut [f64],
        scratch: &mut [f64],
    ) -> nrl_core::Result<()>;
}
pub enum Operation<'a> {
    Kernel {
        input: usize,
        kernel: &'a dyn Kernel,
    },
    Silu {
        input: usize,
    },
    Relu {
        input: usize,
    },
    Add {
        left: usize,
        right: usize,
    },
}
pub struct Program<'a> {
    pub version: u32,
    pub model_id: [u8; 32],
    pub input_width: usize,
    pub max_batch_rows: usize,
    pub max_batch_values: usize,
    pub nodes: &'a [Operation<'a>],
    pub output: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub values_per_row: usize,
    pub scratch_values: usize,
    pub output_width: usize,
    pub work_units_per_row: u64,
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .filter(|n| *n <= MAX_WORKSPACE)
        .ok_or(Error::Limit)
}
impl Program<'_> {
    pub fn layout(&self) -> Result<Layout> {
        if self.version != VERSION
            || self.nodes.is_empty()
            || self.nodes.len() > MAX_NODES
            || !(1..=MAX_WIDTH).contains(&self.input_width)
            || self.max_batch_rows == 0
            || self.max_batch_rows > MAX_WORKSPACE
            || self.max_batch_values == 0
            || self.max_batch_values > MAX_WORKSPACE
            || self.output > self.nodes.len()
        {
            return Err(Error::Profile);
        }
        let mut widths = [0; MAX_NODES + 1];
        widths[0] = self.input_width;
        let mut values = self.input_width;
        let mut scratch = 0;
        let mut work = 0u64;
        for (i, node) in self.nodes.iter().enumerate() {
            let get = |index: usize| -> Result<usize> {
                if index > i {
                    Err(Error::Graph)
                } else {
                    Ok(widths[index])
                }
            };
            let (width, cost) = match node {
                Operation::Kernel { input, kernel } => {
                    let width = get(*input)?;
                    if width != kernel.input_width()
                        || kernel.output_width() == 0
                        || kernel.output_width() > MAX_WIDTH
                        || kernel.scratch_values() > MAX_WORKSPACE
                        || kernel.work_units() == 0
                    {
                        return Err(Error::Graph);
                    }
                    scratch = scratch.max(kernel.scratch_values());
                    (kernel.output_width(), kernel.work_units())
                }
                Operation::Silu { input } | Operation::Relu { input } => {
                    (get(*input)?, get(*input)? as u64)
                }
                Operation::Add { left, right } => {
                    let width = get(*left)?;
                    if width != get(*right)? {
                        return Err(Error::Graph);
                    }
                    (width, width as u64)
                }
            };
            widths[i + 1] = width;
            values = add(values, width)?;
            work = work.checked_add(cost).ok_or(Error::Limit)?;
        }
        add(values, scratch)?;
        Ok(Layout {
            values_per_row: values,
            scratch_values: scratch,
            output_width: widths[self.output],
            work_units_per_row: work,
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub max_rows: u64,
    pub max_work_units: u64,
    pub max_workspace_values: usize,
    pub max_output_values: usize,
    pub max_requests: u64,
    pub deadline_tick: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ready,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub rows: u64,
    pub work_units: u64,
    pub requests: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct Checkpoint {
    pub tick: u64,
    pub cancelled: bool,
}
pub trait Control {
    fn checkpoint(&mut self) -> Checkpoint;
}
pub struct Session {
    owner: u64,
    model_id: [u8; 32],
    budget: Budget,
    status: Status,
    usage: Usage,
    last_tick: Option<u64>,
}
impl Session {
    pub fn new(owner: u64, model_id: [u8; 32], budget: Budget) -> Result<Self> {
        if budget.max_rows == 0
            || budget.max_work_units == 0
            || budget.max_workspace_values == 0
            || budget.max_workspace_values > MAX_WORKSPACE
            || budget.max_output_values == 0
            || budget.max_output_values > MAX_WORKSPACE
            || budget.max_requests == 0
        {
            return Err(Error::Profile);
        }
        Ok(Self {
            owner,
            model_id,
            budget,
            status: Status::Ready,
            usage: Usage {
                rows: 0,
                work_units: 0,
                requests: 0,
            },
            last_tick: None,
        })
    }
    pub fn usage(&self) -> Usage {
        self.usage
    }
    pub fn status(&self) -> Status {
        self.status
    }
    fn checkpoint(&mut self, control: &mut dyn Control) -> Result<()> {
        let c = control.checkpoint();
        if self.last_tick.is_some_and(|t| c.tick < t) {
            self.status = Status::Failed;
            return Err(Error::Clock);
        }
        self.last_tick = Some(c.tick);
        if c.cancelled {
            self.status = Status::Cancelled;
            return Err(Error::Cancelled);
        }
        if c.tick >= self.budget.deadline_tick {
            self.status = Status::Cancelled;
            return Err(Error::Deadline);
        }
        Ok(())
    }
    pub fn execute(
        &mut self,
        owner: u64,
        program: &Program<'_>,
        input: &[f64],
        output: &mut [f64],
        workspace: &mut [f64],
        control: &mut dyn Control,
    ) -> Result<()> {
        let result = self.execute_inner(owner, program, input, output, workspace, control);
        workspace.fill(0.0);
        if result.is_err() {
            output.fill(0.0);
        }
        result
    }
    fn execute_inner(
        &mut self,
        owner: u64,
        program: &Program<'_>,
        input: &[f64],
        output: &mut [f64],
        workspace: &mut [f64],
        control: &mut dyn Control,
    ) -> Result<()> {
        if owner != self.owner {
            return Err(Error::Owner);
        }
        if program.model_id != self.model_id {
            return Err(Error::Model);
        }
        if self.status != Status::Ready {
            return Err(Error::Closed);
        }
        let layout = program.layout()?;
        if input.is_empty() || input.len() % program.input_width != 0 {
            return Err(Error::Shape);
        }
        let rows = input.len() / program.input_width;
        if rows > program.max_batch_rows
            || rows
                .checked_mul(layout.values_per_row)
                .is_none_or(|n| n > program.max_batch_values)
        {
            return Err(Error::Limit);
        }
        let output_len = rows.checked_mul(layout.output_width).ok_or(Error::Limit)?;
        if output.len() != output_len {
            return Err(Error::Shape);
        }
        let workspace_len = add(layout.values_per_row, layout.scratch_values)?;
        let work = layout
            .work_units_per_row
            .checked_mul(rows as u64)
            .ok_or(Error::Limit)?;
        if workspace.len() != workspace_len
            || workspace_len > self.budget.max_workspace_values
            || output_len > self.budget.max_output_values
            || self
                .usage
                .rows
                .checked_add(rows as u64)
                .is_none_or(|n| n > self.budget.max_rows)
            || self
                .usage
                .work_units
                .checked_add(work)
                .is_none_or(|n| n > self.budget.max_work_units)
            || self.usage.requests >= self.budget.max_requests
        {
            return Err(Error::Limit);
        }
        finite(input).map_err(|_| Error::Numeric)?;
        self.checkpoint(control)?;
        self.usage.requests += 1;
        let (mut widths, mut offsets) = ([0; MAX_NODES + 1], [0; MAX_NODES + 1]);
        widths[0] = program.input_width;
        let mut offset = program.input_width;
        for (i, node) in program.nodes.iter().enumerate() {
            let width = match node {
                Operation::Kernel { kernel, .. } => kernel.output_width(),
                Operation::Silu { input } | Operation::Relu { input } => widths[*input],
                Operation::Add { left, .. } => widths[*left],
            };
            widths[i + 1] = width;
            offsets[i + 1] = offset;
            offset += width;
        }
        let (values, scratch) = workspace.split_at_mut(layout.values_per_row);
        for (row, x) in input.chunks_exact(program.input_width).enumerate() {
            self.checkpoint(control)?;
            self.usage.rows += 1;
            values[..x.len()].copy_from_slice(x);
            for (i, node) in program.nodes.iter().enumerate() {
                self.checkpoint(control)?;
                let start = offsets[i + 1];
                let (previous, rest) = values.split_at_mut(start);
                let out = &mut rest[..widths[i + 1]];
                let get = |j: usize| &previous[offsets[j]..offsets[j] + widths[j]];
                let cost = match node {
                    Operation::Kernel { kernel, .. } => kernel.work_units(),
                    _ => out.len() as u64,
                };
                self.usage.work_units += cost;
                let result = match node {
                    Operation::Kernel { input, kernel } => {
                        kernel.forward(get(*input), out, &mut scratch[..kernel.scratch_values()])
                    }
                    Operation::Silu { input } => {
                        for (y, x) in out.iter_mut().zip(get(*input)) {
                            *y = nrl_repr::silu(*x).0;
                        }
                        Ok(())
                    }
                    Operation::Relu { input } => {
                        for (y, x) in out.iter_mut().zip(get(*input)) {
                            *y = x.max(0.0);
                        }
                        Ok(())
                    }
                    Operation::Add { left, right } => {
                        for ((y, a), b) in out.iter_mut().zip(get(*left)).zip(get(*right)) {
                            *y = a + b;
                        }
                        Ok(())
                    }
                };
                if result.is_err() || finite(out).is_err() {
                    self.status = Status::Failed;
                    return Err(Error::Numeric);
                }
            }
            let start = offsets[program.output];
            output[row * layout.output_width..(row + 1) * layout.output_width]
                .copy_from_slice(&values[start..start + layout.output_width]);
        }
        self.checkpoint(control)?;
        Ok(())
    }
}
