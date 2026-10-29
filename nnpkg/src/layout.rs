//! The fixed layout of a package's typed description (262-APP-0112, docs/nn/runtime.md sections 5 and 6): what the
//! parser worker answers, and what nnd gives a job's worker. Little-endian: integers as u64, a count or a length as
//! u32, f64 as its 8 bytes, an `Option` as a byte 0 or 1. Reading it checks every length against what is left, so an
//! answer the parser made up cannot make nnd allocate past it; nnd trusts no value read until section 2's checks pass.
use crate::model::*;
use crate::Refusal;
use alloc::string::String;
use alloc::vec::Vec;

const MAGIC: &[u8; 8] = b"NNPKG\0\0\x01";
const BROKEN: Refusal = Refusal::Malformed("THE PARSER'S ANSWER IS NOT IN THE LAYOUT");

pub struct Writer { pub out: Vec<u8> }

impl Writer {
    pub fn new() -> Self { Self { out: Vec::from(&MAGIC[..]) } }
    fn u8(&mut self, v: u8) { self.out.push(v); }
    fn count(&mut self, n: usize) { self.out.extend_from_slice(&(n as u32).to_le_bytes()); }
    fn uint(&mut self, v: usize) { self.out.extend_from_slice(&(v as u64).to_le_bytes()); }
    fn str(&mut self, s: &str) { self.count(s.len()); self.out.extend_from_slice(s.as_bytes()); }
    fn strs(&mut self, v: &[String]) { self.count(v.len()); for s in v { self.str(s); } }
    fn floats(&mut self, v: &[f64]) { self.count(v.len()); for x in v { self.out.extend_from_slice(&x.to_le_bytes()); } }
    fn uints(&mut self, v: &[usize]) { self.count(v.len()); for &x in v { self.uint(x); } }
    fn option<T>(&mut self, v: &Option<T>, f: impl FnOnce(&mut Self, &T)) { match v { None => self.u8(0), Some(x) => { self.u8(1); f(self, x); } } }

    fn summary(&mut self, s: &Summary) {
        self.str(&s.kind); self.str(&s.schema); self.uint(s.input_width); self.uint(s.output_width); self.str(&s.representation);
        self.option(&s.channels, |w, &c| w.uint(c)); self.option(&s.rank, |w, &r| w.uint(r)); self.option(&s.knots, |w, &k| w.uint(k));
        self.uint(s.numeric_values); self.uint(s.trainable_values); self.uint(s.bias_values);
    }
    fn change(&mut self, c: &Change) {
        self.str(&c.object); self.str(&c.operation); self.str(&c.base_model_id); self.str(&c.before_sha256); self.str(&c.after_sha256);
        self.summary(&c.parameters); self.str(&c.reason); self.str(&c.source_identifier);
    }

    pub fn manifest(&mut self, m: &Manifest) {
        self.str(&m.schema); self.str(&m.kind);
        let d = &m.descriptor;
        self.str(&d.family); self.str(&d.adapter); self.uint(d.adapter_version);
        let g = &d.configuration;
        self.str(&g.schema); self.uint(g.input_width);
        self.count(g.nodes.len());
        for n in &g.nodes {
            self.str(&n.id);
            match &n.op {
                Operation::Affine { input, object } => { self.u8(0); self.str(input); self.str(object); }
                Operation::Gated { input, object } => { self.u8(1); self.str(input); self.str(object); }
                Operation::Silu { input } => { self.u8(2); self.str(input); }
                Operation::Relu { input } => { self.u8(3); self.str(input); }
                Operation::Add { left, right } => { self.u8(4); self.str(left); self.str(right); }
            }
        }
        self.str(&g.output); self.str(&d.preprocessor); self.strs(&d.tasks); self.str(&d.state);
        self.count(d.tied_objects.len());
        for t in &d.tied_objects { self.str(&t.object); self.strs(&t.nodes); }
        let e = &m.execution;
        self.str(&e.schema); self.str(&e.backend); self.str(&e.dtype); self.str(&e.layout); self.uint(e.max_batch_rows); self.uint(e.max_activation_values);
        self.option(&m.base, |w, b| w.str(&b.model_id));
        self.count(m.objects.len());
        for (key, r) in &m.objects { self.str(key); self.str(&r.file); self.str(&r.sha256); self.uint(r.bytes); self.summary(&r.parameters); }
        self.count(m.sources.len());
        for s in &m.sources { self.str(&s.identifier); self.option(&s.sha256, |w, h| w.str(h)); self.str(&s.licence); self.strs(&s.notices); }
        self.count(m.history.len());
        for c in &m.history { self.change(c); }
        self.count(m.changes.len());
        for c in &m.changes { self.change(c); }
    }

