//! The ONNX operators the speech models use (opset 13 semantics), on `View`s; every one makes new tensors.
use super::{op_error, DType, Data, Elems, Node, Result, Tensor, View};
use crate::voice::math;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

pub(crate) fn run(op: &str, node: &Node, inputs: &[Option<View<'_>>]) -> Result<Vec<Tensor>> {
    let input = |i: usize| -> Result<View<'_>> { inputs.get(i).copied().flatten().ok_or_else(|| op_error(format!("input {} missing", i))) };
    let one = |t: Tensor| Ok(vec![t]);
    match op {
        "Add" | "Sub" | "Mul" | "Div" | "Pow" | "Max" => one(arith(op, input(0)?, input(1)?)?),
        "Equal" | "GreaterOrEqual" => one(compare(op, input(0)?, input(1)?)?),
        "Where" => one(where_(input(0)?, input(1)?, input(2)?)?),
        "Abs" | "Neg" | "Sign" | "Exp" | "Log" | "Sin" | "Cos" | "Atan" | "Tanh" | "Sigmoid" | "Relu" => one(unary(op, input(0)?)?),
        "Clip" => one(clip(input(0)?, inputs.get(1).copied().flatten(), inputs.get(2).copied().flatten())?),
        "Cast" => one(cast(input(0)?, node.int("to").ok_or_else(|| op_error("to"))?)?),
        "Shape" => { let x = input(0)?; one(Tensor::i64(vec![x.shape.len()], x.shape.iter().map(|&d| d as i64).collect())) }
        "Reshape" => one(reshape(input(0)?, &ints(input(1)?)?, node.int("allowzero").unwrap_or(0) != 0)?),
        "Unsqueeze" => one(unsqueeze(input(0)?, &ints(input(1)?)?)?),
        "Squeeze" => one(squeeze(input(0)?, inputs.get(1).copied().flatten().map(ints).transpose()?)?),
        "Concat" => one(concat(&inputs.iter().map(|i| i.ok_or_else(|| op_error("an input"))).collect::<Result<Vec<_>>>()?, node.int("axis").unwrap_or(0))?),
        "Gather" => one(gather(input(0)?, input(1)?, node.int("axis").unwrap_or(0))?),
        "GatherElements" => one(gather_elements(input(0)?, input(1)?, node.int("axis").unwrap_or(0))?),
        "Slice" => one(slice(input(0)?, &ints(input(1)?)?, &ints(input(2)?)?, inputs.get(3).copied().flatten().map(ints).transpose()?, inputs.get(4).copied().flatten().map(ints).transpose()?)?),
        "Transpose" => { let x = input(0)?; let perm: Vec<usize> = match node.ints("perm") { Some(p) => p.iter().map(|&a| a as usize).collect(), None => (0..x.shape.len()).rev().collect() }; one(transpose(x, &perm)?) }
        "Expand" => one(expand(input(0)?, &ints(input(1)?)?)?),
        "Tile" => one(tile(input(0)?, &ints(input(1)?)?)?),
        "Range" => one(range(input(0)?, input(1)?, input(2)?)?),
        "ConstantOfShape" => one(constant_of_shape(&ints(input(0)?)?, node)?),
        "ScatterND" => one(scatter_nd(input(0)?, input(1)?, input(2)?)?),
        "ReduceMean" | "ReduceSum" => {
            let axes = match node.ints("axes") { Some(a) => Some(a.to_vec()), None => inputs.get(1).copied().flatten().map(ints).transpose()? };
            one(reduce(op, input(0)?, axes, node.int("keepdims").unwrap_or(1) != 0)?)
        }
        "Softmax" => one(softmax(input(0)?, node.int("axis").unwrap_or(-1))?),
        "MatMul" => one(matmul(input(0)?, input(1)?)?),
        "MatMulInteger" => one(matmul_integer(input(0)?, input(1)?, inputs.get(2).copied().flatten(), inputs.get(3).copied().flatten())?),
        "DynamicQuantizeLinear" => dynamic_quantize(input(0)?),
        "DequantizeLinear" => one(dequantize(input(0)?, input(1)?, inputs.get(2).copied().flatten())?),
        "Conv" => one(conv(input(0)?, input(1)?, inputs.get(2).copied().flatten(), node)?),
        _ => Err(op_error("not supported")),
    }
}

// ---- Helpers ----

fn ints(v: View<'_>) -> Result<Vec<i64>> {
    match v.data { Elems::I64(x) => Ok(x.to_vec()), Elems::I32(x) => Ok(x.iter().map(|&a| a as i64).collect()), _ => Err(op_error("expected integers")) }
}

fn f32s<'a>(v: &View<'a>) -> Result<&'a [f32]> { match v.data { Elems::F32(x) => Ok(x), _ => Err(op_error(format!("expected f32, got {:?}", v.data.dtype()))) } }

fn axis(a: i64, rank: usize) -> Result<usize> {
    let r = rank as i64;
    let a = if a < 0 { a + r } else { a };
    if a < 0 || a >= r.max(1) { return Err(op_error(format!("axis {} of rank {}", a, rank))); }
    Ok(a as usize)
}

fn strides(shape: &[usize]) -> Vec<usize> {
    let mut s = vec![1; shape.len()];
    for i in (0..shape.len().saturating_sub(1)).rev() { s[i] = s[i + 1] * shape[i + 1]; }
    s
}

