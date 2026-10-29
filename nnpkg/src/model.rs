//! The typed description of an `nrl.model/0` package, as `nrl-lab`'s records declare it (262-APP-0112): read from the
//! JSON tree with `serde`'s rules for those records (unknown fields refused, an `Option` absent or null).
use crate::json::Value;
use crate::Refusal;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq)]
pub struct AffineData { pub schema: String, pub shape: [usize; 2], pub weights: Vec<f64>, pub bias: Vec<f64> }

#[derive(Debug, Clone, PartialEq)]
pub struct BlockDescriptor { pub kind: String, pub source_activation: String, pub dtype: String, pub layout: String, pub state: String, pub parameter_sharing: String }

#[derive(Debug, Clone, PartialEq)]
pub struct BiasData { pub gate: Vec<f64>, pub up: Vec<f64>, pub down: Vec<f64> }

#[derive(Debug, Clone, PartialEq)]
pub struct SplineData { pub knots: usize, pub low: Vec<f64>, pub high: Vec<f64>, pub values: Vec<f64> }

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionData { pub mapping: String, pub original_channels: usize, pub indices: Vec<usize>, pub method: String }

#[derive(Debug, Clone, PartialEq)]
pub struct FactorData { pub rank: usize, pub gate_left: Vec<f64>, pub gate_right: Vec<f64>, pub up_left: Vec<f64>, pub up_right: Vec<f64>, pub down_left: Vec<f64>, pub down_right: Vec<f64> }

impl FactorData {
    pub fn tensors(&self) -> [&[f64]; 6] { [&self.gate_left, &self.gate_right, &self.up_left, &self.up_right, &self.down_left, &self.down_right] }
}

/// A gated block (`nrl.block/5`), `nrl-lab`'s `Artifact`.
#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    pub descriptor: BlockDescriptor,
    pub bias: Option<BiasData>,
    pub schema: String,
    pub representation: String,
    pub shape: [usize; 3],
    pub gate: Vec<f64>,
    pub up: Vec<f64>,
    pub down: Vec<f64>,
    pub spline: Option<SplineData>,
    pub selection: Option<SelectionData>,
    pub factors: Option<FactorData>,
}

/// An object file's record: `{"kind": "affine" | "gated", "data": ...}`.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectData { Affine(AffineData), Gated(Artifact) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub kind: String,
    pub schema: String,
    pub input_width: usize,
    pub output_width: usize,
    pub representation: String,
    pub channels: Option<usize>,
    pub rank: Option<usize>,
    pub knots: Option<usize>,
    pub numeric_values: usize,
    pub trainable_values: usize,
    pub bias_values: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation { Affine { input: String, object: String }, Gated { input: String, object: String }, Silu { input: String }, Relu { input: String }, Add { left: String, right: String } }

