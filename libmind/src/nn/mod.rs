//! Neural networks as their ONNX graphs, run in the system (250): the network file `scripts/voice_dictate/convert.py`
//! writes (MINDNN01: the graphs, one table of tensors, the weights' bytes), and an interpreter of the ONNX operators the
//! speech models use (`ops`). Weights are read in place from the file's bytes; values made at run time are freed after
//! their last use. No system calls: tests/nn_host.rs includes this module.
mod extra;
pub mod gemm;
pub(crate) mod ops;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Why a file or a run failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a network file, or a damaged one.
    Format(&'static str),
    /// A graph, input or operator the file does not have.
    Missing(String),
    /// An operator met an input it does not take.
    Op(String),
}

pub type Result<T> = core::result::Result<T, Error>;

fn bad(what: &'static str) -> Error { Error::Format(what) }
pub(crate) fn op_error(what: impl Into<String>) -> Error { Error::Op(what.into()) }

/// The element types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DType { F32, U8, I8, I32, I64, Bool }

impl DType {
    fn of(code: u8) -> Option<Self> { Some(match code { 1 => Self::F32, 2 => Self::U8, 3 => Self::I8, 4 => Self::I32, 5 => Self::I64, 6 => Self::Bool, _ => return None }) }
    fn size(self) -> usize { match self { Self::F32 | Self::I32 => 4, Self::I64 => 8, Self::U8 | Self::I8 | Self::Bool => 1 } }
}

/// A tensor's elements.
#[derive(Clone, Debug, PartialEq)]
pub enum Data { F32(Vec<f32>), U8(Vec<u8>), I8(Vec<i8>), I32(Vec<i32>), I64(Vec<i64>), Bool(Vec<bool>) }

/// The same, borrowed: a value made at run time, or a weight in the file.
#[derive(Clone, Copy, Debug)]
pub enum Elems<'a> { F32(&'a [f32]), U8(&'a [u8]), I8(&'a [i8]), I32(&'a [i32]), I64(&'a [i64]), Bool(&'a [bool]) }

impl Data {
    pub fn view(&self) -> Elems<'_> {
        match self { Data::F32(v) => Elems::F32(v), Data::U8(v) => Elems::U8(v), Data::I8(v) => Elems::I8(v), Data::I32(v) => Elems::I32(v), Data::I64(v) => Elems::I64(v), Data::Bool(v) => Elems::Bool(v) }
    }
}

impl Elems<'_> {
    pub fn len(&self) -> usize { match self { Elems::F32(v) => v.len(), Elems::U8(v) => v.len(), Elems::I8(v) => v.len(), Elems::I32(v) => v.len(), Elems::I64(v) => v.len(), Elems::Bool(v) => v.len() } }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    pub fn dtype(&self) -> DType { match self { Elems::F32(_) => DType::F32, Elems::U8(_) => DType::U8, Elems::I8(_) => DType::I8, Elems::I32(_) => DType::I32, Elems::I64(_) => DType::I64, Elems::Bool(_) => DType::Bool } }
    pub fn to_data(&self) -> Data {
        match self { Elems::F32(v) => Data::F32(v.to_vec()), Elems::U8(v) => Data::U8(v.to_vec()), Elems::I8(v) => Data::I8(v.to_vec()), Elems::I32(v) => Data::I32(v.to_vec()), Elems::I64(v) => Data::I64(v.to_vec()), Elems::Bool(v) => Data::Bool(v.to_vec()) }
    }
}

/// A tensor made at run time.
#[derive(Clone, Debug, PartialEq)]
pub struct Tensor { pub shape: Vec<usize>, pub data: Data }

impl Tensor {
    pub fn f32(shape: Vec<usize>, data: Vec<f32>) -> Self { Self { shape, data: Data::F32(data) } }
    pub fn i64(shape: Vec<usize>, data: Vec<i64>) -> Self { Self { shape, data: Data::I64(data) } }
    pub fn view(&self) -> View<'_> { View { shape: &self.shape, data: self.data.view(), panels: false } }
}

/// A tensor as an operator reads it. `panels`: a weight laid out for MatMulInteger (gemm::Layout::Panels), which only
/// that operator reads.
#[derive(Clone, Copy, Debug)]
pub struct View<'a> { pub shape: &'a [usize], pub data: Elems<'a>, pub panels: bool }

impl<'a> View<'a> {
    pub fn numel(&self) -> usize { self.data.len() }
    pub fn to_tensor(&self) -> Tensor { Tensor { shape: self.shape.to_vec(), data: self.data.to_data() } } // in rows: not for panels
}

