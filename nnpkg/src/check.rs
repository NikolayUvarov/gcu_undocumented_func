//! The checks of a package's description, as `nrl-lab`'s model reader makes them (262-APP-0112, docs/nn/runtime.md
//! section 2, step 6): each object's views and summary, the graph, the sources and the history, within the bounds.
use crate::model::*;
use crate::Refusal;
use alloc::string::String;
use alloc::vec::Vec;
use nrl_core::Shape;
use nrl_repr::affine::Affine;
use nrl_repr::bias::Biases;
use nrl_repr::factor::{Factor, SvdBlock};
use nrl_repr::{Activation, Block, Spline};

pub const MAX_MODEL_BYTES: usize = 16 << 20;
pub const MAX_MANIFEST_BYTES: usize = 1 << 20;
pub const MAX_ARTIFACT_BYTES: usize = 1 << 20;
pub const MAX_ARTIFACT_VALUES: usize = 16384;
pub const MAX_NODES: usize = 64;
pub const MAX_OBJECTS: usize = 64;
pub const MAX_NUMERIC_VALUES: usize = 262144;
pub const MAX_ACTIVATION_VALUES: usize = 1 << 20;
pub const MAX_HISTORY: usize = 128;

fn numeric<T>(r: nrl_core::Result<T>) -> Result<T, Refusal> { r.map_err(|_| Refusal::Malformed("THE VALUES BREAK THE NUMERICAL CONTRACT")) }

/// A graph node's or an object's ID: 1 to 64 of `A-Za-z0-9_.-`.
pub fn id(s: &str) -> bool { !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)) }
/// A SHA-256 written as 64 lower-case hex digits.
pub fn valid_hash(s: &str) -> bool { s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }

pub fn count_bound(a: usize, b: usize, limit: usize) -> Result<usize, Refusal> {
    a.checked_add(b).filter(|n| *n <= limit).ok_or(Refusal::TooLarge("PAST A BOUND OF THE MODEL"))
}

/// A gated block's runtime view: compact (dense, pruned, splines) or factored (SVD).
pub enum RuntimeBlock<'a> { Compact(Block<'a>), Svd(SvdBlock<'a>) }

impl RuntimeBlock<'_> {
    pub fn scratch_len(&self) -> usize { match self { Self::Compact(b) => b.scratch_len(), Self::Svd(b) => b.scratch_len() } }
    pub fn forward(&self, input: &[f64], output: &mut [f64], scratch: &mut [f64]) -> nrl_core::Result<()> {
        match self { Self::Compact(b) => b.forward(input, output, scratch), Self::Svd(b) => b.forward(input, output, scratch) }
    }
}

fn biases(a: &Artifact) -> Biases<'_> { a.bias.as_ref().map_or(Biases::default(), |b| Biases { gate: &b.gate, up: &b.up, down: &b.down }) }

/// An affine object's view (`AffineData::view`).
pub fn affine(a: &AffineData) -> Result<Affine<'_>, Refusal> {
    if a.schema != "nrl.affine/0" { return Err(Refusal::Unsupported("AN AFFINE SCHEMA OTHER THAN nrl.affine/0")); }
    if a.weights.len().checked_add(a.bias.len()).is_none_or(|n| n > MAX_ARTIFACT_VALUES) { return Err(Refusal::TooLarge("AN OBJECT OF MORE THAN 16384 VALUES")); }
    numeric(Affine::new(a.shape[0], a.shape[1], &a.weights, &a.bias))
}