    pub fn object(&mut self, data: &ObjectData) {
        match data {
            ObjectData::Affine(a) => { self.u8(0); self.str(&a.schema); self.uint(a.shape[0]); self.uint(a.shape[1]); self.floats(&a.weights); self.floats(&a.bias); }
            ObjectData::Gated(a) => {
                self.u8(1);
                let d = &a.descriptor;
                for s in [&d.kind, &d.source_activation, &d.dtype, &d.layout, &d.state, &d.parameter_sharing] { self.str(s); }
                self.option(&a.bias, |w, b| { w.floats(&b.gate); w.floats(&b.up); w.floats(&b.down); });
                self.str(&a.schema); self.str(&a.representation);
                for &n in &a.shape { self.uint(n); }
                self.floats(&a.gate); self.floats(&a.up); self.floats(&a.down);
                self.option(&a.spline, |w, s| { w.uint(s.knots); w.floats(&s.low); w.floats(&s.high); w.floats(&s.values); });
                self.option(&a.selection, |w, s| { w.str(&s.mapping); w.uint(s.original_channels); w.uints(&s.indices); w.str(&s.method); });
                self.option(&a.factors, |w, f| { w.uint(f.rank); for t in f.tensors() { w.floats(t); } });
            }
        }
    }

    /// A package's objects, each with its key.
    pub fn objects(&mut self, objects: &[(String, ObjectData)]) {
        self.count(objects.len());
        for (key, data) in objects { self.str(key); self.object(data); }
    }
}

impl Default for Writer { fn default() -> Self { Self::new() } }

