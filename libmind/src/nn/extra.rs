//! The operators the speech synthesis models add to the dictation models' (252): VITS (Piper, Vosk TTS 0.7) pads,
//! splits, gathers by index lists, sums cumulatively, draws noise and upsamples with transposed convolutions.
use super::gemm::{self, Shape};
use super::ops::{axis, f32s, strides, view_walk, zip_with};
use super::{op_error, Data, Elems, Node, Result, Tensor, View};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

pub(super) fn logical(op: &str, a: View<'_>, b: View<'_>) -> Result<Tensor> {
    let (Elems::Bool(x), Elems::Bool(y)) = (a.data, b.data) else { return Err(op_error("not bool")) };
    let (shape, v) = match op { "And" => zip_with(x, a.shape, y, b.shape, |p, q| p && q)?, "Or" => zip_with(x, a.shape, y, b.shape, |p, q| p || q)?, _ => zip_with(x, a.shape, y, b.shape, |p, q| p != q)? };
    Ok(Tensor { shape, data: Data::Bool(v) })
}

pub(super) fn not(a: View<'_>) -> Result<Tensor> {
    let Elems::Bool(x) = a.data else { return Err(op_error("not bool")) };
    Ok(Tensor { shape: a.shape.to_vec(), data: Data::Bool(x.iter().map(|&p| !p).collect()) })
}

// Pad: `pads` holds every axis' start, then every axis' end (negative: cut); modes constant, reflect and edge.
pub(super) fn pad(x: View<'_>, pads: &[i64], value: Option<View<'_>>, axes: Option<Vec<i64>>, mode: &str) -> Result<Tensor> {
    let rank = x.shape.len();
    let axes: Vec<usize> = match axes { Some(a) => a.iter().map(|&a| axis(a, rank)).collect::<Result<_>>()?, None => (0..rank).collect() };
    if pads.len() != 2 * axes.len() { return Err(op_error("pads")); }
    let (mut begin, mut end) = (vec![0i64; rank], vec![0i64; rank]);
    for (k, &a) in axes.iter().enumerate() { (begin[a], end[a]) = (pads[k], pads[k + axes.len()]); }
    let shape: Vec<usize> = (0..rank).map(|d| (x.shape[d] as i64 + begin[d] + end[d]).max(0) as usize).collect();
    let total: usize = shape.iter().product();
    // Per output element: its source index, or None for the constant.
    let own = strides(x.shape);
    let reflect = mode == "reflect";
    let edge = mode == "edge";
    if !(reflect || edge || mode == "constant") { return Err(op_error(format!("mode {}", mode))); }
    if (reflect || edge) && x.shape.contains(&0) { return Err(op_error("nothing to repeat")); }
    if !(reflect || edge) && rank > 0 { return pad_constant(x, &begin, &shape, value); }
    let mut sources: Vec<Option<usize>> = Vec::with_capacity(total);
    let mut index = vec![0usize; rank];
    for _ in 0..total {
        let mut at = Some(0usize);
        for d in 0..rank {
            let n = x.shape[d] as i64;
            let mut i = index[d] as i64 - begin[d];
            if i < 0 || i >= n {
                if edge { i = i.clamp(0, n - 1); }
                else if reflect && n > 1 { let period = 2 * (n - 1); i = i.rem_euclid(period); if i >= n { i = period - i; } }
                else if reflect { i = 0; }
                else { at = None; break; }
            }
            at = at.map(|a| a + i as usize * own[d]);
        }
        sources.push(at);
        for d in (0..rank).rev() { index[d] += 1; if index[d] < shape[d] { break; } index[d] = 0; }
    }
    let data = match x.data {
        Elems::F32(v) => { let c = value.map(|c| f32s(&c).map(|s| s.first().copied().unwrap_or(0.0))).transpose()?.unwrap_or(0.0); Data::F32(sources.iter().map(|s| s.map_or(c, |i| v[i])).collect()) }
        Elems::I64(v) => { let c = match value.map(|c| c.data) { Some(Elems::I64(s)) => s.first().copied().unwrap_or(0), _ => 0 }; Data::I64(sources.iter().map(|s| s.map_or(c, |i| v[i])).collect()) }
        _ => return Err(op_error("type")),
    };
    Ok(Tensor { shape, data })
}

