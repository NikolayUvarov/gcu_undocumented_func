//! The encoders nnd checks a package with (262-APP-0112, docs/nn/runtime.md section 2): an object file in the writer
//! profile `nrl.serde-json-pretty-object/0` (`serde_json` 1.0.151's pretty form of the typed record, floats as `zmij`
//! 1.0.23 writes them) and the manifest's canonical form, whose SHA-256 is the model ID (every key sorted, no space).
use crate::model::*;
use alloc::vec::Vec;

/// The name of the object writer profile this crate reproduces.
pub const OBJECT_PROFILE: &str = "nrl.serde-json-pretty-object/0";

/// A record as `serde_json::to_value` would hold it: fields in their declared order, `None` as null.
pub enum Json<'a> { Null, Uint(u64), Float(f64), Str(&'a str), Array(Vec<Json<'a>>), Object(Vec<(&'a str, Json<'a>)>) }

fn floats(values: &[f64]) -> Json<'_> { Json::Array(values.iter().map(|&v| Json::Float(v)).collect()) }
fn uints(values: &[usize]) -> Json<'_> { Json::Array(values.iter().map(|&v| Json::Uint(v as u64)).collect()) }
fn strs(values: &[alloc::string::String]) -> Json<'_> { Json::Array(values.iter().map(|v| Json::Str(v)).collect()) }
fn option<'a, T>(value: &'a Option<T>, f: impl FnOnce(&'a T) -> Json<'a>) -> Json<'a> { value.as_ref().map_or(Json::Null, f) }
fn uint(v: usize) -> Json<'static> { Json::Uint(v as u64) }

/// An object file's record: `kind` before `data`, each record's fields in their declared order.
pub fn object(data: &ObjectData) -> Json<'_> {
    let (kind, data) = match data {
        ObjectData::Affine(a) => ("affine", Json::Object(alloc::vec![("schema", Json::Str(&a.schema)), ("shape", uints(&a.shape)), ("weights", floats(&a.weights)), ("bias", floats(&a.bias))])),
        ObjectData::Gated(a) => ("gated", Json::Object(alloc::vec![
            ("descriptor", Json::Object(alloc::vec![
                ("kind", Json::Str(&a.descriptor.kind)),
                ("source_activation", Json::Str(&a.descriptor.source_activation)),
                ("dtype", Json::Str(&a.descriptor.dtype)),
                ("layout", Json::Str(&a.descriptor.layout)),
                ("state", Json::Str(&a.descriptor.state)),
                ("parameter_sharing", Json::Str(&a.descriptor.parameter_sharing)),
            ])),
            ("bias", option(&a.bias, |b| Json::Object(alloc::vec![("gate", floats(&b.gate)), ("up", floats(&b.up)), ("down", floats(&b.down))]))),
            ("schema", Json::Str(&a.schema)),
            ("representation", Json::Str(&a.representation)),
            ("shape", uints(&a.shape)),
            ("gate", floats(&a.gate)),
            ("up", floats(&a.up)),
            ("down", floats(&a.down)),
            ("spline", option(&a.spline, |s| Json::Object(alloc::vec![("knots", uint(s.knots)), ("low", floats(&s.low)), ("high", floats(&s.high)), ("values", floats(&s.values))]))),
            ("selection", option(&a.selection, |s| Json::Object(alloc::vec![("mapping", Json::Str(&s.mapping)), ("original_channels", uint(s.original_channels)), ("indices", uints(&s.indices)), ("method", Json::Str(&s.method))]))),
            ("factors", option(&a.factors, |f| Json::Object(alloc::vec![
                ("rank", uint(f.rank)),
                ("gate_left", floats(&f.gate_left)),
                ("gate_right", floats(&f.gate_right)),
                ("up_left", floats(&f.up_left)),
                ("up_right", floats(&f.up_right)),
                ("down_left", floats(&f.down_left)),
                ("down_right", floats(&f.down_right)),
            ]))),
        ])),
    };
    Json::Object(alloc::vec![("kind", Json::Str(kind)), ("data", data)])
}