fn broadcast(a: &[usize], b: &[usize]) -> Result<Vec<usize>> {
    let n = a.len().max(b.len());
    (0..n).map(|i| {
        let x = if i + a.len() >= n { a[i + a.len() - n] } else { 1 };
        let y = if i + b.len() >= n { b[i + b.len() - n] } else { 1 };
        if x == y || y == 1 { Ok(x) } else if x == 1 { Ok(y) } else { Err(op_error(format!("shapes {:?} and {:?} do not broadcast", a, b))) }
    }).collect()
}

// For each element of `out`, the index into a tensor of shape `shape` broadcast to it.
fn index_map(shape: &[usize], out: &[usize]) -> Vec<usize> {
    let total: usize = out.iter().product();
    if shape.iter().product::<usize>() == total && shape.len() <= out.len() && shape.iter().rev().zip(out.iter().rev()).all(|(a, b)| a == b) { return (0..total).collect(); }
    let rank = out.len();
    let mut s = vec![0usize; rank];
    let own = strides(shape);
    for (i, (&d, &st)) in shape.iter().zip(&own).enumerate() { if d != 1 { s[rank - shape.len() + i] = st; } }
    let mut map = Vec::with_capacity(total);
    let mut idx = vec![0usize; rank];
    let mut at = 0usize;
    for _ in 0..total {
        map.push(at);
        for d in (0..rank).rev() {
            idx[d] += 1;
            at += s[d];
            if idx[d] < out[d] { break; }
            at -= s[d] * idx[d];
            idx[d] = 0;
        }
    }
    map
}

// Two inputs broadcast together, an element function over them.
fn zip_with<A: Copy, B: Copy, C>(a: &[A], sa: &[usize], b: &[B], sb: &[usize], f: impl Fn(A, B) -> C) -> Result<(Vec<usize>, Vec<C>)> {
    let shape = broadcast(sa, sb)?;
    let total: usize = shape.iter().product();
    if a.len() == total && b.len() == total { return Ok((shape, a.iter().zip(b).map(|(&x, &y)| f(x, y)).collect())); }
    if b.len() == 1 && a.len() == total { let y = b[0]; return Ok((shape, a.iter().map(|&x| f(x, y)).collect())); }
    if a.len() == 1 && b.len() == total { let x = a[0]; return Ok((shape, b.iter().map(|&y| f(x, y)).collect())); }
    // b repeats along the leading dimensions (a row vector, say): no index map.
    if a.len() == total && !b.is_empty() && total % b.len() == 0 && sb.iter().rev().zip(shape.iter().rev()).all(|(x, y)| x == y) {
        return Ok((shape, a.chunks(b.len()).flat_map(|row| row.iter().zip(b).map(|(&x, &y)| f(x, y)).collect::<Vec<_>>()).collect()));
    }
    let (ma, mb) = (index_map(sa, &shape), index_map(sb, &shape));
    Ok((shape, ma.iter().zip(&mb).map(|(&i, &j)| f(a[i], b[j])).collect()))
}

// ---- Element-wise ----

fn powf(x: f32, y: f32) -> f32 {
    if y == 2.0 { return x * x; }
    if y == 1.0 { return x; }
    if y == 0.5 { return sqrtf(x); }
    if y == -0.5 { return 1.0 / sqrtf(x); }
    if y == 0.0 { return 1.0; }
    if x > 0.0 { return math::expf(y * math::lnf(x)); }
    if x == 0.0 { return if y > 0.0 { 0.0 } else { f32::INFINITY }; }
    let n = y as i64;
    if n as f32 == y { let m = math::expf(y * math::lnf(-x)); return if n % 2 == 0 { m } else { -m }; }
    f32::NAN
}

pub(crate) fn sqrtf(x: f32) -> f32 {
    if x <= 0.0 { return if x == 0.0 { 0.0 } else { f32::NAN }; }
    #[cfg(all(target_arch = "x86_64", target_feature = "sse"))]
    // SAFETY: SSE is part of the target (the float build and the host).
    unsafe { use core::arch::x86_64::*; _mm_cvtss_f32(_mm_sqrt_ss(_mm_set_ss(x))) }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse")))]
    { math::sqrt(x as f64) as f32 }
}

fn arith(op: &str, a: View<'_>, b: View<'_>) -> Result<Tensor> {
    match (a.data, b.data) {
        (Elems::F32(x), Elems::F32(y)) => {
            let (shape, v) = match op {
                "Add" => zip_with(x, a.shape, y, b.shape, |p, q| p + q),
                "Sub" => zip_with(x, a.shape, y, b.shape, |p, q| p - q),
                "Mul" => zip_with(x, a.shape, y, b.shape, |p, q| p * q),
                "Div" => zip_with(x, a.shape, y, b.shape, |p, q| p / q),
                "Pow" => zip_with(x, a.shape, y, b.shape, powf),
                _ => zip_with(x, a.shape, y, b.shape, |p: f32, q: f32| if p >= q || q.is_nan() { p } else { q }),
            }?;
            Ok(Tensor::f32(shape, v))
        }
        (Elems::I64(x), Elems::I64(y)) => {
            let (shape, v) = match op {
                "Add" => zip_with(x, a.shape, y, b.shape, |p, q| p.wrapping_add(q)),
                "Sub" => zip_with(x, a.shape, y, b.shape, |p, q| p.wrapping_sub(q)),
                "Mul" => zip_with(x, a.shape, y, b.shape, |p, q| p.wrapping_mul(q)),
                "Div" => zip_with(x, a.shape, y, b.shape, |p, q| if q == 0 { 0 } else { p.wrapping_div(q) }),
                "Max" => zip_with(x, a.shape, y, b.shape, |p: i64, q: i64| p.max(q)),
                _ => zip_with(x, a.shape, y, b.shape, |p: i64, q: i64| p.pow(q.clamp(0, 62) as u32)),
            }?;
            Ok(Tensor::i64(shape, v))
        }
        (Elems::F32(x), Elems::I64(y)) if op == "Pow" => { let (shape, v) = zip_with(x, a.shape, y, b.shape, |p, q| powf(p, q as f32))?; Ok(Tensor::f32(shape, v)) }
        (p, q) => Err(op_error(format!("{:?} and {:?}", p.dtype(), q.dtype()))),
    }
}