// Pad with a constant: the constant everywhere, then each run of the input's last axis that lands in the output copied.
fn pad_constant(x: View<'_>, begin: &[i64], shape: &[usize], value: Option<View<'_>>) -> Result<Tensor> {
    let rank = shape.len();
    let total: usize = shape.iter().product();
    let (own, out) = (strides(x.shape), strides(shape));
    // Per axis the input positions kept: from..to.
    let range: Vec<(usize, usize)> = (0..rank).map(|d| ((-begin[d]).max(0) as usize, (shape[d] as i64 - begin[d]).clamp(0, x.shape[d] as i64) as usize)).collect();
    fn fill<T: Copy>(v: &[T], c: T, total: usize, range: &[(usize, usize)], begin: &[i64], own: &[usize], out: &[usize]) -> Vec<T> {
        let rank = range.len();
        let mut result = vec![c; total];
        if range.iter().any(|&(a, b)| a >= b) { return result; }
        let last = rank - 1;
        let (from, to) = range[last];
        let mut index: Vec<usize> = range.iter().map(|r| r.0).collect();
        loop {
            let src: usize = (0..rank).map(|d| index[d] * own[d]).sum();
            let dst: usize = (0..rank).map(|d| (index[d] as i64 + begin[d]) as usize * out[d]).sum();
            result[dst..dst + to - from].copy_from_slice(&v[src..src + to - from]);
            let mut d = last;
            loop {
                if d == 0 { return result; }
                d -= 1;
                index[d] += 1;
                if index[d] < range[d].1 { break; }
                index[d] = range[d].0;
            }
        }
    }
    let data = match x.data {
        Elems::F32(v) => { let c = value.map(|c| f32s(&c).map(|s| s.first().copied().unwrap_or(0.0))).transpose()?.unwrap_or(0.0); Data::F32(fill(v, c, total, &range, begin, &own, &out)) }
        Elems::I64(v) => { let c = match value.map(|c| c.data) { Some(Elems::I64(s)) => s.first().copied().unwrap_or(0), _ => 0 }; Data::I64(fill(v, c, total, &range, begin, &own, &out)) }
        _ => return Err(op_error("type")),
    };
    Ok(Tensor { shape: shape.to_vec(), data })
}

// Split into `outputs` parts along `a`: the sizes given, or equal parts (the last smaller, as opset 18 has it).
pub(super) fn split(x: View<'_>, sizes: Option<Vec<i64>>, a: i64, outputs: usize) -> Result<Vec<Tensor>> {
    let a = axis(a, x.shape.len())?;
    let dim = x.shape[a];
    let sizes: Vec<usize> = match sizes {
        Some(s) => s.iter().map(|&v| v.max(0) as usize).collect(),
        None => { let each = dim.div_ceil(outputs.max(1)); (0..outputs).map(|k| each.min(dim.saturating_sub(k * each))).collect() }
    };
    if sizes.iter().sum::<usize>() != dim { return Err(op_error("split sizes")); }
    let own = strides(x.shape);
    let steps: Vec<isize> = own.iter().map(|&s| s as isize).collect();
    let mut start = 0;
    let mut out = Vec::with_capacity(sizes.len());
    for size in sizes {
        let mut shape = x.shape.to_vec();
        shape[a] = size;
        out.push(Tensor { data: view_walk(x.data, (start * own[a]) as isize, &shape, &steps), shape });
        start += size;
    }
    Ok(out)
}

