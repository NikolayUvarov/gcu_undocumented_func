//! Host tests of the network interpreter (libmind/src/nn, 250): the operators on small tensors against values worked
//! out by hand; and, when MIND_DICTATE_MODEL names a converted model and MIND_DICTATE_REFERENCE the directory
//! scripts/voice_dictate/reference.py wrote, the encoder against onnxruntime on the same features.
extern crate alloc;
#[path = "../libmind/src/nn/mod.rs"]
mod nn;
#[path = "../libmind/src/voice/math.rs"]
pub mod math;
mod voice { pub use super::math; }

use nn::{Data, Tensor};

fn f(shape: &[usize], v: &[f32]) -> Tensor { Tensor::f32(shape.to_vec(), v.to_vec()) }
fn i(shape: &[usize], v: &[i64]) -> Tensor { Tensor::i64(shape.to_vec(), v.to_vec()) }

#[test]
fn model_files_are_checked() {
    assert!(nn::Model::parse(b"not a model at all, not at all", true).is_err());
}

fn node(attrs: Vec<(&str, nn::Attr)>) -> nn::Node { nn::Node { op: 0, inputs: vec![], outputs: vec![], freed: vec![], attrs: attrs.into_iter().map(|(n, a)| (n.to_string(), a)).collect() } }
fn ints_attr(v: &[i64]) -> nn::Attr { nn::Attr::Ints(v.to_vec()) }
fn run(op: &str, n: &nn::Node, inputs: &[&Tensor]) -> Vec<Tensor> { nn::ops::run(op, n, &inputs.iter().map(|t| Some(t.view())).collect::<Vec<_>>()).unwrap() }
fn floats_of(t: &Tensor) -> &[f32] { match &t.data { Data::F32(v) => v, other => panic!("{:?}", other) } }
fn close(a: &[f32], b: &[f32]) -> bool { a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 1e-5 * (1.0 + y.abs())) }