impl Operation {
    pub fn inputs(&self) -> [Option<&str>; 2] {
        match self {
            Self::Affine { input, .. } | Self::Gated { input, .. } | Self::Silu { input } | Self::Relu { input } => [Some(input), None],
            Self::Add { left, right } => [Some(left), Some(right)],
        }
    }
    pub fn object(&self) -> Option<&str> {
        match self { Self::Affine { object, .. } | Self::Gated { object, .. } => Some(object), _ => None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node { pub id: String, pub op: Operation }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph { pub schema: String, pub input_width: usize, pub nodes: Vec<Node>, pub output: String }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tie { pub object: String, pub nodes: Vec<String> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor { pub family: String, pub adapter: String, pub adapter_version: usize, pub configuration: Graph, pub preprocessor: String, pub tasks: Vec<String>, pub state: String, pub tied_objects: Vec<Tie> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution { pub schema: String, pub backend: String, pub dtype: String, pub layout: String, pub max_batch_rows: usize, pub max_activation_values: usize }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source { pub identifier: String, pub sha256: Option<String>, pub licence: String, pub notices: Vec<String> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRecord { pub file: String, pub sha256: String, pub bytes: usize, pub parameters: Summary }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency { pub model_id: String }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change { pub object: String, pub operation: String, pub base_model_id: String, pub before_sha256: String, pub after_sha256: String, pub parameters: Summary, pub reason: String, pub source_identifier: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub schema: String,
    pub kind: String,
    pub descriptor: Descriptor,
    pub execution: Execution,
    pub base: Option<Dependency>,
    /// Sorted by key, as a `BTreeMap` keeps them.
    pub objects: Vec<(String, ObjectRecord)>,
    pub sources: Vec<Source>,
    pub history: Vec<Change>,
    pub changes: Vec<Change>,
}

const MALFORMED: Refusal = Refusal::Malformed("A RECORD NOT AS nrl.model/0 DECLARES IT");

// A record's members, taken by name; any left over is an unknown field (`deny_unknown_fields`).
struct Fields<'v, 'a> { members: &'v [(String, Value<'a>)], taken: u64 }

impl<'v, 'a> Fields<'v, 'a> {
    fn of(value: &'v Value<'a>) -> Result<Self, Refusal> {
        match value { Value::Object(members) if members.len() <= 64 => Ok(Self { members, taken: 0 }), _ => Err(MALFORMED) }
    }
    fn get(&mut self, name: &str) -> Option<&'v Value<'a>> {
        let i = self.members.iter().position(|(k, _)| k == name)?;
        self.taken |= 1 << i;
        Some(&self.members[i].1)
    }
    fn need(&mut self, name: &str) -> Result<&'v Value<'a>, Refusal> { self.get(name).ok_or(MALFORMED) }
    fn option(&mut self, name: &str) -> Option<&'v Value<'a>> { self.get(name).filter(|v| **v != Value::Null) }
    fn done(self) -> Result<(), Refusal> {
        if self.taken.count_ones() as usize == self.members.len() { Ok(()) } else { Err(Refusal::Malformed("AN UNKNOWN FIELD")) }
    }
}

fn string(value: &Value) -> Result<String, Refusal> { match value { Value::String(s) => Ok(s.clone()), _ => Err(MALFORMED) } }

/// An unsigned integer as `serde_json` reads one: an integer literal (`-0` is 0), never a fraction or an exponent.
pub fn integer(value: &Value) -> Result<usize, Refusal> {
    let Value::Number(text) = value else { return Err(MALFORMED) };
    let digits = text.strip_prefix('-').unwrap_or(text);
    if !digits.bytes().all(|b| b.is_ascii_digit()) { return Err(MALFORMED); }
    let n: u64 = digits.parse().map_err(|_| MALFORMED)?;
    if text.starts_with('-') && n != 0 { return Err(MALFORMED); }
    usize::try_from(n).map_err(|_| MALFORMED)
}

/// An f64 as `serde_json` reads one: an integer literal within 64 bits converted, else the nearest double; out of
/// range refused.
pub fn float(value: &Value) -> Result<f64, Refusal> {
    let Value::Number(text) = value else { return Err(MALFORMED) };
    let integral = !text.bytes().any(|b| matches!(b, b'.' | b'e' | b'E'));
    if integral {
        if let Some(digits) = text.strip_prefix('-') {
            if let Ok(n) = digits.parse::<u64>() { if n <= 1 << 63 { return Ok(-(n as i128) as i64 as f64); } }
        } else if let Ok(n) = text.parse::<u64>() {
            return Ok(n as f64);
        }
    }
    let v: f64 = text.parse().map_err(|_| MALFORMED)?;
    if v.is_finite() { Ok(v) } else { Err(Refusal::Malformed("A NUMBER OUT OF RANGE")) }
}

fn array<'v, 'a>(value: &'v Value<'a>) -> Result<&'v [Value<'a>], Refusal> { match value { Value::Array(items) => Ok(items), _ => Err(MALFORMED) } }
fn floats(value: &Value) -> Result<Vec<f64>, Refusal> { array(value)?.iter().map(float).collect() }
fn integers(value: &Value) -> Result<Vec<usize>, Refusal> { array(value)?.iter().map(integer).collect() }
fn strings(value: &Value) -> Result<Vec<String>, Refusal> { array(value)?.iter().map(string).collect() }
fn fixed<const N: usize>(value: &Value) -> Result<[usize; N], Refusal> {
    let items = array(value)?;
    if items.len() != N { return Err(MALFORMED); }
    let mut out = [0; N];
    for (o, v) in out.iter_mut().zip(items) { *o = integer(v)?; }
    Ok(out)
}

fn affine(value: &Value) -> Result<AffineData, Refusal> {
    let mut f = Fields::of(value)?;
    let data = AffineData { schema: string(f.need("schema")?)?, shape: fixed(f.need("shape")?)?, weights: floats(f.need("weights")?)?, bias: floats(f.need("bias")?)? };
    f.done().map(|_| data)
}

fn artifact(value: &Value) -> Result<Artifact, Refusal> {
    let mut f = Fields::of(value)?;
    let descriptor = {
        let mut d = Fields::of(f.need("descriptor")?)?;
        let descriptor = BlockDescriptor {
            kind: string(d.need("kind")?)?,
            source_activation: string(d.need("source_activation")?)?,
            dtype: string(d.need("dtype")?)?,
            layout: string(d.need("layout")?)?,
            state: string(d.need("state")?)?,
            parameter_sharing: string(d.need("parameter_sharing")?)?,
        };
        d.done()?;
        descriptor
    };
    let bias = f.option("bias").map(|v| {
        let mut b = Fields::of(v)?;
        let bias = BiasData { gate: floats(b.need("gate")?)?, up: floats(b.need("up")?)?, down: floats(b.need("down")?)? };
        b.done().map(|_| bias)
    }).transpose()?;
    let schema = string(f.need("schema")?)?;
    let representation = string(f.need("representation")?)?;
    let shape = fixed(f.need("shape")?)?;
    let gate = floats(f.need("gate")?)?;
    let up = floats(f.need("up")?)?;
    let down = floats(f.need("down")?)?;
    let spline = f.option("spline").map(|v| {
        let mut s = Fields::of(v)?;
        let spline = SplineData { knots: integer(s.need("knots")?)?, low: floats(s.need("low")?)?, high: floats(s.need("high")?)?, values: floats(s.need("values")?)? };
        s.done().map(|_| spline)
    }).transpose()?;
    let selection = f.option("selection").map(|v| {
        let mut s = Fields::of(v)?;
        let selection = SelectionData { mapping: string(s.need("mapping")?)?, original_channels: integer(s.need("original_channels")?)?, indices: integers(s.need("indices")?)?, method: string(s.need("method")?)? };
        s.done().map(|_| selection)
    }).transpose()?;
    let factors = f.option("factors").map(|v| {
        let mut s = Fields::of(v)?;
        let factors = FactorData {
            rank: integer(s.need("rank")?)?,
            gate_left: floats(s.need("gate_left")?)?,
            gate_right: floats(s.need("gate_right")?)?,
            up_left: floats(s.need("up_left")?)?,
            up_right: floats(s.need("up_right")?)?,
            down_left: floats(s.need("down_left")?)?,
            down_right: floats(s.need("down_right")?)?,
        };
        s.done().map(|_| factors)
    }).transpose()?;
    f.done()?;
    Ok(Artifact { descriptor, bias, schema, representation, shape, gate, up, down, spline, selection, factors })
}

/// An object file's record, as `nrl-lab`'s `decode_object`: its kind and its data's schema first.
pub fn object(value: &Value) -> Result<ObjectData, Refusal> {
    let mut f = Fields::of(value)?;
    let kind = f.need("kind")?;
    let data = f.need("data")?;
    f.done()?;
    let schema = match data { Value::Object(m) => m.iter().find(|(k, _)| k == "schema").map(|(_, v)| v), _ => None };
    let schema = match schema { Some(Value::String(s)) => s.as_str(), _ => return Err(Refusal::Malformed("NO SCHEMA OF THE OBJECT'S DATA")) };
    match kind {
        Value::String(k) if k == "gated" => {
            if schema != "nrl.block/5" { return Err(Refusal::Unsupported("AN OBJECT SCHEMA OTHER THAN nrl.block/5")); }
            artifact(data).map(ObjectData::Gated)
        }
        Value::String(k) if k == "affine" => {
            if schema != "nrl.affine/0" { return Err(Refusal::Unsupported("AN OBJECT SCHEMA OTHER THAN nrl.affine/0")); }
            affine(data).map(ObjectData::Affine)
        }
        _ => Err(Refusal::Unsupported("AN OBJECT KIND OTHER THAN affine OR gated")),
    }
}

fn summary(value: &Value) -> Result<Summary, Refusal> {
    let mut f = Fields::of(value)?;
    let s = Summary {
        kind: string(f.need("kind")?)?,
        schema: string(f.need("schema")?)?,
        input_width: integer(f.need("input_width")?)?,
        output_width: integer(f.need("output_width")?)?,
        representation: string(f.need("representation")?)?,
        channels: f.option("channels").map(integer).transpose()?,
        rank: f.option("rank").map(integer).transpose()?,
        knots: f.option("knots").map(integer).transpose()?,
        numeric_values: integer(f.need("numeric_values")?)?,
        trainable_values: integer(f.need("trainable_values")?)?,
        bias_values: integer(f.need("bias_values")?)?,
    };
    f.done().map(|_| s)
}

fn operation(value: &Value) -> Result<Operation, Refusal> {
    let mut f = Fields::of(value)?;
    let op = match string(f.need("operator")?)?.as_str() {
        "affine" => Operation::Affine { input: string(f.need("input")?)?, object: string(f.need("object")?)? },
        "gated" => Operation::Gated { input: string(f.need("input")?)?, object: string(f.need("object")?)? },
        "silu" => Operation::Silu { input: string(f.need("input")?)? },
        "relu" => Operation::Relu { input: string(f.need("input")?)? },
        "add" => Operation::Add { left: string(f.need("left")?)?, right: string(f.need("right")?)? },
        _ => return Err(Refusal::Unsupported("AN OPERATOR OTHER THAN affine, gated, silu, relu OR add")),
    };
    f.done().map(|_| op)
}

fn change(value: &Value) -> Result<Change, Refusal> {
    let mut f = Fields::of(value)?;
    let c = Change {
        object: string(f.need("object")?)?,
        operation: string(f.need("operation")?)?,
        base_model_id: string(f.need("base_model_id")?)?,
        before_sha256: string(f.need("before_sha256")?)?,
        after_sha256: string(f.need("after_sha256")?)?,
        parameters: summary(f.need("parameters")?)?,
        reason: string(f.need("reason")?)?,
        source_identifier: string(f.need("source_identifier")?)?,
    };
    f.done().map(|_| c)
}

fn list<T>(value: &Value, each: impl Fn(&Value) -> Result<T, Refusal>) -> Result<Vec<T>, Refusal> { array(value)?.iter().map(each).collect() }

/// The manifest, as `nrl-lab`'s `Manifest` (its schema checked first, as `require_schema` does).
pub fn manifest(value: &Value) -> Result<Manifest, Refusal> {
    let mut f = Fields::of(value)?;
    match f.need("schema")? {
        Value::String(s) if s == "nrl.model/0" => {}
        Value::String(_) => return Err(Refusal::Unsupported("A MANIFEST SCHEMA OTHER THAN nrl.model/0")),
        _ => return Err(Refusal::Malformed("NO MANIFEST SCHEMA")),
    }
    let kind = string(f.need("kind")?)?;
    let descriptor = {
        let mut d = Fields::of(f.need("descriptor")?)?;
        let configuration = {
            let mut g = Fields::of(d.need("configuration")?)?;
            let graph = Graph {
                schema: string(g.need("schema")?)?,
                input_width: integer(g.need("input_width")?)?,
                nodes: list(g.need("nodes")?, |v| {
                    let mut n = Fields::of(v)?;
                    let node = Node { id: string(n.need("id")?)?, op: operation(n.need("op")?)? };
                    n.done().map(|_| node)
                })?,
                output: string(g.need("output")?)?,
            };
            g.done()?;
            graph
        };
        let descriptor = Descriptor {
            family: string(d.need("family")?)?,
            adapter: string(d.need("adapter")?)?,
            adapter_version: integer(d.need("adapter_version")?)?,
            configuration,
            preprocessor: string(d.need("preprocessor")?)?,
            tasks: strings(d.need("tasks")?)?,
            state: string(d.need("state")?)?,
            tied_objects: list(d.need("tied_objects")?, |v| {
                let mut t = Fields::of(v)?;
                let tie = Tie { object: string(t.need("object")?)?, nodes: strings(t.need("nodes")?)? };
                t.done().map(|_| tie)
            })?,
        };
        d.done()?;
        descriptor
    };
    let execution = {
        let mut e = Fields::of(f.need("execution")?)?;
        let execution = Execution {
            schema: string(e.need("schema")?)?,
            backend: string(e.need("backend")?)?,
            dtype: string(e.need("dtype")?)?,
            layout: string(e.need("layout")?)?,
            max_batch_rows: integer(e.need("max_batch_rows")?)?,
            max_activation_values: integer(e.need("max_activation_values")?)?,
        };
        e.done()?;
        execution
    };
    let base = f.option("base").map(|v| {
        let mut b = Fields::of(v)?;
        let base = Dependency { model_id: string(b.need("model_id")?)? };
        b.done().map(|_| base)
    }).transpose()?;
    let mut objects = match f.need("objects")? {
        Value::Object(members) => members.iter().map(|(key, v)| {
            let mut r = Fields::of(v)?;
            let record = ObjectRecord { file: string(r.need("file")?)?, sha256: string(r.need("sha256")?)?, bytes: integer(r.need("bytes")?)?, parameters: summary(r.need("parameters")?)? };
            r.done().map(|_| (key.to_string(), record))
        }).collect::<Result<Vec<_>, _>>()?,
        _ => return Err(MALFORMED),
    };
    objects.sort_by(|a, b| a.0.cmp(&b.0));
    let sources = list(f.need("sources")?, |v| {
        let mut s = Fields::of(v)?;
        let source = Source { identifier: string(s.need("identifier")?)?, sha256: s.option("sha256").map(string).transpose()?, licence: string(s.need("licence")?)?, notices: strings(s.need("notices")?)? };
        s.done().map(|_| source)
    })?;
    let history = list(f.need("history")?, change)?;
    let changes = list(f.need("changes")?, change)?;
    f.done()?;
    Ok(Manifest { schema: "nrl.model/0".into(), kind, descriptor, execution, base, objects, sources, history, changes })
}
