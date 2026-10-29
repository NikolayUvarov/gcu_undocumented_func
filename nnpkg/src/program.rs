//! A checked package as `nrl-runtime`'s program (262-APP-0112): the objects' views as kernels, with the work policy of
//! `NRL`'s adapter (an affine object: its values and its output width; a gated one: its values, 16 a channel and its
//! output width).
use crate::check::{self, RuntimeBlock};
use crate::model::{ObjectData, Operation};
use crate::package::Package;
use crate::Refusal;
use alloc::vec::Vec;
use nrl_repr::affine::Affine;
use nrl_runtime::{Kernel, Program};

pub enum View<'a> { Affine(Affine<'a>, usize, usize, u64), Gated(RuntimeBlock<'a>, usize, usize, u64) }

impl<'a> View<'a> {
    pub fn new(data: &'a ObjectData) -> Result<Self, Refusal> {
        let s = check::summary(data)?;
        Ok(match data {
            ObjectData::Affine(a) => Self::Affine(check::affine(a)?, s.input_width, s.output_width, s.numeric_values as u64 + s.output_width as u64),
            ObjectData::Gated(a) => Self::Gated(check::block(a)?, s.input_width, s.output_width, s.numeric_values as u64 + 16 * s.channels.unwrap_or(0) as u64 + s.output_width as u64),
        })
    }
}

impl Kernel for View<'_> {
    fn input_width(&self) -> usize { match self { Self::Affine(_, n, _, _) | Self::Gated(_, n, _, _) => *n } }
    fn output_width(&self) -> usize { match self { Self::Affine(_, _, n, _) | Self::Gated(_, _, n, _) => *n } }
    fn scratch_values(&self) -> usize { match self { Self::Affine(..) => 0, Self::Gated(b, ..) => b.scratch_len() } }
    fn work_units(&self) -> u64 { match self { Self::Affine(_, _, _, n) | Self::Gated(_, _, _, n) => *n } }
    fn forward(&self, x: &[f64], y: &mut [f64], scratch: &mut [f64]) -> nrl_core::Result<()> {
        match self { Self::Affine(a, ..) => a.forward(x, y), Self::Gated(b, ..) => b.forward(x, y, scratch) }
    }
}

/// The views of a package's objects, in the order the graph first names them.
pub struct Prepared<'a> { package: &'a Package, views: Vec<View<'a>>, indices: Vec<(&'a str, usize)> }

impl<'a> Prepared<'a> {
    pub fn new(package: &'a Package) -> Result<Self, Refusal> {
        let mut views = Vec::new();
        let mut indices: Vec<(&str, usize)> = Vec::new();
        for node in &package.manifest.descriptor.configuration.nodes {
            let Some(name) = node.op.object() else { continue };
            if indices.iter().any(|(n, _)| *n == name) { continue; }
            let data = package.object(name).ok_or(Refusal::Malformed("A MODEL OBJECT MISSING"))?;
            indices.push((name, views.len()));
            views.push(View::new(data)?);
        }
        Ok(Self { package, views, indices })
    }

    /// The graph's nodes as `nrl-runtime`'s operations.
    pub fn operations(&self) -> Result<Vec<nrl_runtime::Operation<'_>>, Refusal> {
        use nrl_runtime::Operation as Op;
        let graph = &self.package.manifest.descriptor.configuration;
        let mut names: Vec<&str> = alloc::vec!["input"];
        let at = |names: &Vec<&str>, s: &str| names.iter().position(|n| *n == s).ok_or(Refusal::Malformed("A GRAPH REFERENCE MISSING, FORWARD OR CYCLIC"));
        let view = |object: &str| self.indices.iter().find(|(n, _)| *n == object).map(|(_, i)| &self.views[*i] as &dyn Kernel).ok_or(Refusal::Malformed("A MODEL OBJECT MISSING"));
        let mut result = Vec::new();
        for node in &graph.nodes {
            result.push(match &node.op {
                Operation::Affine { input, object } | Operation::Gated { input, object } => Op::Kernel { input: at(&names, input)?, kernel: view(object)? },
                Operation::Silu { input } => Op::Silu { input: at(&names, input)? },
                Operation::Relu { input } => Op::Relu { input: at(&names, input)? },
                Operation::Add { left, right } => Op::Add { left: at(&names, left)?, right: at(&names, right)? },
            });
            names.push(&node.id);
        }
        Ok(result)
    }

    /// The program over `nodes`, its layout checked.
    pub fn program<'b>(&'b self, nodes: &'b [nrl_runtime::Operation<'b>]) -> Result<Program<'b>, Refusal> {
        let p = Program {
            version: nrl_runtime::VERSION,
            model_id: self.package.id,
            input_width: self.package.manifest.descriptor.configuration.input_width,
            max_batch_rows: self.package.manifest.execution.max_batch_rows,
            max_batch_values: self.package.manifest.execution.max_activation_values,
            nodes,
            output: self.package.output,
        };
        p.layout().map_err(|_| Refusal::Unsupported("THE RUNTIME'S PROFILE REFUSES THE GRAPH"))?;
        Ok(p)
    }
}