pub struct Reader<'a> { bytes: &'a [u8], at: usize }

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, Refusal> {
        if !bytes.starts_with(MAGIC) { return Err(BROKEN); }
        Ok(Self { bytes, at: MAGIC.len() })
    }
    pub fn done(&self) -> Result<(), Refusal> { if self.at == self.bytes.len() { Ok(()) } else { Err(BROKEN) } }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Refusal> {
        let end = self.at.checked_add(n).filter(|e| *e <= self.bytes.len()).ok_or(BROKEN)?;
        let s = &self.bytes[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, Refusal> { Ok(self.take(1)?[0]) }
    /// A description's head: the model ID and the files' bytes.
    pub fn head(&mut self) -> Result<([u8; 32], usize), Refusal> { let id = self.take(32)?.try_into().unwrap(); Ok((id, self.uint()?)) }
    // A count of items of at least `least` bytes each, no more than what is left could hold.
    fn count(&mut self, least: usize) -> Result<usize, Refusal> {
        let n = u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize;
        if n.saturating_mul(least) > self.bytes.len() - self.at { return Err(BROKEN); }
        Ok(n)
    }
    fn uint(&mut self) -> Result<usize, Refusal> { usize::try_from(u64::from_le_bytes(self.take(8)?.try_into().unwrap())).map_err(|_| BROKEN) }
    fn str(&mut self) -> Result<String, Refusal> { let n = self.count(1)?; core::str::from_utf8(self.take(n)?).map(String::from).map_err(|_| BROKEN) }
    fn strs(&mut self) -> Result<Vec<String>, Refusal> { let n = self.count(4)?; (0..n).map(|_| self.str()).collect() }
    fn floats(&mut self) -> Result<Vec<f64>, Refusal> { let n = self.count(8)?; Ok(self.take(n * 8)?.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()) }
    fn uints(&mut self) -> Result<Vec<usize>, Refusal> { let n = self.count(8)?; (0..n).map(|_| self.uint()).collect() }
    fn option<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, Refusal>) -> Result<Option<T>, Refusal> {
        match self.u8()? { 0 => Ok(None), 1 => f(self).map(Some), _ => Err(BROKEN) }
    }

    fn summary(&mut self) -> Result<Summary, Refusal> {
        Ok(Summary {
            kind: self.str()?, schema: self.str()?, input_width: self.uint()?, output_width: self.uint()?, representation: self.str()?,
            channels: self.option(Self::uint)?, rank: self.option(Self::uint)?, knots: self.option(Self::uint)?,
            numeric_values: self.uint()?, trainable_values: self.uint()?, bias_values: self.uint()?,
        })
    }
    fn change(&mut self) -> Result<Change, Refusal> {
        Ok(Change { object: self.str()?, operation: self.str()?, base_model_id: self.str()?, before_sha256: self.str()?, after_sha256: self.str()?, parameters: self.summary()?, reason: self.str()?, source_identifier: self.str()? })
    }

    pub fn manifest(&mut self) -> Result<Manifest, Refusal> {
        let (schema, kind) = (self.str()?, self.str()?);
        let (family, adapter, adapter_version) = (self.str()?, self.str()?, self.uint()?);
        let (graph_schema, input_width) = (self.str()?, self.uint()?);
        let nodes = (0..self.count(5)?).map(|_| {
            let id = self.str()?;
            let op = match self.u8()? {
                0 => Operation::Affine { input: self.str()?, object: self.str()? },
                1 => Operation::Gated { input: self.str()?, object: self.str()? },
                2 => Operation::Silu { input: self.str()? },
                3 => Operation::Relu { input: self.str()? },
                4 => Operation::Add { left: self.str()?, right: self.str()? },
                _ => return Err(BROKEN),
            };
            Ok(Node { id, op })
        }).collect::<Result<Vec<_>, _>>()?;
        let configuration = Graph { schema: graph_schema, input_width, nodes, output: self.str()? };
        let (preprocessor, tasks, state) = (self.str()?, self.strs()?, self.str()?);
        let tied_objects = (0..self.count(8)?).map(|_| Ok(Tie { object: self.str()?, nodes: self.strs()? })).collect::<Result<Vec<_>, _>>()?;
        let descriptor = Descriptor { family, adapter, adapter_version, configuration, preprocessor, tasks, state, tied_objects };
        let execution = Execution { schema: self.str()?, backend: self.str()?, dtype: self.str()?, layout: self.str()?, max_batch_rows: self.uint()?, max_activation_values: self.uint()? };
        let base = self.option(|r| Ok(Dependency { model_id: r.str()? }))?;
        let objects = (0..self.count(16)?).map(|_| {
            let key = self.str()?;
            Ok((key, ObjectRecord { file: self.str()?, sha256: self.str()?, bytes: self.uint()?, parameters: self.summary()? }))
        }).collect::<Result<Vec<_>, _>>()?;
        let sources = (0..self.count(13)?).map(|_| Ok(Source { identifier: self.str()?, sha256: self.option(Self::str)?, licence: self.str()?, notices: self.strs()? })).collect::<Result<Vec<_>, _>>()?;
        let history = (0..self.count(16)?).map(|_| self.change()).collect::<Result<Vec<_>, _>>()?;
        let changes = (0..self.count(16)?).map(|_| self.change()).collect::<Result<Vec<_>, _>>()?;
        Ok(Manifest { schema, kind, descriptor, execution, base, objects, sources, history, changes })
    }

    pub fn object(&mut self) -> Result<ObjectData, Refusal> {
        match self.u8()? {
            0 => Ok(ObjectData::Affine(AffineData { schema: self.str()?, shape: [self.uint()?, self.uint()?], weights: self.floats()?, bias: self.floats()? })),
            1 => {
                let descriptor = BlockDescriptor { kind: self.str()?, source_activation: self.str()?, dtype: self.str()?, layout: self.str()?, state: self.str()?, parameter_sharing: self.str()? };
                let bias = self.option(|r| Ok(BiasData { gate: r.floats()?, up: r.floats()?, down: r.floats()? }))?;
                let (schema, representation) = (self.str()?, self.str()?);
                let shape = [self.uint()?, self.uint()?, self.uint()?];
                let (gate, up, down) = (self.floats()?, self.floats()?, self.floats()?);
                let spline = self.option(|r| Ok(SplineData { knots: r.uint()?, low: r.floats()?, high: r.floats()?, values: r.floats()? }))?;
                let selection = self.option(|r| Ok(SelectionData { mapping: r.str()?, original_channels: r.uint()?, indices: r.uints()?, method: r.str()? }))?;
                let factors = self.option(|r| Ok(FactorData { rank: r.uint()?, gate_left: r.floats()?, gate_right: r.floats()?, up_left: r.floats()?, up_right: r.floats()?, down_left: r.floats()?, down_right: r.floats()? }))?;
                Ok(ObjectData::Gated(Artifact { descriptor, bias, schema, representation, shape, gate, up, down, spline, selection, factors }))
            }
            _ => Err(BROKEN),
        }
    }

    pub fn objects(&mut self) -> Result<Vec<(String, ObjectData)>, Refusal> {
        (0..self.count(5)?).map(|_| Ok((self.str()?, self.object()?))).collect()
    }
}