fn compare(op: &str, a: View<'_>, b: View<'_>) -> Result<Tensor> {
    let eq = op == "Equal";
    let (shape, v) = match (a.data, b.data) {
        (Elems::F32(x), Elems::F32(y)) => zip_with(x, a.shape, y, b.shape, |p, q| if eq { p == q } else { p >= q })?,
        (Elems::I64(x), Elems::I64(y)) => zip_with(x, a.shape, y, b.shape, |p, q| if eq { p == q } else { p >= q })?,
        (Elems::Bool(x), Elems::Bool(y)) if eq => zip_with(x, a.shape, y, b.shape, |p, q| p == q)?,
        (p, q) => return Err(op_error(format!("{:?} and {:?}", p.dtype(), q.dtype()))),
    };
    Ok(Tensor { shape, data: Data::Bool(v) })
}

fn where_(c: View<'_>, x: View<'_>, y: View<'_>) -> Result<Tensor> {
    let Elems::Bool(cond) = c.data else { return Err(op_error("the condition is not bool")) };
    let shape = broadcast(&broadcast(c.shape, x.shape)?, y.shape)?;
    let (mc, mx, my) = (index_map(c.shape, &shape), index_map(x.shape, &shape), index_map(y.shape, &shape));
    let pick = |i: usize| if cond[mc[i]] { (true, mx[i]) } else { (false, my[i]) };
    let n = mc.len();
    let data = match (x.data, y.data) {
        (Elems::F32(a), Elems::F32(b)) => Data::F32((0..n).map(|i| { let (t, j) = pick(i); if t { a[j] } else { b[j] } }).collect()),
        (Elems::I64(a), Elems::I64(b)) => Data::I64((0..n).map(|i| { let (t, j) = pick(i); if t { a[j] } else { b[j] } }).collect()),
        (Elems::Bool(a), Elems::Bool(b)) => Data::Bool((0..n).map(|i| { let (t, j) = pick(i); if t { a[j] } else { b[j] } }).collect()),
        (p, q) => return Err(op_error(format!("{:?} and {:?}", p.dtype(), q.dtype()))),
    };
    Ok(Tensor { shape, data })
}

fn atanf(x: f32) -> f32 {
    // atan x = 2 atan(x / (1 + sqrt(1 + x^2))) brings |x| below tan(pi/8); then the series.
    let x = x as f64;
    let (r, invert) = if x.abs() > 1.0 { (1.0 / x, true) } else { (x, false) };
    let h = r / (1.0 + math::sqrt(1.0 + r * r));
    let (h2, mut term, mut sum) = (h * h, h, 0.0);
    for k in 0..12 { sum += term / (2 * k + 1) as f64; term *= -h2; }
    let a = 2.0 * sum;
    (if invert { (if x > 0.0 { core::f64::consts::FRAC_PI_2 } else { -core::f64::consts::FRAC_PI_2 }) - a } else { a }) as f32
}

fn tanhf(x: f32) -> f32 {
    if x > 9.0 { return 1.0; }
    if x < -9.0 { return -1.0; }
    if x.abs() < 0.004 { return x - x * x * x / 3.0; }
    let e = math::expf(2.0 * x);
    (e - 1.0) / (e + 1.0)
}

fn unary(op: &str, x: View<'_>) -> Result<Tensor> {
    if let Elems::I64(v) = x.data {
        let f: fn(i64) -> i64 = match op { "Abs" => |a| a.abs(), "Neg" => |a| -a, "Sign" => |a| a.signum(), "Relu" => |a| a.max(0), _ => return Err(op_error("i64")) };
        return Ok(Tensor::i64(x.shape.to_vec(), v.iter().map(|&a| f(a)).collect()));
    }
    let v = f32s(&x)?;
    let f: fn(f32) -> f32 = match op {
        "Abs" => |a| a.abs(), "Neg" => |a| -a, "Sign" => |a| if a > 0.0 { 1.0 } else if a < 0.0 { -1.0 } else { 0.0 },
        "Exp" => math::expf, "Log" => math::lnf, "Sin" => |a| math::sin(a as f64) as f32, "Cos" => |a| math::cos(a as f64) as f32,
        "Atan" => atanf, "Tanh" => tanhf, "Sigmoid" => |a| 1.0 / (1.0 + math::expf(-a)), _ => |a| a.max(0.0),
    };
    Ok(Tensor::f32(x.shape.to_vec(), v.iter().map(|&a| f(a)).collect()))
}