/// A gated block's view (`Artifact::block`).
pub fn block(a: &Artifact) -> Result<RuntimeBlock<'_>, Refusal> {
    if a.schema != "nrl.block/5" { return Err(Refusal::Unsupported("A BLOCK SCHEMA OTHER THAN nrl.block/5")); }
    let d = &a.descriptor;
    if d.kind != "gated-feed-forward/0" || d.source_activation != "silu" || d.dtype != "f64" || d.layout != "row-major" || d.state != "stateless" || d.parameter_sharing != "independent" {
        return Err(Refusal::Unsupported("A BLOCK DESCRIPTOR OTHER THAN gated-feed-forward/0, silu, f64, row-major, stateless, independent"));
    }
    let mut total: usize = 0;
    let mut add = |n: usize| -> Result<(), Refusal> { total = total.checked_add(n).ok_or(Refusal::TooLarge("AN OBJECT OF MORE THAN 16384 VALUES"))?; Ok(()) };
    for n in [a.gate.len(), a.up.len(), a.down.len()] { add(n)?; }
    if let Some(s) = &a.spline { for n in [s.low.len(), s.high.len(), s.values.len()] { add(n)?; } }
    if let Some(f) = &a.factors { for t in f.tensors() { add(t.len())?; } }
    if let Some(b) = &a.bias { for n in [b.gate.len(), b.up.len(), b.down.len()] { add(n)?; } }
    if total > MAX_ARTIFACT_VALUES { return Err(Refusal::TooLarge("AN OBJECT OF MORE THAN 16384 VALUES")); }
    let shape = numeric(Shape::new(a.shape[0], a.shape[1], a.shape[2]))?;
    let biases = biases(a);
    numeric(biases.validate(shape))?;
    if a.representation == "svd" {
        if a.spline.is_some() || a.selection.is_some() || !a.gate.is_empty() || !a.up.is_empty() || !a.down.is_empty() {
            return Err(Refusal::Malformed("A FACTOR BLOCK WITH DENSE, SPLINE OR SELECTION DATA"));
        }
        let f = a.factors.as_ref().ok_or(Refusal::Malformed("AN SVD BLOCK WITHOUT FACTORS"))?;
        let gate = numeric(Factor::new(shape.input(), shape.channels(), f.rank, &f.gate_left, &f.gate_right, biases.gate))?;
        let up = numeric(Factor::new(shape.input(), shape.channels(), f.rank, &f.up_left, &f.up_right, biases.up))?;
        let down = numeric(Factor::new(shape.channels(), shape.output(), f.rank, &f.down_left, &f.down_right, biases.down))?;
        return Ok(RuntimeBlock::Svd(numeric(SvdBlock::new(shape, gate, up, down))?));
    }
    if a.factors.is_some() { return Err(Refusal::Malformed("A COMPACT BLOCK WITH SVD FACTORS")); }
    if let Some(s) = &a.selection {
        numeric(Shape::new(shape.input(), s.original_channels, shape.output()))?;
        let valid = match s.mapping.as_str() {
            "known" => s.indices.len() == shape.channels() && s.indices.iter().all(|i| *i < s.original_channels) && s.indices.windows(2).all(|p| p[0] < p[1])
                && matches!(s.method.as_str(), "weight-energy-f64/1" | "activation-energy-f64/1"),
            "unrecorded" => s.indices.is_empty() && s.method == "qsl-unrecorded/0",
            _ => false,
        };
        if !valid || s.original_channels < shape.channels() || a.representation == "dense" { return Err(Refusal::Malformed("A RETAINED-CHANNEL MAPPING NOT VALID")); }
    } else if a.representation == "pruned" {
        return Err(Refusal::Malformed("A PRUNED BLOCK WITHOUT ITS RETAINED-CHANNEL MAPPING"));
    }
    let activation = match (a.representation.as_str(), &a.spline) {
        ("dense" | "pruned", None) => Activation::Silu,
        ("spline-linear", Some(s)) => Activation::Spline(numeric(Spline::new(shape.channels(), s.knots, &s.low, &s.high, &s.values))?),
        ("spline-cubic", Some(s)) => Activation::Spline(numeric(Spline::hermite(shape.channels(), s.knots, &s.low, &s.high, &s.values))?),
        _ => return Err(Refusal::Unsupported("A REPRESENTATION NOT RUN, OR SPLINE DATA NOT AS IT NEEDS")),
    };
    numeric(Block::new(shape, &a.gate, &a.up, &a.down, activation, biases)).map(RuntimeBlock::Compact)
}