fn summary(s: &Summary) -> Json<'_> {
    Json::Object(alloc::vec![
        ("kind", Json::Str(&s.kind)),
        ("schema", Json::Str(&s.schema)),
        ("input_width", uint(s.input_width)),
        ("output_width", uint(s.output_width)),
        ("representation", Json::Str(&s.representation)),
        ("channels", option(&s.channels, |&c| uint(c))),
        ("rank", option(&s.rank, |&r| uint(r))),
        ("knots", option(&s.knots, |&k| uint(k))),
        ("numeric_values", uint(s.numeric_values)),
        ("trainable_values", uint(s.trainable_values)),
        ("bias_values", uint(s.bias_values)),
    ])
}

fn operation(op: &Operation) -> Json<'_> {
    Json::Object(match op {
        Operation::Affine { input, object } => alloc::vec![("operator", Json::Str("affine")), ("input", Json::Str(input)), ("object", Json::Str(object))],
        Operation::Gated { input, object } => alloc::vec![("operator", Json::Str("gated")), ("input", Json::Str(input)), ("object", Json::Str(object))],
        Operation::Silu { input } => alloc::vec![("operator", Json::Str("silu")), ("input", Json::Str(input))],
        Operation::Relu { input } => alloc::vec![("operator", Json::Str("relu")), ("input", Json::Str(input))],
        Operation::Add { left, right } => alloc::vec![("operator", Json::Str("add")), ("left", Json::Str(left)), ("right", Json::Str(right))],
    })
}

fn change(c: &Change) -> Json<'_> {
    Json::Object(alloc::vec![
        ("object", Json::Str(&c.object)),
        ("operation", Json::Str(&c.operation)),
        ("base_model_id", Json::Str(&c.base_model_id)),
        ("before_sha256", Json::Str(&c.before_sha256)),
        ("after_sha256", Json::Str(&c.after_sha256)),
        ("parameters", summary(&c.parameters)),
        ("reason", Json::Str(&c.reason)),
        ("source_identifier", Json::Str(&c.source_identifier)),
    ])
}

/// The manifest as `nrl-lab`'s `Manifest` serializes.
pub fn manifest(m: &Manifest) -> Json<'_> {
    let d = &m.descriptor;
    let g = &d.configuration;
    let e = &m.execution;
    Json::Object(alloc::vec![
        ("schema", Json::Str(&m.schema)),
        ("kind", Json::Str(&m.kind)),
        ("descriptor", Json::Object(alloc::vec![
            ("family", Json::Str(&d.family)),
            ("adapter", Json::Str(&d.adapter)),
            ("adapter_version", uint(d.adapter_version)),
            ("configuration", Json::Object(alloc::vec![
                ("schema", Json::Str(&g.schema)),
                ("input_width", uint(g.input_width)),
                ("nodes", Json::Array(g.nodes.iter().map(|n| Json::Object(alloc::vec![("id", Json::Str(&n.id)), ("op", operation(&n.op))])).collect())),
                ("output", Json::Str(&g.output)),
            ])),
            ("preprocessor", Json::Str(&d.preprocessor)),
            ("tasks", strs(&d.tasks)),
            ("state", Json::Str(&d.state)),
            ("tied_objects", Json::Array(d.tied_objects.iter().map(|t| Json::Object(alloc::vec![("object", Json::Str(&t.object)), ("nodes", strs(&t.nodes))])).collect())),
        ])),
        ("execution", Json::Object(alloc::vec![
            ("schema", Json::Str(&e.schema)),
            ("backend", Json::Str(&e.backend)),
            ("dtype", Json::Str(&e.dtype)),
            ("layout", Json::Str(&e.layout)),
            ("max_batch_rows", uint(e.max_batch_rows)),
            ("max_activation_values", uint(e.max_activation_values)),
        ])),
        ("base", option(&m.base, |b| Json::Object(alloc::vec![("model_id", Json::Str(&b.model_id))]))),
        ("objects", Json::Object(m.objects.iter().map(|(k, r)| (k.as_str(), Json::Object(alloc::vec![
            ("file", Json::Str(&r.file)),
            ("sha256", Json::Str(&r.sha256)),
            ("bytes", uint(r.bytes)),
            ("parameters", summary(&r.parameters)),
        ]))).collect())),
        ("sources", Json::Array(m.sources.iter().map(|s| Json::Object(alloc::vec![
            ("identifier", Json::Str(&s.identifier)),
            ("sha256", option(&s.sha256, |h| Json::Str(h))),
            ("licence", Json::Str(&s.licence)),
            ("notices", strs(&s.notices)),
        ])).collect())),
        ("history", Json::Array(m.history.iter().map(change).collect())),
        ("changes", Json::Array(m.changes.iter().map(change).collect())),
    ])
}