// Normal noise of the input's shape: Box and Muller over a 64-bit generator seeded by `seed` (or a constant), so a
// voice says a sentence the same way every time.
pub(super) fn random_normal_like(x: View<'_>, node: &Node) -> Result<Tensor> {
    let float = |name: &str, default: f64| node.floats(name).and_then(|v| v.first().copied()).unwrap_or(default);
    let (mean, scale) = (float("mean", 0.0), float("scale", 1.0));
    let mut state = (float("seed", 252.0) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut uniform = || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; ((state >> 11) as f64 + 0.5) / (1u64 << 53) as f64 };
    let n = x.numel();
    let mut out = Vec::with_capacity(n + 1);
    while out.len() < n {
        let (u, v) = (uniform(), uniform());
        let r = crate::voice::math::sqrt(-2.0 * crate::voice::math::ln(u));
        let angle = 2.0 * core::f64::consts::PI * v;
        out.push((mean + scale * r * crate::voice::math::cos(angle)) as f32);
        out.push((mean + scale * r * crate::voice::math::sin(angle)) as f32);
    }
    out.truncate(n);
    Ok(Tensor::f32(x.shape.to_vec(), out))
}

// The indices of the elements that are not zero: [rank, count], in row-major order.
pub(super) fn non_zero(x: View<'_>) -> Result<Tensor> {
    let set: Vec<usize> = match x.data {
        Elems::Bool(v) => v.iter().enumerate().filter(|(_, &a)| a).map(|(i, _)| i).collect(),
        Elems::I64(v) => v.iter().enumerate().filter(|(_, &a)| a != 0).map(|(i, _)| i).collect(),
        Elems::F32(v) => v.iter().enumerate().filter(|(_, &a)| a != 0.0).map(|(i, _)| i).collect(),
        _ => return Err(op_error("type")),
    };
    let rank = x.shape.len();
    let own = strides(x.shape);
    let mut out = vec![0i64; rank * set.len()];
    for (k, &flat) in set.iter().enumerate() {
        for d in 0..rank { out[d * set.len() + k] = (flat / own[d] % x.shape[d]) as i64; }
    }
    Ok(Tensor::i64(vec![rank, set.len()], out))
}

// GatherND: each row of `idx`'s last axis names a slice of `x`.
pub(super) fn gather_nd(x: View<'_>, idx: View<'_>, batch: i64) -> Result<Tensor> {
    if batch != 0 { return Err(op_error("batch_dims")); }
    let indices = super::ops::ints(idx)?;
    let k = *idx.shape.last().ok_or_else(|| op_error("indices"))?;
    if k > x.shape.len() { return Err(op_error("indices too long")); }
    let own = strides(x.shape);
    let slice: usize = x.shape[k..].iter().product();
    let count = indices.len() / k.max(1);
    let mut starts = Vec::with_capacity(count);
    for row in indices.chunks(k.max(1)).take(count) {
        let mut at = 0usize;
        for (d, &i) in row.iter().enumerate().take(k) {
            let n = x.shape[d] as i64;
            let i = if i < 0 { i + n } else { i };
            if i < 0 || i >= n { return Err(op_error("index out of range")); }
            at += i as usize * own[d];
        }
        starts.push(at);
    }
    let mut shape = idx.shape[..idx.shape.len() - 1].to_vec();
    shape.extend_from_slice(&x.shape[k..]);
    macro_rules! take { ($v:expr) => { starts.iter().flat_map(|&s| $v[s..s + slice].iter().copied()).collect() }; }
    let data = match x.data { Elems::F32(v) => Data::F32(take!(v)), Elems::I64(v) => Data::I64(take!(v)), Elems::Bool(v) => Data::Bool(take!(v)), Elems::I32(v) => Data::I32(take!(v)), _ => return Err(op_error("type")) };
    Ok(Tensor { shape, data })
}