/// An attribute of a node.
#[derive(Clone, Debug)]
pub(crate) enum Attr { Ints(Vec<i64>), Floats(Vec<f64>), Graph(usize), Text(String) }

pub(crate) struct Node { pub op: usize, pub inputs: Vec<u32>, pub outputs: Vec<u32>, pub freed: Vec<u32>, pub attrs: Vec<(String, Attr)> }

impl Node {
    pub fn int(&self, name: &str) -> Option<i64> { self.attrs.iter().find(|a| a.0 == name).and_then(|a| match &a.1 { Attr::Ints(v) => v.first().copied(), _ => None }) }
    pub fn ints(&self, name: &str) -> Option<&[i64]> { self.attrs.iter().find(|a| a.0 == name).and_then(|a| match &a.1 { Attr::Ints(v) => Some(&v[..]), _ => None }) }
    pub fn floats(&self, name: &str) -> Option<&[f64]> { self.attrs.iter().find(|a| a.0 == name).and_then(|a| match &a.1 { Attr::Floats(v) => Some(&v[..]), _ => None }) }
    pub fn text(&self, name: &str) -> Option<&str> { self.attrs.iter().find(|a| a.0 == name).and_then(|a| match &a.1 { Attr::Text(t) => Some(t.as_str()), _ => None }) }
    pub fn graph(&self, name: &str) -> Option<usize> { self.attrs.iter().find(|a| a.0 == name).and_then(|a| match a.1 { Attr::Graph(g) => Some(g), _ => None }) }
}

pub(crate) struct Graph { pub name: String, pub inputs: Vec<u32>, pub outputs: Vec<u32>, pub nodes: Vec<Node> }

// A tensor of the file: a weight (dtype, dims, bytes) or a value made at run time.
struct Entry { dtype: Option<DType>, panels: bool, dims: Vec<usize>, offset: usize, bytes: usize, name: String }

/// A network file in memory: its graphs, its weights read in place, its tokens.
pub struct Model<'f> {
    file: &'f [u8],
    ops: Vec<String>,
    tensors: Vec<Entry>,
    graphs: Vec<Graph>,
    pub tokens: Vec<String>,
}

struct Reader<'f> { bytes: &'f [u8], at: usize }

impl<'f> Reader<'f> {
    fn take(&mut self, n: usize) -> Result<&'f [u8]> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or(bad("cut short"))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16> { Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap())) }
    fn u32(&mut self) -> Result<u32> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn u64(&mut self) -> Result<u64> { Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap())) }
    fn text(&mut self, n: usize) -> Result<String> { core::str::from_utf8(self.take(n)?).map(String::from).map_err(|_| bad("a name is not UTF-8")) }
    fn align(&mut self) { self.at = (self.at + 63) & !63; }
}

fn fnv1a(bytes: &[u8]) -> u32 { bytes.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193)) }