#[test]
fn element_wise_with_broadcasting() {
    let none = node(vec![]);
    let a = f(&[2, 3], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let row = f(&[3], &[10.0, 20.0, 30.0]);
    let col = f(&[2, 1], &[100.0, 200.0]);
    assert_eq!(floats_of(&run("Add", &none, &[&a, &row])[0]), [11.0, 22.0, 33.0, 14.0, 25.0, 36.0]);
    let out = run("Mul", &none, &[&col, &row]);
    assert_eq!(out[0].shape, [2, 3]);
    assert_eq!(floats_of(&out[0]), [1000.0, 2000.0, 3000.0, 2000.0, 4000.0, 6000.0]);
    assert!(close(floats_of(&run("Pow", &none, &[&a, &f(&[], &[0.5])])[0]), &[1.0, 1.4142135, 1.7320508, 2.0, 2.236068, 2.4494898]));
    let cond = Tensor { shape: vec![3], data: Data::Bool(vec![true, false, true]) };
    assert_eq!(floats_of(&run("Where", &none, &[&cond, &a, &f(&[], &[0.0])])[0]), [1.0, 0.0, 3.0, 4.0, 0.0, 6.0]);
    assert!(close(floats_of(&run("Sigmoid", &none, &[&f(&[2], &[0.0, 2.0])])[0]), &[0.5, 0.880797]));
    assert!(close(floats_of(&run("Tanh", &none, &[&f(&[2], &[0.5, -3.0])])[0]), &[0.46211716, -0.9950548]));
    assert!(close(floats_of(&run("Atan", &none, &[&f(&[3], &[0.5, -3.0, 20.0])])[0]), &[0.4636476, -1.2490458, 1.5208379]));
    let soft = run("Softmax", &node(vec![("axis", ints_attr(&[-1]))]), &[&f(&[1, 3], &[1.0, 2.0, 3.0])]);
    assert!(close(floats_of(&soft[0]), &[0.09003057, 0.24472847, 0.66524096]));
}

#[test]
fn shapes() {
    let none = node(vec![]);
    let x = f(&[2, 3, 4], &(0..24).map(|v| v as f32).collect::<Vec<_>>());
    assert_eq!(run("Reshape", &none, &[&x, &i(&[2], &[0, -1])])[0].shape, [2, 12]);
    let t = run("Transpose", &node(vec![("perm", ints_attr(&[2, 0, 1]))]), &[&x]);
    assert_eq!(t[0].shape, [4, 2, 3]);
    assert_eq!(floats_of(&t[0])[..6], [0.0, 4.0, 8.0, 12.0, 16.0, 20.0]);
    // Slice with a negative step and Gather with a negative index.
    let s = run("Slice", &none, &[&x, &i(&[1], &[-1]), &i(&[1], &[i64::MIN]), &i(&[1], &[2]), &i(&[1], &[-2])]);
    assert_eq!(s[0].shape, [2, 3, 2]);
    assert_eq!(floats_of(&s[0])[..2], [3.0, 1.0]);
    let g = run("Gather", &node(vec![("axis", ints_attr(&[1]))]), &[&x, &i(&[], &[-1])]);
    assert_eq!(g[0].shape, [2, 4]);
    assert_eq!(floats_of(&g[0]), [8.0, 9.0, 10.0, 11.0, 20.0, 21.0, 22.0, 23.0]);
    let c = run("Concat", &node(vec![("axis", ints_attr(&[-1]))]), &[&f(&[2, 1], &[1.0, 2.0]), &f(&[2, 2], &[3.0, 4.0, 5.0, 6.0])]);
    assert_eq!(floats_of(&c[0]), [1.0, 3.0, 4.0, 2.0, 5.0, 6.0]);
    assert_eq!(floats_of(&run("Tile", &none, &[&f(&[2], &[1.0, 2.0]), &i(&[1], &[3])])[0]), [1.0, 2.0, 1.0, 2.0, 1.0, 2.0]);
    assert_eq!(run("Expand", &none, &[&f(&[3, 1], &[1.0, 2.0, 3.0]), &i(&[2], &[1, 2])])[0].shape, [3, 2]);
    assert_eq!(run("Range", &none, &[&i(&[], &[10]), &i(&[], &[3]), &i(&[], &[-3])])[0].data, Data::I64(vec![10, 7, 4]));
    let m = run("ReduceMean", &node(vec![("axes", ints_attr(&[2])), ("keepdims", ints_attr(&[1]))]), &[&x]);
    assert_eq!((m[0].shape.clone(), floats_of(&m[0])[..2].to_vec()), (vec![2, 3, 1], vec![1.5, 5.5]));
    let scattered = run("ScatterND", &none, &[&f(&[4], &[0.0; 4]), &i(&[2, 1], &[3, 1]), &f(&[2], &[9.0, 8.0])]);
    assert_eq!(floats_of(&scattered[0]), [0.0, 8.0, 0.0, 9.0]);
}

#[test]
fn products_and_quantization() {
    let none = node(vec![]);
    // A batch of two [2, 3] x one [3, 2].
    let a = f(&[2, 2, 3], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let b = f(&[3, 2], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let p = run("MatMul", &none, &[&a, &b]);
    assert_eq!(p[0].shape, [2, 2, 2]);
    assert_eq!(floats_of(&p[0]), [22.0, 28.0, 49.0, 64.0, 1.0, 2.0, 3.0, 4.0]);
    // ONNX's own example for DynamicQuantizeLinear.
    let q = run("DynamicQuantizeLinear", &none, &[&f(&[6], &[0.0, 2.0, -3.0, -2.5, 1.34, 0.5])]);
    assert_eq!(q[0].data, Data::U8(vec![153, 255, 0, 26, 221, 179]));
    assert!(close(floats_of(&q[1]), &[0.019607844]));
    assert_eq!(q[2].data, Data::U8(vec![153]));
    // u8 x i8 with zero points: (a - 2) . (b - (-1)).
    let ua = Tensor { shape: vec![1, 2], data: Data::U8(vec![3, 255]) };
    let ib = Tensor { shape: vec![2, 1], data: Data::I8(vec![-128, 127]) };
    let (za, zb) = (Tensor { shape: vec![], data: Data::U8(vec![2]) }, Tensor { shape: vec![], data: Data::I8(vec![-1]) });
    let r = run("MatMulInteger", &none, &[&ua, &ib, &za, &zb]);
    assert_eq!(r[0].data, Data::I32(vec![(3 - 2) * (-128 + 1) + (255 - 2) * (127 + 1)]));
    // A depthwise 1-D convolution with padding, as the conformer modules use it.
    let conv = node(vec![("group", ints_attr(&[2])), ("kernel_shape", ints_attr(&[3])), ("pads", ints_attr(&[1, 1]))]);
    let x = f(&[1, 2, 4], &[1.0, 2.0, 3.0, 4.0, 1.0, 1.0, 1.0, 1.0]);
    let w = f(&[2, 1, 3], &[1.0, 0.0, -1.0, 1.0, 1.0, 1.0]);
    let y = run("Conv", &conv, &[&x, &w, &f(&[2], &[0.0, 10.0])]);
    assert_eq!(y[0].shape, [1, 2, 4]);
    assert_eq!(floats_of(&y[0]), [-2.0, -2.0, -2.0, 3.0, 12.0, 13.0, 13.0, 12.0]);
}

// The model file read into 8-byte aligned memory (its weights are read in place).
fn load(path: &str) -> Vec<u64> {
    let bytes = std::fs::read(path).unwrap();
    let mut words = vec![0u64; bytes.len().div_ceil(8) + 1];
    unsafe { std::slice::from_raw_parts_mut(words.as_mut_ptr() as *mut u8, bytes.len()) }.copy_from_slice(&bytes);
    words.truncate(bytes.len().div_ceil(8));
    words.push(bytes.len() as u64);
    words
}
fn bytes(words: &[u64]) -> &[u8] { unsafe { std::slice::from_raw_parts(words.as_ptr() as *const u8, *words.last().unwrap() as usize) } }

fn floats(path: String) -> Vec<f32> { std::fs::read(path).unwrap().chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect() }

// Greedy search over the decoder and joiner, as reference.py does over onnxruntime's: the text.
fn greedy(model: &nn::Model, enc: &[f32], frames: usize, dim: usize) -> String {
    let mut context = [0i64, 0];
    let decode = |context: &[i64; 2]| model.run("decoder", vec![i(&[1, 2], context)]).unwrap().remove(0);
    let mut dec = decode(&context);
    let mut found = Vec::new();
    for t in 0..frames {
        let logit = model.run("joiner", vec![f(&[1, dim], &enc[t * dim..(t + 1) * dim]), dec.clone()]).unwrap().remove(0);
        let Data::F32(l) = &logit.data else { panic!() };
        let y = l.iter().enumerate().fold((0, f32::NEG_INFINITY), |b, (k, &v)| if v > b.1 { (k, v) } else { b }).0;
        if y != 0 { found.push(y); context = [context[1], y as i64]; dec = decode(&context); }
    }
    found.iter().map(|&y| model.tokens[y].as_str()).collect::<String>().replace('\u{2581}', " ").trim().to_string()
}

#[test]
fn clips_against_onnxruntime() {
    // MIND_DICTATE_REFERENCE: a directory of clips, each a directory reference.py wrote. The encoder's output differs
    // from onnxruntime's by the noise of dynamic int8 quantization: its exp differs from ours by about 1e-5, and a value
    // near a rounding step lands on the other side. So a rare word may come out otherwise: at most 2 % of the words.
    let (Ok(path), Ok(reference)) = (std::env::var("MIND_DICTATE_MODEL"), std::env::var("MIND_DICTATE_REFERENCE")) else {
        println!("skipped: MIND_DICTATE_MODEL and MIND_DICTATE_REFERENCE are not set");
        return;
    };
    let words = load(&path);
    let model = nn::Model::parse(bytes(&words), true).unwrap();
    let mut clips: Vec<_> = std::fs::read_dir(&reference).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("text.txt").exists()).collect();
    clips.sort();
    let (mut audio, mut spent, mut words, mut differ) = (0usize, std::time::Duration::ZERO, 0usize, 0usize);
    for clip in &clips {
        let dir = clip.display().to_string();
        let features = floats(format!("{}/features.f32", dir));
        let frames = features.len() / 80;
        let start = std::time::Instant::now();
        let out = model.run("encoder", vec![f(&[1, frames, 80], &features), i(&[1], &[frames as i64])]).unwrap();
        let Data::F32(got) = &out[0].data else { panic!() };
        let text = greedy(&model, got, out[0].shape[1], out[0].shape[2]);
        spent += start.elapsed();
        audio += frames;
        let want = floats(format!("{}/encoder_out.f32", dir));
        let mean = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).sum::<f32>() / got.len() as f32;
        let scale = want.iter().map(|a| a.abs()).sum::<f32>() / want.len() as f32;
        println!("{}: {} frames, encoder mean difference {:.3} of {:.3}; {}", clip.file_name().unwrap().to_string_lossy(), frames, mean, scale, text);
        assert!(mean < 0.1 * scale, "{} against {}", mean, scale);
        let want_text = std::fs::read_to_string(format!("{}/text.txt", dir)).unwrap();
        let (a, b): (Vec<&str>, Vec<&str>) = (text.split_whitespace().collect(), want_text.split_whitespace().collect());
        let errors = edit_distance(&a, &b);
        if errors > 0 { println!("  onnxruntime: {}", want_text.trim()); }
        (words, differ) = (words + b.len(), differ + errors);
    }
    println!("{} clips, {:.1} s of speech in {:.1} s; {} of {} words differ from onnxruntime's", clips.len(), audio as f32 / 100.0, spent.as_secs_f32(), differ, words);
    assert!(differ * 50 <= words, "more than 2 % of the words differ");
}

// Word-level edit distance.
fn edit_distance(a: &[&str], b: &[&str]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let next = (row[j + 1] + 1).min(row[j] + 1).min(prev + (x != y) as usize);
            prev = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

#[test]
fn first_departure_from_onnxruntime() {
    // Debugging aid: MIND_DICTATE_DUMP names the directory scripts/voice_dictate/dump.py wrote for the first 100 frames.
    let (Ok(model), Ok(reference), Ok(dump)) = (std::env::var("MIND_DICTATE_MODEL"), std::env::var("MIND_DICTATE_REFERENCE"), std::env::var("MIND_DICTATE_DUMP")) else { return };
    let words = load(&model);
    let model = nn::Model::parse(bytes(&words), false).unwrap();
    let features: Vec<f32> = std::fs::read(format!("{}/features.f32", reference)).unwrap().chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).take(100 * 80).collect();
    let mut index = std::collections::HashMap::new();
    for line in std::fs::read_to_string(format!("{}/index.txt", dump)).unwrap().lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        index.insert(parts[0].to_string(), (parts[1].to_string(), parts[2].to_string(), parts[3].to_string()));
    }
    let mut reported = 0;
    let mut checked = 0;
    model.run_watched("encoder", vec![f(&[1, 100, 80], &features), i(&[1], &[100])], &mut |name, t| {
        let Some((dtype, shape, file)) = index.get(name) else { return };
        checked += 1;
        let raw = std::fs::read(format!("{}/{}", dump, file)).unwrap();
        let want_shape: Vec<usize> = if shape.is_empty() { vec![] } else { shape.split(',').map(|d| d.parse().unwrap()).collect() };
        let bad = |what: String| { if reported < 5 { println!("{} ({}): {}", name, dtype, what); } };
        if want_shape != t.shape { bad(format!("shape {:?} against {:?}", t.shape, want_shape)); reported += 1; return; }
        let diff = match (&t.data, dtype.as_str()) {
            (Data::F32(v), "float32") => { let w: Vec<f32> = raw.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect(); let scale = w.iter().map(|a| a.abs()).fold(1e-6f32, f32::max); v.iter().zip(&w).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max) / scale }
            (Data::I64(v), "int64") => { let w: Vec<i64> = raw.chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().unwrap())).collect(); if *v == w { 0.0 } else { 1.0 } }
            (Data::I32(v), "int32") => { let w: Vec<i32> = raw.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect(); let scale = w.iter().map(|a| a.abs()).max().unwrap_or(1).max(1) as f32; v.iter().zip(&w).map(|(a, b)| (a - b).abs()).max().unwrap_or(0) as f32 / scale }
            (Data::U8(v), "uint8") => { let n = v.iter().zip(&raw).filter(|(a, b)| a != b).count(); if n > 0 { bad(format!("{} of {} bytes differ", n, v.len())); } 0.0 }
            (Data::Bool(v), "bool") => { if v.iter().zip(&raw).all(|(a, b)| *a as u8 == *b) { 0.0 } else { 1.0 } }
            (d, w) => { bad(format!("type {:?} against {}", std::mem::discriminant(d), w)); reported += 1; return; }
        };
        if diff > std::env::var("MIND_DICTATE_TOLERANCE").ok().and_then(|t| t.parse().ok()).unwrap_or(1e-3) { bad(format!("relative difference {}", diff)); reported += 1; }
    }).unwrap();
    println!("checked {} values, {} departed", checked, reported);
}