// CumSum along an axis, inclusive or not, forward or backward.
pub(super) fn cum_sum(x: View<'_>, a: &[i64], exclusive: bool, reverse: bool) -> Result<Tensor> {
    let a = axis(*a.first().ok_or_else(|| op_error("axis"))?, x.shape.len())?;
    let (outer, dim, inner) = (x.shape[..a].iter().product::<usize>(), x.shape[a], x.shape[a + 1..].iter().product::<usize>());
    fn scan<T: Copy + Default + core::ops::Add<Output = T>>(v: &[T], (outer, dim, inner): (usize, usize, usize), exclusive: bool, reverse: bool) -> Vec<T> {
        let mut out = vec![T::default(); v.len()];
        for o in 0..outer {
            for i in 0..inner {
                let mut sum = T::default();
                for step in 0..dim {
                    let k = if reverse { dim - 1 - step } else { step };
                    let at = (o * dim + k) * inner + i;
                    if exclusive { out[at] = sum; sum = sum + v[at]; } else { sum = sum + v[at]; out[at] = sum; }
                }
            }
        }
        out
    }
    let data = match x.data {
        Elems::F32(v) => Data::F32(scan(v, (outer, dim, inner), exclusive, reverse)),
        Elems::I64(v) => Data::I64(scan(v, (outer, dim, inner), exclusive, reverse)),
        Elems::I32(v) => Data::I32(scan(v, (outer, dim, inner), exclusive, reverse)),
        _ => return Err(op_error("type")),
    };
    Ok(Tensor { shape: x.shape.to_vec(), data })
}

// ConvTranspose in one dimension (the HiFi-GAN decoder's upsampling): per batch and group, every input position's
// contribution to every output tap (x^T w, one product), then added in place (col2im).
pub(super) fn conv_transpose(x: View<'_>, w: View<'_>, b: Option<View<'_>>, node: &Node) -> Result<Tensor> {
    let (xv, wv) = (f32s(&x)?, f32s(&w)?);
    let bias = b.map(|b| f32s(&b)).transpose()?;
    if x.shape.len() != 3 || w.shape.len() != 3 { return Err(op_error("one spatial dimension")); }
    let (batch, cin, length) = (x.shape[0], x.shape[1], x.shape[2]);
    let group = node.int("group").unwrap_or(1).max(1) as usize;
    let (opg, k) = (w.shape[1], w.shape[2]);
    if w.shape[0] != cin || cin % group != 0 { return Err(op_error("groups")); }
    let first = |name: &str, default: i64| node.ints(name).and_then(|v| v.first().copied()).unwrap_or(default).max(0) as usize;
    let (stride, dilation, extra) = (first("strides", 1).max(1), first("dilations", 1).max(1), first("output_padding", 0));
    let pads = node.ints("pads").map(|v| v.to_vec()).unwrap_or_else(|| vec![0, 0]);
    let (begin, end) = (pads.first().copied().unwrap_or(0).max(0) as usize, pads.get(1).copied().unwrap_or(0).max(0) as usize);
    let full = stride * (length.max(1) - 1) + extra + (k - 1) * dilation + 1;
    let out_len = full.checked_sub(begin + end).ok_or_else(|| op_error("pads"))?;
    let (cpg, cout) = (cin / group, opg * group);
    let mut out = vec![0.0f32; batch * cout * out_len];
    for (oc, row) in out.chunks_exact_mut(out_len.max(1)).enumerate().filter(|_| out_len > 0) { row.fill(bias.map_or(0.0, |b| b[oc % cout])); }
    let mut xt = vec![0.0f32; length * cpg];
    let mut col = vec![0.0f32; length * opg * k];
    for nb in 0..batch {
        for g in 0..group {
            // x^T: [length, cpg]; w of the group: [cpg, opg * k]; col: [length, opg * k].
            for c in 0..cpg { for t in 0..length { xt[t * cpg + c] = xv[(nb * cin + g * cpg + c) * length + t]; } }
            col.fill(0.0);
            gemm::f32(&xt, Shape::dense(length, cpg), &wv[g * cpg * opg * k..(g + 1) * cpg * opg * k], Shape::dense(cpg, opg * k), &mut col, Shape::dense(length, opg * k));
            for o in 0..opg {
                let row = &mut out[((nb * cout + g * opg + o) * out_len)..((nb * cout + g * opg + o + 1) * out_len)];
                for t in 0..length {
                    let taps = &col[t * opg * k + o * k..t * opg * k + (o + 1) * k];
                    for (kk, &v) in taps.iter().enumerate() {
                        let at = t * stride + kk * dilation;
                        if at >= begin && at - begin < out_len { row[at - begin] += v; }
                    }
                }
            }
        }
    }
    Ok(Tensor::f32(vec![batch, cout, out_len], out))
}