fn clip(x: View<'_>, lo: Option<View<'_>>, hi: Option<View<'_>>) -> Result<Tensor> {
    match x.data {
        Elems::F32(v) => {
            let lo = lo.map(|l| f32s(&l).map(|s| s[0])).transpose()?.unwrap_or(f32::NEG_INFINITY);
            let hi = hi.map(|h| f32s(&h).map(|s| s[0])).transpose()?.unwrap_or(f32::INFINITY);
            Ok(Tensor::f32(x.shape.to_vec(), v.iter().map(|&a| a.max(lo).min(hi)).collect()))
        }
        Elems::I64(v) => {
            let lo = lo.map(|l| ints(l).map(|s| s[0])).transpose()?.unwrap_or(i64::MIN);
            let hi = hi.map(|h| ints(h).map(|s| s[0])).transpose()?.unwrap_or(i64::MAX);
            Ok(Tensor::i64(x.shape.to_vec(), v.iter().map(|&a| a.clamp(lo, hi)).collect()))
        }
        _ => Err(op_error("type")),
    }
}

fn cast(x: View<'_>, to: i64) -> Result<Tensor> {
    let as_f32: Vec<f32> = match x.data {
        Elems::F32(v) => v.to_vec(), Elems::U8(v) => v.iter().map(|&a| a as f32).collect(), Elems::I8(v) => v.iter().map(|&a| a as f32).collect(),
        Elems::I32(v) => v.iter().map(|&a| a as f32).collect(), Elems::I64(v) => v.iter().map(|&a| a as f32).collect(), Elems::Bool(v) => v.iter().map(|&a| a as u8 as f32).collect(),
    };
    let shape = x.shape.to_vec();
    let data = match to {
        1 => Data::F32(as_f32),
        7 => Data::I64(match x.data { Elems::I64(v) => v.to_vec(), Elems::I32(v) => v.iter().map(|&a| a as i64).collect(), Elems::Bool(v) => v.iter().map(|&a| a as i64).collect(), _ => as_f32.iter().map(|&a| a as i64).collect() }),
        6 => Data::I32(match x.data { Elems::I32(v) => v.to_vec(), Elems::I64(v) => v.iter().map(|&a| a as i32).collect(), _ => as_f32.iter().map(|&a| a as i32).collect() }),
        9 => Data::Bool(match x.data { Elems::I64(v) => v.iter().map(|&a| a != 0).collect(), Elems::Bool(v) => v.to_vec(), _ => as_f32.iter().map(|&a| a != 0.0).collect() }),
        _ => return Err(op_error(format!("to {}", to))),
    };
    Ok(Tensor { shape, data })
}

// ---- Shapes ----