// A string as `serde_json` escapes it: `"`, `\` and the control characters; everything else as it is.
fn string(out: &mut Vec<u8>, s: &str) {
    out.push(b'"');
    for &b in s.as_bytes() {
        match b {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x09 => out.extend_from_slice(b"\\t"),
            0x0A => out.extend_from_slice(b"\\n"),
            0x0C => out.extend_from_slice(b"\\f"),
            0x0D => out.extend_from_slice(b"\\r"),
            0x00..=0x1F => { const HEX: &[u8; 16] = b"0123456789abcdef"; out.extend_from_slice(&[b'\\', b'u', b'0', b'0', HEX[(b >> 4) as usize], HEX[(b & 15) as usize]]); }
            _ => out.push(b),
        }
    }
    out.push(b'"');
}

fn scalar(out: &mut Vec<u8>, value: &Json) {
    match value {
        Json::Null => out.extend_from_slice(b"null"),
        Json::Uint(n) => { let mut b = [0u8; 20]; let mut i = b.len(); let mut n = *n; loop { i -= 1; b[i] = b'0' + (n % 10) as u8; n /= 10; if n == 0 { break; } } out.extend_from_slice(&b[i..]); }
        Json::Float(v) if v.is_finite() => out.extend_from_slice(zmij::Buffer::new().format_finite(*v).as_bytes()),
        Json::Float(_) => out.extend_from_slice(b"null"),
        Json::Str(s) => string(out, s),
        Json::Array(_) | Json::Object(_) => unreachable!(),
    }
}

fn indent(out: &mut Vec<u8>, depth: usize) { for _ in 0..depth { out.extend_from_slice(b"  "); } }

/// `serde_json::to_vec_pretty`: two spaces, LF, `": "` after a key, no final newline.
pub fn pretty(out: &mut Vec<u8>, value: &Json, depth: usize) {
    match value {
        Json::Array(items) if items.is_empty() => out.extend_from_slice(b"[]"),
        Json::Object(members) if members.is_empty() => out.extend_from_slice(b"{}"),
        Json::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() { out.extend_from_slice(if i == 0 { b"\n" } else { b",\n" }); indent(out, depth + 1); pretty(out, item, depth + 1); }
            out.push(b'\n'); indent(out, depth); out.push(b']');
        }
        Json::Object(members) => {
            out.push(b'{');
            for (i, (key, item)) in members.iter().enumerate() {
                out.extend_from_slice(if i == 0 { b"\n" } else { b",\n" });
                indent(out, depth + 1); string(out, key); out.extend_from_slice(b": "); pretty(out, item, depth + 1);
            }
            out.push(b'\n'); indent(out, depth); out.push(b'}');
        }
        _ => scalar(out, value),
    }
}

/// `nrl-lab`'s `canonical_compact`: every object's keys sorted by their bytes, no white space.
pub fn canonical(out: &mut Vec<u8>, value: &Json) {
    match value {
        Json::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() { if i > 0 { out.push(b','); } canonical(out, item); }
            out.push(b']');
        }
        Json::Object(members) => {
            let mut order: Vec<usize> = (0..members.len()).collect();
            order.sort_by(|&a, &b| members[a].0.as_bytes().cmp(members[b].0.as_bytes()));
            out.push(b'{');
            for (i, &m) in order.iter().enumerate() { if i > 0 { out.push(b','); } string(out, members[m].0); out.push(b':'); canonical(out, &members[m].1); }
            out.push(b'}');
        }
        _ => scalar(out, value),
    }
}

/// The object file `data` is written as in the profile.
pub fn object_bytes(data: &ObjectData) -> Vec<u8> { let mut out = Vec::new(); pretty(&mut out, &object(data), 0); out }