/// An object's summary, its views built first (`ObjectData::summary`).
pub fn summary(data: &ObjectData) -> Result<Summary, Refusal> {
    match data {
        ObjectData::Affine(a) => {
            affine(a)?;
            let values = a.weights.len() + a.bias.len();
            Ok(Summary { kind: "affine".into(), schema: a.schema.clone(), input_width: a.shape[0], output_width: a.shape[1], representation: "dense".into(), channels: None, rank: None, knots: None, numeric_values: values, trainable_values: values, bias_values: a.bias.len() })
        }
        ObjectData::Gated(a) => {
            block(a)?;
            let mut trainable = a.gate.len() + a.up.len() + a.down.len();
            if let Some(f) = &a.factors { trainable += f.tensors().iter().map(|t| t.len()).sum::<usize>(); }
            let bias = a.bias.as_ref().map_or(0, |b| b.gate.len() + b.up.len() + b.down.len());
            trainable += bias + a.spline.as_ref().map_or(0, |s| s.values.len());
            let numeric_values = trainable + a.spline.as_ref().map_or(0, |s| s.low.len() + s.high.len());
            Ok(Summary { kind: "gated".into(), schema: a.schema.clone(), input_width: a.shape[0], output_width: a.shape[2], representation: a.representation.clone(), channels: Some(a.shape[1]),
                         rank: a.factors.as_ref().map(|f| f.rank), knots: a.spline.as_ref().map(|s| s.knots), numeric_values, trainable_values: trainable, bias_values: bias })
        }
    }
}

/// What the graph check gives: each node's width (`input` first, then the nodes), the values a row needs, the output's
/// position (0 for `input`, else a node's index + 1).
pub struct CheckedGraph { pub widths: Vec<usize>, pub values_per_row: usize, pub numeric_values: usize, pub output: usize }

/// The graph against the objects' summaries (`check_graph`); `objects` sorted by key.
pub fn graph(d: &Descriptor, e: &Execution, objects: &[(String, Summary)]) -> Result<CheckedGraph, Refusal> {
    if d.family != "feed-forward" || d.adapter != "nrl.feed-forward" || d.adapter_version != 0 || d.preprocessor != "identity-f64/0" || d.state != "stateless" {
        return Err(Refusal::Unsupported("A MODEL FAMILY, ADAPTER, PREPROCESSOR OR STATE NOT RUN"));
    }
    let task = |t: &String| matches!(t.as_str(), "regression-vector/0" | "classification-logits/0");
    if d.tasks.is_empty() || d.tasks.len() > 2 || !d.tasks.iter().all(task) || (d.tasks.len() == 2 && d.tasks[0] == d.tasks[1]) {
        return Err(Refusal::Unsupported("A MODEL TASK NOT RUN, OR ONE TWICE"));
    }
    if e.schema != "nrl.execution/0" || e.backend != "cpu-scalar" || e.dtype != "f64" || e.layout != "row-major" { return Err(Refusal::Unsupported("AN EXECUTION PROFILE OTHER THAN nrl.execution/0, cpu-scalar, f64, row-major")); }
    if e.max_batch_rows == 0 || e.max_batch_rows > 4096 || e.max_activation_values == 0 || e.max_activation_values > MAX_ACTIVATION_VALUES { return Err(Refusal::TooLarge("AN EXECUTION PROFILE PAST ITS BOUNDS")); }
    let g = &d.configuration;
    if g.schema != "nrl.graph/0" { return Err(Refusal::Unsupported("A GRAPH SCHEMA OTHER THAN nrl.graph/0")); }
    if g.input_width == 0 || g.input_width > 4096 || g.nodes.is_empty() || g.nodes.len() > MAX_NODES || objects.is_empty() || objects.len() > MAX_OBJECTS {
        return Err(Refusal::TooLarge("A GRAPH'S WIDTH OR COUNTS PAST THEIR BOUNDS"));
    }
    let mut numeric_values = 0;
    let mut trainable_values = 0;
    for (key, s) in objects {
        if !id(key) { return Err(Refusal::Malformed("AN OBJECT ID NOT VALID")); }
        numeric_values = count_bound(numeric_values, s.numeric_values, MAX_NUMERIC_VALUES)?;
        trainable_values = count_bound(trainable_values, s.trainable_values, MAX_NUMERIC_VALUES)?;
    }
    let _ = trainable_values;
    // Names in the order they became known: `input`, then each node.
    let mut names: Vec<&str> = alloc::vec!["input"];
    let mut widths: Vec<usize> = alloc::vec![g.input_width];
    let mut aliases: Vec<(&str, Vec<&str>)> = Vec::new();
    let mut values_per_row = g.input_width;
    for node in &g.nodes {
        if !id(&node.id) || names.contains(&node.id.as_str()) { return Err(Refusal::Malformed("A GRAPH NODE ID NOT VALID OR TWICE")); }
        let mut input_widths = [0usize; 2];
        for (w, input) in input_widths.iter_mut().zip(node.op.inputs()) {
            let Some(input) = input else { continue };
            let at = names.iter().position(|n| *n == input).ok_or(Refusal::Malformed("A GRAPH REFERENCE MISSING, FORWARD OR CYCLIC"))?;
            *w = widths[at];
        }
        let width = if let Some(key) = node.op.object() {
            let s = &objects.iter().find(|(k, _)| k == key).ok_or(Refusal::Malformed("A MODEL OBJECT MISSING"))?.1;
            let expected = if matches!(node.op, Operation::Affine { .. }) { "affine" } else { "gated" };
            if s.kind != expected || s.input_width != input_widths[0] { return Err(Refusal::Malformed("A GRAPH OPERATOR'S KIND OR INPUT WIDTH NOT ITS OBJECT'S")); }
            match aliases.iter_mut().find(|(k, _)| *k == key) { Some((_, nodes)) => nodes.push(&node.id), None => aliases.push((key, alloc::vec![node.id.as_str()])) }
            s.output_width
        } else {
            if node.op.inputs()[1].is_some() && input_widths[0] != input_widths[1] { return Err(Refusal::Malformed("AN ADD OF TWO WIDTHS")); }
            input_widths[0]
        };
        if width > 4096 { return Err(Refusal::TooLarge("A GRAPH WIDTH PAST 4096")); }
        values_per_row = count_bound(values_per_row, width, e.max_activation_values)?;
        names.push(&node.id);
        widths.push(width);
    }
    if aliases.len() != objects.len() { return Err(Refusal::Malformed("A MODEL OBJECT UNUSED")); }
    aliases.sort_by(|a, b| a.0.cmp(b.0));
    let ties = aliases.iter().filter(|(_, nodes)| nodes.len() > 1);
    if ties.clone().count() != d.tied_objects.len() || !ties.zip(&d.tied_objects).all(|((object, nodes), tie)| tie.object == *object && tie.nodes.iter().map(String::as_str).eq(nodes.iter().copied())) {
        return Err(Refusal::Malformed("THE DECLARED TIED OBJECTS ARE NOT THE GRAPH'S"));
    }
    if g.output == "input" { return Err(Refusal::Malformed("NO GRAPH OUTPUT")); }
    let output = names.iter().position(|n| *n == g.output).ok_or(Refusal::Malformed("NO GRAPH OUTPUT"))?;
    let mut needed = alloc::vec![false; names.len()];
    needed[output] = true;
    for (i, node) in g.nodes.iter().enumerate().rev() {
        if needed[i + 1] { for input in node.op.inputs().into_iter().flatten() { if let Some(at) = names.iter().position(|n| *n == input) { needed[at] = true; } } }
    }
    if !needed[1..].iter().all(|n| *n) { return Err(Refusal::Malformed("A GRAPH NODE UNREACHABLE")); }
    Ok(CheckedGraph { widths, values_per_row, numeric_values, output })
}