fn with_shape(x: View<'_>, shape: Vec<usize>) -> Tensor { Tensor { shape, data: x.data.to_data() } }

fn reshape(x: View<'_>, target: &[i64], allowzero: bool) -> Result<Tensor> {
    let mut shape: Vec<i64> = target.iter().enumerate().map(|(i, &d)| if d == 0 && !allowzero { x.shape.get(i).copied().unwrap_or(0) as i64 } else { d }).collect();
    let known: i64 = shape.iter().filter(|&&d| d != -1).product();
    if let Some(at) = shape.iter().position(|&d| d == -1) { shape[at] = if known == 0 { 0 } else { x.numel() as i64 / known }; }
    let shape: Vec<usize> = shape.iter().map(|&d| d.max(0) as usize).collect();
    if shape.iter().product::<usize>() != x.numel() { return Err(op_error(format!("{:?} to {:?}", x.shape, target))); }
    Ok(with_shape(x, shape))
}

fn unsqueeze(x: View<'_>, axes: &[i64]) -> Result<Tensor> {
    let rank = x.shape.len() + axes.len();
    let mut axes: Vec<usize> = axes.iter().map(|&a| axis(a, rank)).collect::<Result<_>>()?;
    axes.sort_unstable();
    let mut shape = x.shape.to_vec();
    for a in axes { shape.insert(a, 1); }
    Ok(with_shape(x, shape))
}

fn squeeze(x: View<'_>, axes: Option<Vec<i64>>) -> Result<Tensor> {
    let axes: Vec<usize> = match axes { Some(a) => a.iter().map(|&a| axis(a, x.shape.len())).collect::<Result<_>>()?, None => (0..x.shape.len()).filter(|&i| x.shape[i] == 1).collect() };
    Ok(with_shape(x, x.shape.iter().enumerate().filter(|(i, _)| !axes.contains(i)).map(|(_, &d)| d).collect()))
}

// Copies blocks of a tensor: `pick(outer, inner)` names the element of `x` for each element of the result.
macro_rules! gather_elems {
    ($x:expr, $n:expr, $f:expr) => {
        match $x {
            Elems::F32(v) => Data::F32((0..$n).map(|i| v[$f(i)]).collect()),
            Elems::I64(v) => Data::I64((0..$n).map(|i| v[$f(i)]).collect()),
            Elems::I32(v) => Data::I32((0..$n).map(|i| v[$f(i)]).collect()),
            Elems::U8(v) => Data::U8((0..$n).map(|i| v[$f(i)]).collect()),
            Elems::I8(v) => Data::I8((0..$n).map(|i| v[$f(i)]).collect()),
            Elems::Bool(v) => Data::Bool((0..$n).map(|i| v[$f(i)]).collect()),
        }
    };
}

fn concat(xs: &[View<'_>], a: i64) -> Result<Tensor> {
    let first = xs.first().ok_or_else(|| op_error("nothing to join"))?;
    let rank = first.shape.len();
    let a = axis(a, rank)?;
    let outer: usize = first.shape[..a].iter().product();
    let mut shape = first.shape.to_vec();
    shape[a] = xs.iter().map(|x| x.shape.get(a).copied().unwrap_or(0)).sum();
    let blocks: Vec<usize> = xs.iter().map(|x| x.shape[a..].iter().product()).collect();
    let mut index = Vec::with_capacity(shape.iter().product());
    for o in 0..outer { for (k, &b) in blocks.iter().enumerate() { for i in 0..b { index.push((k, o * b + i)); } } }
    let n = index.len();
    macro_rules! join { ($variant:ident) => {{
        let parts: Vec<&[_]> = xs.iter().map(|x| match x.data { Elems::$variant(v) => Ok(v), _ => Err(op_error("mixed types")) }).collect::<Result<_>>()?;
        Data::$variant((0..n).map(|i| { let (k, j) = index[i]; parts[k][j] }).collect())
    }}; }
    let data = match first.data { Elems::F32(_) => join!(F32), Elems::I64(_) => join!(I64), Elems::I32(_) => join!(I32), Elems::U8(_) => join!(U8), Elems::I8(_) => join!(I8), Elems::Bool(_) => join!(Bool) };
    Ok(Tensor { shape, data })
}

fn gather(x: View<'_>, idx: View<'_>, a: i64) -> Result<Tensor> {
    let a = axis(a, x.shape.len())?;
    let indices = ints(idx)?;
    let dim = x.shape[a] as i64;
    let outer: usize = x.shape[..a].iter().product();
    let inner: usize = x.shape[a + 1..].iter().product();
    let fixed: Vec<usize> = indices.iter().map(|&i| { let i = if i < 0 { i + dim } else { i }; if i < 0 || i >= dim { Err(op_error("index out of range")) } else { Ok(i as usize) } }).collect::<Result<_>>()?;
    let mut shape = x.shape[..a].to_vec();
    shape.extend_from_slice(idx.shape);
    shape.extend_from_slice(&x.shape[a + 1..]);
    let n = outer * fixed.len() * inner;
    let pick = |i: usize| { let (o, rest) = (i / (fixed.len() * inner), i % (fixed.len() * inner)); let (k, j) = (rest / inner, rest % inner); (o * x.shape[a] + fixed[k]) * inner + j };
    Ok(Tensor { shape, data: gather_elems!(x.data, n, pick) })
}

fn gather_elements(x: View<'_>, idx: View<'_>, a: i64) -> Result<Tensor> {
    let a = axis(a, x.shape.len())?;
    let indices = ints(idx)?;
    let (xs, is) = (strides(x.shape), strides(idx.shape));
    let dim = x.shape[a] as i64;
    let mut map = Vec::with_capacity(indices.len());
    for (flat, &i) in indices.iter().enumerate() {
        let i = if i < 0 { i + dim } else { i };
        if i < 0 || i >= dim { return Err(op_error("index out of range")); }
        let mut at = 0;
        for d in 0..idx.shape.len() { let c = flat / is[d] % idx.shape[d]; at += if d == a { i as usize } else { c } * xs[d]; }
        map.push(at);
    }
    let n = map.len();
    Ok(Tensor { shape: idx.shape.to_vec(), data: gather_elems!(x.data, n, |i: usize| map[i]) })
}

fn slice(x: View<'_>, starts: &[i64], ends: &[i64], axes: Option<Vec<i64>>, steps: Option<Vec<i64>>) -> Result<Tensor> {
    let rank = x.shape.len();
    let axes: Vec<usize> = match axes { Some(a) => a.iter().map(|&a| axis(a, rank)).collect::<Result<_>>()?, None => (0..starts.len()).collect() };
    let steps = steps.unwrap_or_else(|| vec![1; starts.len()]);
    let (mut first, mut step, mut shape): (Vec<i64>, Vec<i64>, Vec<usize>) = (vec![0; rank], vec![1; rank], x.shape.to_vec());
    for (k, &a) in axes.iter().enumerate() {
        let dim = x.shape[a] as i64;
        let s = steps[k];
        if s == 0 { return Err(op_error("a step of 0")); }
        let clamp = |v: i64, lo: i64, hi: i64| v.max(lo).min(hi);
        let fix = |v: i64| if v < 0 { v + dim } else { v };
        let (b, e) = if s > 0 { (clamp(fix(starts[k]), 0, dim), clamp(fix(ends[k]), 0, dim)) } else { (clamp(fix(starts[k]), -1, dim - 1), clamp(fix(ends[k]), -1, dim - 1)) };
        let n = if s > 0 { (e - b + s - 1).max(0) / s } else { (b - e + (-s) - 1).max(0) / (-s) };
        first[a] = b;
        step[a] = s;
        shape[a] = n as usize;
    }
    let xs = strides(x.shape);
    let total: usize = shape.iter().product();
    let os = strides(&shape);
    let map: Vec<usize> = (0..total).map(|i| (0..rank).map(|d| ((first[d] + (i / os[d] % shape[d].max(1)) as i64 * step[d]) as usize) * xs[d]).sum()).collect();
    Ok(Tensor { shape, data: gather_elems!(x.data, total, |i: usize| map[i]) })
}

fn transpose(x: View<'_>, perm: &[usize]) -> Result<Tensor> {
    if perm.len() != x.shape.len() { return Err(op_error("perm")); }
    let shape: Vec<usize> = perm.iter().map(|&p| x.shape[p]).collect();
    let xs = strides(x.shape);
    let src: Vec<usize> = perm.iter().map(|&p| xs[p]).collect();
    let total = x.numel();
    let rank = shape.len();
    let mut map = Vec::with_capacity(total);
    let mut idx = vec![0usize; rank];
    let mut at = 0usize;
    for _ in 0..total {
        map.push(at);
        for d in (0..rank).rev() {
            idx[d] += 1;
            at += src[d];
            if idx[d] < shape[d] { break; }
            at -= src[d] * idx[d];
            idx[d] = 0;
        }
    }
    Ok(Tensor { shape, data: gather_elems!(x.data, total, |i: usize| map[i]) })
}

fn expand(x: View<'_>, target: &[i64]) -> Result<Tensor> {
    let target: Vec<usize> = target.iter().map(|&d| d.max(0) as usize).collect();
    let shape = broadcast(x.shape, &target)?;
    let map = index_map(x.shape, &shape);
    let n = map.len();
    Ok(Tensor { shape, data: gather_elems!(x.data, n, |i: usize| map[i]) })
}

fn tile(x: View<'_>, repeats: &[i64]) -> Result<Tensor> {
    let shape: Vec<usize> = x.shape.iter().zip(repeats).map(|(&d, &r)| d * r.max(0) as usize).collect();
    let (xs, os) = (strides(x.shape), strides(&shape));
    let total: usize = shape.iter().product();
    let map: Vec<usize> = (0..total).map(|i| (0..shape.len()).map(|d| (i / os[d] % shape[d]) % x.shape[d] * xs[d]).sum()).collect();
    Ok(Tensor { shape, data: gather_elems!(x.data, total, |i: usize| map[i]) })
}

fn range(start: View<'_>, limit: View<'_>, delta: View<'_>) -> Result<Tensor> {
    match (start.data, limit.data, delta.data) {
        (Elems::I64(s), Elems::I64(l), Elems::I64(d)) => {
            let (s, l, d) = (s[0], l[0], d[0]);
            if d == 0 { return Err(op_error("delta 0")); }
            let n = ((l - s + d - d.signum()) / d).max(0) as usize;
            Ok(Tensor::i64(vec![n], (0..n as i64).map(|i| s + i * d).collect()))
        }
        (Elems::F32(s), Elems::F32(l), Elems::F32(d)) => {
            let (s, l, d) = (s[0], l[0], d[0]);
            let n = math_ceil((l - s) / d).max(0.0) as usize;
            Ok(Tensor::f32(vec![n], (0..n).map(|i| s + i as f32 * d).collect()))
        }
        _ => Err(op_error("types")),
    }
}

fn math_ceil(x: f32) -> f32 { let t = x as i64 as f32; if t < x { t + 1.0 } else { t } }

fn constant_of_shape(shape: &[i64], node: &Node) -> Result<Tensor> {
    let shape: Vec<usize> = shape.iter().map(|&d| d.max(0) as usize).collect();
    let n: usize = shape.iter().product();
    // The converter keeps the value tensor as integers or floats; ONNX's default is f32 0.
    let data = match (node.ints("value"), node.floats("value")) {
        (Some(v), _) => Data::I64(vec![v.first().copied().unwrap_or(0); n]),
        (_, Some(v)) => Data::F32(vec![v.first().copied().unwrap_or(0.0) as f32; n]),
        _ => Data::F32(vec![0.0; n]),
    };
    Ok(Tensor { shape, data })
}

fn scatter_nd(x: View<'_>, idx: View<'_>, upd: View<'_>) -> Result<Tensor> {
    let indices = ints(idx)?;
    let k = *idx.shape.last().ok_or_else(|| op_error("indices"))?;
    let xs = strides(x.shape);
    let slice: usize = x.shape[k..].iter().product();
    let mut out = x.to_tensor();
    let count = indices.len() / k.max(1);
    for n in 0..count {
        let mut at = 0usize;
        for d in 0..k {
            let dim = x.shape[d] as i64;
            let i = indices[n * k + d];
            let i = if i < 0 { i + dim } else { i };
            if i < 0 || i >= dim { return Err(op_error("index out of range")); }
            at += i as usize * xs[d];
        }
        match (&mut out.data, upd.data) {
            (Data::F32(o), Elems::F32(u)) => o[at..at + slice].copy_from_slice(&u[n * slice..(n + 1) * slice]),
            (Data::I64(o), Elems::I64(u)) => o[at..at + slice].copy_from_slice(&u[n * slice..(n + 1) * slice]),
            (Data::Bool(o), Elems::Bool(u)) => o[at..at + slice].copy_from_slice(&u[n * slice..(n + 1) * slice]),
            _ => return Err(op_error("types")),
        }
    }
    Ok(out)
}

// ---- Reductions ----

fn reduce(op: &str, x: View<'_>, axes: Option<Vec<i64>>, keep: bool) -> Result<Tensor> {
    let rank = x.shape.len();
    let axes: Vec<usize> = match axes { Some(a) if !a.is_empty() => a.iter().map(|&a| axis(a, rank)).collect::<Result<_>>()?, _ => (0..rank).collect() };
    let shape_kept: Vec<usize> = x.shape.iter().enumerate().map(|(i, &d)| if axes.contains(&i) { 1 } else { d }).collect();
    let count: usize = axes.iter().map(|&a| x.shape[a]).product();
    let map = index_map(&shape_kept, x.shape);
    let out_n: usize = shape_kept.iter().product();
    let shape = if keep { shape_kept } else { x.shape.iter().enumerate().filter(|(i, _)| !axes.contains(i)).map(|(_, &d)| d).collect() };
    match x.data {
        Elems::F32(v) => {
            let mut sums = vec![0.0f32; out_n];
            for (i, &m) in map.iter().enumerate() { sums[m] += v[i]; }
            if op == "ReduceMean" { for s in sums.iter_mut() { *s /= count.max(1) as f32; } }
            Ok(Tensor::f32(shape, sums))
        }
        Elems::I64(v) => {
            let mut sums = vec![0i64; out_n];
            for (i, &m) in map.iter().enumerate() { sums[m] += v[i]; }
            if op == "ReduceMean" { for s in sums.iter_mut() { *s /= count.max(1) as i64; } }
            Ok(Tensor::i64(shape, sums))
        }
        _ => Err(op_error("type")),
    }
}

fn softmax(x: View<'_>, a: i64) -> Result<Tensor> {
    let a = axis(a, x.shape.len())?;
    if a != x.shape.len() - 1 { return Err(op_error("only the last axis")); }
    let v = f32s(&x)?;
    let n = x.shape[a].max(1);
    let mut out = Vec::with_capacity(v.len());
    for row in v.chunks(n) {
        let max = row.iter().fold(f32::NEG_INFINITY, |m, &a| m.max(a));
        let start = out.len();
        let mut sum = 0.0;
        for &a in row { let e = math::expf(a - max); sum += e; out.push(e); }
        for e in &mut out[start..] { *e /= sum; }
    }
    Ok(Tensor::f32(x.shape.to_vec(), out))
}

// ---- Products ----

// Batches of [m, k] x [k, n]: the batch shape broadcast, the offsets of each operand's matrix in each batch.
fn batches(sa: &[usize], sb: &[usize]) -> Result<(Vec<usize>, usize, usize, usize, Vec<(usize, usize)>)> {
    let (sa, sb) = (if sa.len() == 1 { [&[1usize][..], sa].concat() } else { sa.to_vec() }, if sb.len() == 1 { [sb, &[1usize][..]].concat() } else { sb.to_vec() });
    let (m, k, n) = (sa[sa.len() - 2], sa[sa.len() - 1], sb[sb.len() - 1]);
    if sb[sb.len() - 2] != k { return Err(op_error(format!("{:?} x {:?}", sa, sb))); }
    let (ba, bb) = (&sa[..sa.len() - 2], &sb[..sb.len() - 2]);
    let batch = broadcast(ba, bb)?;
    let (ma, mb) = (index_map(ba, &batch), index_map(bb, &batch));
    let pairs = ma.iter().zip(&mb).map(|(&i, &j)| (i * m * k, j * k * n)).collect();
    let mut shape = batch;
    shape.push(m);
    shape.push(n);
    Ok((shape, m, k, n, pairs))
}

fn matmul(a: View<'_>, b: View<'_>) -> Result<Tensor> {
    let (x, y) = (f32s(&a)?, f32s(&b)?);
    let (mut shape, m, k, n, pairs) = batches(a.shape, b.shape)?;
    let mut out = vec![0.0f32; pairs.len() * m * n];
    for (p, &(oa, ob)) in pairs.iter().enumerate() {
        for i in 0..m {
            let row = &mut out[(p * m + i) * n..(p * m + i + 1) * n];
            for kk in 0..k {
                let s = x[oa + i * k + kk];
                if s == 0.0 { continue; }
                let col = &y[ob + kk * n..ob + kk * n + n];
                for (o, &w) in row.iter_mut().zip(col) { *o += s * w; }
            }
        }
    }
    if a.shape.len() == 1 { shape.remove(shape.len() - 2); }
    if b.shape.len() == 1 { shape.pop(); }
    Ok(Tensor::f32(shape, out))
}

fn matmul_integer(a: View<'_>, b: View<'_>, za: Option<View<'_>>, zb: Option<View<'_>>) -> Result<Tensor> {
    let Elems::U8(x) = a.data else { return Err(op_error("A is not u8")) };
    let Elems::I8(y) = b.data else { return Err(op_error("B is not i8")) };
    let za = match za.map(|z| z.data) { Some(Elems::U8(z)) => z[0] as i32, None => 0, _ => return Err(op_error("A's zero point")) };
    let zb = match zb.map(|z| z.data) { Some(Elems::I8(z)) if z.len() == 1 => z[0] as i32, None => 0, _ => return Err(op_error("B's zero point")) };
    let (shape, m, k, n, pairs) = batches(a.shape, b.shape)?;
    let mut out = vec![0i32; pairs.len() * m * n];
    let mut acc = vec![0i32; n];
    for (p, &(oa, ob)) in pairs.iter().enumerate() {
        for i in 0..m {
            acc.fill(0);
            for kk in 0..k {
                let s = x[oa + i * k + kk] as i32 - za;
                if s == 0 { continue; }
                let col = &y[ob + kk * n..ob + kk * n + n];
                for (o, &w) in acc.iter_mut().zip(col) { *o += s * (w as i32 - zb); }
            }
            out[(p * m + i) * n..(p * m + i + 1) * n].copy_from_slice(&acc);
        }
    }
    Ok(Tensor { shape, data: Data::I32(out) })
}

// Round half to even, as onnxruntime's quantization does.
fn round_even(x: f32) -> f32 {
    let t = x as i64 as f32;
    let f = if t > x { t - 1.0 } else { t };
    let d = x - f;
    if d > 0.5 || (d == 0.5 && (f as i64) % 2 != 0) { f + 1.0 } else { f }
}

fn dynamic_quantize(x: View<'_>) -> Result<Vec<Tensor>> {
    let v = f32s(&x)?;
    let (lo, hi) = v.iter().fold((0.0f32, 0.0f32), |(lo, hi), &a| (lo.min(a), hi.max(a)));
    let scale = (hi - lo) / 255.0;
    let zero = if scale == 0.0 { 0.0 } else { round_even(0.0 - lo / scale).clamp(0.0, 255.0) };
    let q: Vec<u8> = v.iter().map(|&a| if scale == 0.0 { zero as u8 } else { (round_even(a / scale) + zero).clamp(0.0, 255.0) as u8 }).collect();
    Ok(vec![Tensor { shape: x.shape.to_vec(), data: Data::U8(q) }, Tensor::f32(vec![], vec![scale]), Tensor { shape: vec![], data: Data::U8(vec![zero as u8]) }])
}

fn dequantize(x: View<'_>, scale: View<'_>, zero: Option<View<'_>>) -> Result<Tensor> {
    let s = f32s(&scale)?;
    if s.len() != 1 { return Err(op_error("only a scalar scale")); }
    let s = s[0];
    let out: Vec<f32> = match (x.data, zero.map(|z| z.data)) {
        (Elems::I8(v), Some(Elems::I8(z))) => { let z = z[0] as f32; v.iter().map(|&a| (a as f32 - z) * s).collect() }
        (Elems::I8(v), None) => v.iter().map(|&a| a as f32 * s).collect(),
        (Elems::U8(v), Some(Elems::U8(z))) => { let z = z[0] as f32; v.iter().map(|&a| (a as f32 - z) * s).collect() }
        (Elems::U8(v), None) => v.iter().map(|&a| a as f32 * s).collect(),
        (Elems::I32(v), _) => v.iter().map(|&a| a as f32 * s).collect(),
        _ => return Err(op_error("types")),
    };
    Ok(Tensor::f32(x.shape.to_vec(), out))
}

// Conv over 1 or 2 spatial dimensions, with groups, padding, strides and dilations.
fn conv(x: View<'_>, w: View<'_>, b: Option<View<'_>>, node: &Node) -> Result<Tensor> {
    let (xv, wv) = (f32s(&x)?, f32s(&w)?);
    let bias = b.map(|b| f32s(&b)).transpose()?;
    let dims = x.shape.len() - 2;
    if !(1..=2).contains(&dims) || w.shape.len() != dims + 2 { return Err(op_error("1 or 2 spatial dimensions")); }
    let (batch, cin, cout) = (x.shape[0], x.shape[1], w.shape[0]);
    let group = node.int("group").unwrap_or(1) as usize;
    let cpg = w.shape[1];
    if cin != cpg * group || cout % group != 0 { return Err(op_error("groups")); }
    let get = |name: &str, default: i64| -> Vec<usize> { node.ints(name).map(|v| v.iter().map(|&a| a as usize).collect()).unwrap_or_else(|| vec![default as usize; dims]) };
    let (strides, dil) = (get("strides", 1), get("dilations", 1));
    let pads = node.ints("pads").map(|v| v.iter().map(|&a| a as usize).collect()).unwrap_or_else(|| vec![0; 2 * dims]);
    let (ih, iw) = if dims == 2 { (x.shape[2], x.shape[3]) } else { (1, x.shape[2]) };
    let (kh, kw) = if dims == 2 { (w.shape[2], w.shape[3]) } else { (1, w.shape[2]) };
    let (sh, sw) = if dims == 2 { (strides[0], strides[1]) } else { (1, strides[0]) };
    let (dh, dw) = if dims == 2 { (dil[0], dil[1]) } else { (1, dil[0]) };
    let (pt, pl, pb, pr) = if dims == 2 { (pads[0], pads[1], pads[2], pads[3]) } else { (0, pads[0], 0, pads[1]) };
    let oh = (ih + pt + pb).checked_sub(dh * (kh - 1) + 1).map(|v| v / sh + 1).unwrap_or(0);
    let ow = (iw + pl + pr).checked_sub(dw * (kw - 1) + 1).map(|v| v / sw + 1).unwrap_or(0);
    let opg = cout / group;
    let mut out = vec![0.0f32; batch * cout * oh * ow];
    for nb in 0..batch {
        for oc in 0..cout {
            let g = oc / opg;
            let base = bias.map_or(0.0, |b| b[oc]);
            let plane = &mut out[((nb * cout + oc) * oh) * ow..((nb * cout + oc + 1) * oh) * ow];
            plane.fill(base);
            for ic in 0..cpg {
                let input = &xv[((nb * cin + g * cpg + ic) * ih) * iw..((nb * cin + g * cpg + ic + 1) * ih) * iw];
                let kernel = &wv[((oc * cpg + ic) * kh) * kw..((oc * cpg + ic + 1) * kh) * kw];
                for ky in 0..kh {
                    for kx in 0..kw {
                        let k = kernel[ky * kw + kx];
                        if k == 0.0 { continue; }
                        for oy in 0..oh {
                            let iy = (oy * sh + ky * dh) as isize - pt as isize;
                            if iy < 0 || iy >= ih as isize { continue; }
                            let row = &input[iy as usize * iw..(iy as usize + 1) * iw];
                            let orow = &mut plane[oy * ow..(oy + 1) * ow];
                            for (ox, o) in orow.iter_mut().enumerate() {
                                let ix = (ox * sw + kx * dw) as isize - pl as isize;
                                if ix >= 0 && ix < iw as isize { *o += k * row[ix as usize]; }
                            }
                        }
                    }
                }
            }
        }
    }
    let mut shape = vec![batch, cout];
    if dims == 2 { shape.push(oh); }
    shape.push(ow);
    Ok(Tensor::f32(shape, out))
}

#[allow(dead_code)]
fn dtype_name(d: DType) -> &'static str { match d { DType::F32 => "f32", DType::U8 => "u8", DType::I8 => "i8", DType::I32 => "i32", DType::I64 => "i64", DType::Bool => "bool" } }