impl<'f> Model<'f> {
    /// Reads a network file. `check`: verify its checksum (a pass over every byte).
    pub fn parse(file: &'f [u8], check: bool) -> Result<Self> {
        if file.len() < 28 || &file[..8] != b"MINDNN01" { return Err(bad("not a network file")); }
        let body = &file[..file.len() - 4];
        if check && fnv1a(body) != u32::from_le_bytes(file[file.len() - 4..].try_into().unwrap()) { return Err(bad("checksum")); }
        let mut r = Reader { bytes: body, at: 8 };
        if r.u32()? != 2 { return Err(bad("version")); }
        let (graph_count, tensor_count, op_count) = (r.u32()? as usize, r.u32()? as usize, r.u32()? as usize);
        let mut ops = Vec::with_capacity(op_count);
        for _ in 0..op_count { let n = r.u8()? as usize; ops.push(r.text(n)?); }
        r.align();
        let mut tensors = Vec::with_capacity(tensor_count);
        for _ in 0..tensor_count {
            let (code, rank, name_len) = (r.u8()?, r.u8()? as usize, r.u16()? as usize);
            r.u32()?;
            let (offset, bytes) = (r.u64()? as usize, r.u64()? as usize);
            let mut dims = Vec::with_capacity(rank);
            for _ in 0..rank { dims.push(r.u64()? as usize); }
            let name = r.text(name_len)?;
            // Code 7: i8 in panels for MatMulInteger's B (gemm::Layout::Panels), [k, n] with k even and n a multiple of 16.
            let panels = code == 7;
            if panels && (dims.len() != 2 || dims[0] % 2 != 0 || dims[1] % 16 != 0) { return Err(bad("a weight in panels")); }
            let dtype = if code == 0 { None } else if panels { Some(DType::I8) } else { Some(DType::of(code).ok_or(bad("an element type"))?) };
            if let Some(d) = dtype {
                if offset % 8 != 0 || offset.checked_add(bytes).is_none_or(|e| e > body.len()) || dims.iter().product::<usize>() * d.size() != bytes { return Err(bad("a weight's place")); }
            }
            tensors.push(Entry { dtype, panels, dims, offset, bytes, name });
        }
        r.align();
        let id = |v: u32| -> Result<u32> { if v == u32::MAX || (v as usize) < tensor_count { Ok(v) } else { Err(bad("a tensor id")) } };
        let mut graphs = Vec::with_capacity(graph_count);
        for _ in 0..graph_count {
            let n = r.u16()? as usize;
            let name = r.text(n)?;
            let (ins, outs, count) = (r.u32()? as usize, r.u32()? as usize, r.u32()? as usize);
            let inputs = (0..ins).map(|_| r.u32().and_then(id)).collect::<Result<Vec<_>>>()?;
            let outputs = (0..outs).map(|_| r.u32().and_then(id)).collect::<Result<Vec<_>>>()?;
            let mut nodes = Vec::with_capacity(count);
            for _ in 0..count {
                let (op, ni, no, na) = (r.u16()? as usize, r.u8()? as usize, r.u8()? as usize, r.u16()? as usize);
                r.u16()?;
                if op >= op_count { return Err(bad("an operator")); }
                let inputs = (0..ni).map(|_| r.u32().and_then(id)).collect::<Result<Vec<_>>>()?;
                let outputs = (0..no).map(|_| r.u32().and_then(id)).collect::<Result<Vec<_>>>()?;
                let nf = r.u32()? as usize;
                let freed = (0..nf).map(|_| r.u32().and_then(id)).collect::<Result<Vec<_>>>()?;
                let mut attrs = Vec::with_capacity(na);
                for _ in 0..na {
                    let n = r.u8()? as usize;
                    let name = r.text(n)?;
                    let (kind, _, count) = (r.u8()?, r.u8()?, r.u32()? as usize);
                    let attr = match kind {
                        1 | 3 => Attr::Ints((0..count).map(|_| r.u64().map(|v| v as i64)).collect::<Result<_>>()?),
                        2 | 4 => Attr::Floats((0..count).map(|_| r.u64().map(f64::from_bits)).collect::<Result<_>>()?),
                        5 => { let g = (0..count).map(|_| r.u32()).collect::<Result<Vec<_>>>()?; Attr::Graph(*g.first().ok_or(bad("a branch"))? as usize) }
                        6 => Attr::Text(r.text(count)?),
                        _ => return Err(bad("an attribute")),
                    };
                    attrs.push((name, attr));
                }
                nodes.push(Node { op, inputs, outputs, freed, attrs });
            }
            graphs.push(Graph { name, inputs, outputs, nodes });
        }
        for g in &graphs {
            for n in &g.nodes { if let Some(b) = n.attrs.iter().find_map(|a| if let Attr::Graph(b) = a.1 { Some(b) } else { None }) { if b >= graphs.len() { return Err(bad("a branch")); } } }
        }
        r.align();
        let count = r.u32()? as usize;
        let mut tokens = Vec::with_capacity(count);
        for _ in 0..count { let n = r.u16()? as usize; tokens.push(r.text(n)?); }
        Ok(Self { file: body, ops, tensors, graphs, tokens })
    }