/// The sources and their licences (`validate_sources`).
pub fn sources(sources: &[Source]) -> Result<(), Refusal> {
    if sources.is_empty() || sources.len() > 32 { return Err(Refusal::Malformed("NO SOURCE, OR MORE THAN 32")); }
    for s in sources {
        if s.identifier.is_empty() || s.identifier.len() > 2048 || s.licence.trim().is_empty() || s.licence.len() > 4096 || s.sha256.as_ref().is_some_and(|h| !valid_hash(h)) || s.notices.len() > 16 || s.notices.iter().any(|n| n.len() > 16384) {
            return Err(Refusal::Malformed("A SOURCE, LICENCE OR NOTICE NOT VALID"));
        }
    }
    Ok(())
}

/// A model's change records (`check_history`).
pub fn history(records: &[Change]) -> Result<(), Refusal> {
    if records.len() > MAX_HISTORY { return Err(Refusal::TooLarge("MORE THAN 128 CHANGES")); }
    for c in records {
        if !id(&c.object) || c.operation != "manual-parameter-replacement/0" || !valid_hash(&c.base_model_id) || !valid_hash(&c.before_sha256) || !valid_hash(&c.after_sha256)
            || c.reason.trim().is_empty() || c.reason.len() > 4096 || c.source_identifier.is_empty() || c.source_identifier.len() > 2048 {
            return Err(Refusal::Malformed("A CHANGE RECORD NOT VALID"));
        }
    }
    Ok(())
}