    /// A weight's elements, in place when the file's bytes are aligned for them.
    fn weight(&self, id: usize) -> Option<View<'_>> {
        let e = &self.tensors[id];
        let dtype = e.dtype?;
        let bytes = &self.file[e.offset..e.offset + e.bytes];
        let n = e.bytes / dtype.size();
        let ptr = bytes.as_ptr();
        let aligned = ptr as usize % dtype.size() == 0;
        if !aligned { return None; }
        // SAFETY: the bytes are in the file for as long as the model borrows it, aligned (checked) and every bit pattern
        // is a valid value of these types; bools are 0 or 1 as ONNX writes them (other bytes are refused in `ops`).
        let data = unsafe {
            match dtype {
                DType::F32 => Elems::F32(core::slice::from_raw_parts(ptr as *const f32, n)),
                DType::U8 => Elems::U8(bytes),
                DType::I8 => Elems::I8(core::slice::from_raw_parts(ptr as *const i8, n)),
                DType::I32 => Elems::I32(core::slice::from_raw_parts(ptr as *const i32, n)),
                DType::I64 => Elems::I64(core::slice::from_raw_parts(ptr as *const i64, n)),
                DType::Bool => { if bytes.iter().any(|&b| b > 1) { return None; } Elems::Bool(core::slice::from_raw_parts(ptr as *const bool, n)) }
            }
        };
        Some(View { shape: &e.dims, data, panels: e.panels })
    }

    /// Whether the file has a graph `name`.
    pub fn has(&self, name: &str) -> bool { self.graph(name).is_ok() }

    fn graph(&self, name: &str) -> Result<usize> { self.graphs.iter().position(|g| g.name == name).ok_or_else(|| Error::Missing(String::from(name))) }

    /// Runs graph `name` on `inputs` (in the graph's order); its outputs in its order.
    pub fn run(&self, name: &str, inputs: Vec<Tensor>) -> Result<Vec<Tensor>> { self.run_watched(name, inputs, &mut |_, _, _| {}) }

    /// The same, showing `watch` every value a node of the graph makes: the node's operator, the value's name (to compare
    /// with another runtime) and the value.
    pub fn run_watched(&self, name: &str, inputs: Vec<Tensor>, watch: &mut dyn FnMut(&str, &str, &Tensor)) -> Result<Vec<Tensor>> {
        let g = self.graph(name)?;
        let graph = &self.graphs[g];
        if inputs.len() != graph.inputs.len() { return Err(Error::Missing(alloc::format!("{} takes {} inputs", name, graph.inputs.len()))); }
        let mut values: Vec<Option<Tensor>> = vec![None; self.tensors.len()];
        for (&id, t) in graph.inputs.iter().zip(inputs) { values[id as usize] = Some(t); }
        self.execute(g, &mut values, watch)?;
        graph.outputs.iter().map(|&id| values[id as usize].take().ok_or_else(|| op_error("an output was not made"))).collect()
    }

    // Runs graph `g`'s nodes in order (ONNX keeps them sorted).
    fn execute(&self, g: usize, values: &mut [Option<Tensor>], watch: &mut dyn FnMut(&str, &str, &Tensor)) -> Result<()> {
        for node in &self.graphs[g].nodes {
            let op = self.ops[node.op].as_str();
            if op == "If" {
                let cond = self.input(values, node, 0)?.ok_or_else(|| op_error("If without a condition"))?;
                let take = match cond.data { Elems::Bool(b) => b.first().copied().unwrap_or(false), _ => return Err(op_error("If: the condition is not a bool")) };
                let branch = node.graph(if take { "then_branch" } else { "else_branch" }).ok_or_else(|| op_error("If without its branches"))?;
                self.execute(branch, values, watch)?;
                for (&out, &made) in node.outputs.iter().zip(&self.graphs[branch].outputs) {
                    let t = match values[made as usize].take() { Some(t) => t, None => self.weight(made as usize).filter(|w| !w.panels).ok_or_else(|| op_error("If: a branch output"))?.to_tensor() };
                    values[out as usize] = Some(t);
                }
            } else {
                let made = {
                    let inputs = (0..node.inputs.len()).map(|i| self.input(values, node, i)).collect::<Result<Vec<_>>>()?;
                    ops::run(op, node, &inputs).map_err(|e| match e {
                        Error::Op(m) => Error::Op(alloc::format!("{} ({}): {}", op, node.outputs.first().map_or("", |&o| self.tensors[o as usize].name.as_str()), m)),
                        other => other,
                    })?
                };
                if made.len() < node.outputs.len() { return Err(op_error(alloc::format!("{} made {} outputs", op, made.len()))); }
                for (&out, t) in node.outputs.iter().zip(made) { watch(op, &self.tensors[out as usize].name, &t); values[out as usize] = Some(t); }
            }
            for &id in &node.freed { values[id as usize] = None; }
        }
        Ok(())
    }

    // Input `i` of `node`: absent, a value made at run time, or a weight.
    fn input<'v>(&'v self, values: &'v [Option<Tensor>], node: &Node, i: usize) -> Result<Option<View<'v>>> {
        let id = node.inputs[i];
        if id == u32::MAX { return Ok(None); }
        if let Some(t) = &values[id as usize] { return Ok(Some(t.view())); }
        self.weight(id as usize).map(Some).ok_or_else(|| op_error(alloc::format!("input {} of {} is missing", i, self.ops[node.op])))
    }
}
