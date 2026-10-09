//! The products under MatMul, MatMulInteger and Conv: C += A B in f32, and C = (A - za)(B - zb) in i32, exactly.
//! AVX2 with FMA when the processor and the system enable them (CPUID and XCR0, checked once), else plain loops;
//! the two f32 paths round differently (FMA rounds once), the i32 ones agree to the bit.
use alloc::vec;
use alloc::vec::Vec;

/// A matrix in a slice, row major: `rows` rows of `cols` elements, `stride` apart.
#[derive(Clone, Copy)]
pub(crate) struct Shape { pub rows: usize, pub cols: usize, pub stride: usize }

impl Shape {
    pub(crate) fn dense(rows: usize, cols: usize) -> Self { Self { rows, cols, stride: cols } }
    fn fits(&self, len: usize) -> bool { self.cols <= self.stride && (self.rows == 0 || self.cols == 0 || (self.rows - 1) * self.stride + self.cols <= len) }
}

/// How B's elements lie for `i8`: in rows; or in panels of 16 columns (k even, n a multiple of 16), one after another,
/// each holding for every pair of rows p the pairs (b[2p][j], b[2p+1][j]) of its columns j in order. The converter
/// lays MatMulInteger's weights out so (tensor code 7): one load then gives a vpmaddwd its operand, and a panel is read
/// straight through.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layout { Rows, Panels }

/// Whether the products use SIMD; `Some(false)` turns it off, `Some(true)` back on where the processor has it.
pub fn simd(set: Option<bool>) -> bool {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    {
        use core::sync::atomic::Ordering;
        if let Some(on) = set { x86::STATE.store(if on { 0 } else { 1 }, Ordering::Relaxed); }
        x86::avx2()
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    { let _ = set; false }
}

/// c += a b: `a` is m x k, `b` k x n, `c` m x n.
pub(crate) fn f32(a: &[f32], sa: Shape, b: &[f32], sb: Shape, c: &mut [f32], sc: Shape) {
    let (m, k, n) = (sa.rows, sa.cols, sb.cols);
    assert!(sb.rows == k && sc.rows == m && sc.cols == n && sa.fits(a.len()) && sb.fits(b.len()) && sc.fits(c.len()));
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    if x86::avx2() {
        // SAFETY: the shapes fit the slices (asserted), and the processor has AVX2 and FMA.
        unsafe { x86::f32(m, k, n, a.as_ptr(), sa.stride, b.as_ptr(), sb.stride, c.as_mut_ptr(), sc.stride) };
        return;
    }
    for i in 0..m {
        let row = &mut c[i * sc.stride..i * sc.stride + n];
        for kk in 0..k {
            let s = a[i * sa.stride + kk];
            let col = &b[kk * sb.stride..kk * sb.stride + n];
            for (o, &w) in row.iter_mut().zip(col) { *o += s * w; }
        }
    }
}

/// c = (a - za)(b - zb): `a` is m x k, `b` k x n laid out as `layout` says, `c` m x n (dense).
pub(crate) fn i8(a: &[u8], sa: Shape, za: u8, b: &[i8], sb: Shape, layout: Layout, zb: i8, c: &mut [i32]) {
    let (m, k, n) = (sa.rows, sa.cols, sb.cols);
    assert!(sb.rows == k && sa.fits(a.len()) && sb.fits(b.len()) && c.len() == m * n);
    let panels = layout == Layout::Panels;
    assert!(!panels || (k % 2 == 0 && n % 16 == 0 && b.len() >= k * n));
    // A less its zero point, in i16 pairs along k (k made even with a zero).
    let k2 = k + (k & 1);
    let mut a16 = vec![0i16; m * k2];
    for i in 0..m {
        for (o, &x) in a16[i * k2..i * k2 + k].iter_mut().zip(&a[i * sa.stride..i * sa.stride + k]) { *o = x as i16 - za as i16; }
    }
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    if x86::avx2() {
        // SAFETY: a16 is m x k2, b fits k x n in its layout (asserted), c is m x n; the processor has AVX2.
        unsafe {
            if panels { x86::i8_panels(m, k, n, a16.as_ptr(), b.as_ptr(), zb, c.as_mut_ptr()) } else { x86::i8(m, k, n, a16.as_ptr(), k2, b.as_ptr(), sb.stride, zb, c.as_mut_ptr()) }
        };
        return;
    }
    if panels {
        for i in 0..m {
            let row = &a16[i * k..(i + 1) * k];
            for (panel, out) in b.chunks_exact(16 * k).zip(c[i * n..(i + 1) * n].chunks_exact_mut(16)) {
                let mut acc = [0i32; 16];
                for (pair, w) in row.chunks_exact(2).zip(panel.chunks_exact(32)) {
                    for (o, w) in acc.iter_mut().zip(w.chunks_exact(2)) { *o += pair[0] as i32 * (w[0] as i32 - zb as i32) + pair[1] as i32 * (w[1] as i32 - zb as i32); }
                }
                out.copy_from_slice(&acc);
            }
        }
        return;
    }
    let mut acc: Vec<i32> = vec![0; n];
    for i in 0..m {
        acc.fill(0);
        for kk in 0..k {
            let s = a16[i * k2 + kk] as i32;
            if s == 0 { continue; }
            let col = &b[kk * sb.stride..kk * sb.stride + n];
            for (o, &w) in acc.iter_mut().zip(col) { *o += s * (w as i32 - zb as i32); }
        }
        c[i * n..(i + 1) * n].copy_from_slice(&acc);
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod x86 {
    use core::arch::x86_64::*;
    use core::sync::atomic::{AtomicU8, Ordering};

    /// 0: not checked yet, 1: no AVX2, 2: AVX2 and FMA.
    pub(super) static STATE: AtomicU8 = AtomicU8::new(0);

    pub(super) fn avx2() -> bool {
        match STATE.load(Ordering::Relaxed) {
            1 => false,
            2 => true,
            _ => { let yes = detect(); STATE.store(if yes { 2 } else { 1 }, Ordering::Relaxed); yes }
        }
    }

    // AVX, FMA and AVX2 in CPUID, and the system saving the AVX registers (OSXSAVE, XCR0's SSE and AVX bits).
    #[allow(unused_unsafe)]
    fn detect() -> bool {
        // SAFETY: CPUID is on every x86_64 processor; XGETBV runs only when CPUID says the system enabled XSAVE.
        unsafe {
            let one = __cpuid(1);
            let (fma, osxsave, avx) = ((one.ecx >> 12) & 1 == 1, (one.ecx >> 27) & 1 == 1, (one.ecx >> 28) & 1 == 1);
            if !(fma && osxsave && avx) || __get_cpuid_max(0).0 < 7 { return false; }
            (__cpuid_count(7, 0).ebx >> 5) & 1 == 1 && xcr0() & 6 == 6
        }
    }

    #[target_feature(enable = "xsave")]
    unsafe fn xcr0() -> u64 { _xgetbv(0) }

    // Columns j.. of 8 lanes: all of them, or the first `left` (< 8).
    #[target_feature(enable = "avx2")]
    unsafe fn mask(left: usize) -> __m256i { _mm256_cmpgt_epi32(_mm256_set1_epi32(left as i32), _mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7)) }

    // Depth of a block of k: its rows of a 16-column block of B (16 KiB) stay in the first-level cache for every row.
    const KC: usize = 256;

    #[target_feature(enable = "avx2,fma")]
    pub(super) unsafe fn f32(m: usize, k: usize, n: usize, a: *const f32, lda: usize, b: *const f32, ldb: usize, c: *mut f32, ldc: usize) {
        // The block of B, copied into one contiguous run so that it is read straight through.
        let mut panel: alloc::vec::Vec<f32> = alloc::vec![0.0; KC * 16];
        let mut j = 0;
        while j + 16 <= n {
            let mut p = 0;
            while p < k {
                let kc = (k - p).min(KC);
                for r in 0..kc { core::ptr::copy_nonoverlapping(b.add((p + r) * ldb + j), panel.as_mut_ptr().add(r * 16), 16); }
                let (ap, bp) = (a.add(p), panel.as_ptr());
                let mut i = 0;
                while i + 6 <= m { f32_wide::<6>(i, 0, kc, ap, lda, bp, 16, c.add(j), ldc); i += 6; }
                if i + 4 <= m { f32_wide::<4>(i, 0, kc, ap, lda, bp, 16, c.add(j), ldc); i += 4; }
                if i + 2 <= m { f32_wide::<2>(i, 0, kc, ap, lda, bp, 16, c.add(j), ldc); i += 2; }
                if i < m { f32_wide::<1>(i, 0, kc, ap, lda, bp, 16, c.add(j), ldc); }
                p += kc;
            }
            j += 16;
        }
        while j < n {
            let m8 = mask(n - j);
            let mut i = 0;
            while i + 6 <= m { f32_narrow::<6>(i, j, k, a, lda, b, ldb, c, ldc, m8); i += 6; }
            while i < m { f32_narrow::<1>(i, j, k, a, lda, b, ldb, c, ldc, m8); i += 1; }
            j += 8;
        }
    }

    // Rows i..i+R, columns j..j+16.
    #[target_feature(enable = "avx2,fma")]
    #[inline]
    unsafe fn f32_wide<const R: usize>(i: usize, j: usize, k: usize, a: *const f32, lda: usize, b: *const f32, ldb: usize, c: *mut f32, ldc: usize) {
        let mut acc = [[_mm256_setzero_ps(); 2]; R];
        for (r, acc) in acc.iter_mut().enumerate() {
            let p = c.add((i + r) * ldc + j);
            *acc = [_mm256_loadu_ps(p), _mm256_loadu_ps(p.add(8))];
        }
        for kk in 0..k {
            let row = b.add(kk * ldb + j);
            let (b0, b1) = (_mm256_loadu_ps(row), _mm256_loadu_ps(row.add(8)));
            for (r, acc) in acc.iter_mut().enumerate() {
                let x = _mm256_broadcast_ss(&*a.add((i + r) * lda + kk));
                acc[0] = _mm256_fmadd_ps(x, b0, acc[0]);
                acc[1] = _mm256_fmadd_ps(x, b1, acc[1]);
            }
        }
        for (r, acc) in acc.iter().enumerate() {
            let p = c.add((i + r) * ldc + j);
            _mm256_storeu_ps(p, acc[0]);
            _mm256_storeu_ps(p.add(8), acc[1]);
        }
    }

    // Rows i..i+R, the columns of `m8` from j.
    #[target_feature(enable = "avx2,fma")]
    #[inline]
    unsafe fn f32_narrow<const R: usize>(i: usize, j: usize, k: usize, a: *const f32, lda: usize, b: *const f32, ldb: usize, c: *mut f32, ldc: usize, m8: __m256i) {
        let mut acc = [_mm256_setzero_ps(); R];
        for (r, acc) in acc.iter_mut().enumerate() { *acc = _mm256_maskload_ps(c.add((i + r) * ldc + j), m8); }
        for kk in 0..k {
            let b0 = _mm256_maskload_ps(b.add(kk * ldb + j), m8);
            for (r, acc) in acc.iter_mut().enumerate() { *acc = _mm256_fmadd_ps(_mm256_broadcast_ss(&*a.add((i + r) * lda + kk)), b0, *acc); }
        }
        for (r, acc) in acc.iter().enumerate() { _mm256_maskstore_ps(c.add((i + r) * ldc + j), m8, *acc); }
    }

    // a: m x k2 in i16 (k2 even), b: k x n in i8 rows ldb apart, c: m x n.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn i8(m: usize, k: usize, n: usize, a: *const i16, k2: usize, b: *const i8, ldb: usize, zb: i8, c: *mut i32) {
        let mut j = 0;
        while j + 16 <= n {
            let mut i = 0;
            while i + 6 <= m { i8_wide::<6>(i, j, k, a, k2, b, ldb, zb, c, n); i += 6; }
            while i < m { i8_wide::<1>(i, j, k, a, k2, b, ldb, zb, c, n); i += 1; }
            j += 16;
        }
        for i in 0..m {
            for jj in j..n {
                let mut s = 0i32;
                for kk in 0..k { s += *a.add(i * k2 + kk) as i32 * (*b.add(kk * ldb + jj) as i32 - zb as i32); }
                *c.add(i * n + jj) = s;
            }
        }
    }

    // a: m x k in i16 (k even), b: k x n in panels (Layout::Panels), c: m x n.
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn i8_panels(m: usize, k: usize, n: usize, a: *const i16, b: *const i8, zb: i8, c: *mut i32) {
        for p in 0..n / 16 {
            let panel = b.add(p * 16 * k);
            let mut i = 0;
            while i + 6 <= m { i8_panel::<6>(i, p * 16, k, a, panel, zb, c, n); i += 6; }
            while i < m { i8_panel::<1>(i, p * 16, k, a, panel, zb, c, n); i += 1; }
        }
    }

    // Rows i..i+R, columns j..j+16 from their panel: per pair of rows 32 bytes, widened to two vpmaddwd operands.
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn i8_panel<const R: usize>(i: usize, j: usize, k: usize, a: *const i16, panel: *const i8, zb: i8, c: *mut i32, ldc: usize) {
        let z = _mm256_set1_epi16(zb as i16);
        let mut acc = [[_mm256_setzero_si256(); 2]; R];
        let mut p = 0;
        while p < k {
            let at = panel.add(p * 16) as *const __m128i;
            let lo = _mm256_sub_epi16(_mm256_cvtepi8_epi16(_mm_loadu_si128(at)), z);
            let hi = _mm256_sub_epi16(_mm256_cvtepi8_epi16(_mm_loadu_si128(at.add(1))), z);
            for (r, acc) in acc.iter_mut().enumerate() {
                let pair = _mm256_set1_epi32((a.add((i + r) * k + p) as *const i32).read_unaligned());
                acc[0] = _mm256_add_epi32(acc[0], _mm256_madd_epi16(pair, lo));
                acc[1] = _mm256_add_epi32(acc[1], _mm256_madd_epi16(pair, hi));
            }
            p += 2;
        }
        for (r, acc) in acc.iter().enumerate() {
            let out = c.add((i + r) * ldc + j) as *mut __m256i;
            _mm256_storeu_si256(out, acc[0]);
            _mm256_storeu_si256(out.add(1), acc[1]);
        }
    }

    // Rows i..i+R, columns j..j+16: B's rows 2p and 2p+1 widened to i16 and interleaved, so that one vpmaddwd per
    // eight columns adds a row of A's pair (a[2p], a[2p+1]) times them; every product and pair sum fits i32 exactly.
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn i8_wide<const R: usize>(i: usize, j: usize, k: usize, a: *const i16, k2: usize, b: *const i8, ldb: usize, zb: i8, c: *mut i32, ldc: usize) {
        let z = _mm256_set1_epi16(zb as i16);
        let mut acc = [[_mm256_setzero_si256(); 2]; R];
        let mut p = 0;
        while p < k {
            let r0 = b.add(p * ldb + j);
            let r1 = if p + 1 < k { b.add((p + 1) * ldb + j) } else { r0 }; // A's pad is 0 there
            let x0 = _mm256_sub_epi16(_mm256_cvtepi8_epi16(_mm_loadu_si128(r0 as *const __m128i)), z);
            let x1 = _mm256_sub_epi16(_mm256_cvtepi8_epi16(_mm_loadu_si128(r1 as *const __m128i)), z);
            // Per 128-bit lane: lo holds columns 0-3 and 8-11, hi 4-7 and 12-15.
            let (lo, hi) = (_mm256_unpacklo_epi16(x0, x1), _mm256_unpackhi_epi16(x0, x1));
            for (r, acc) in acc.iter_mut().enumerate() {
                let pair = _mm256_set1_epi32((a.add((i + r) * k2 + p) as *const i32).read_unaligned());
                acc[0] = _mm256_add_epi32(acc[0], _mm256_madd_epi16(pair, lo));
                acc[1] = _mm256_add_epi32(acc[1], _mm256_madd_epi16(pair, hi));
            }
            p += 2;
        }
        for (r, acc) in acc.iter().enumerate() {
            let out = c.add((i + r) * ldc + j) as *mut __m256i;
            _mm256_storeu_si256(out, _mm256_permute2x128_si256(acc[0], acc[1], 0x20));
            _mm256_storeu_si256(out.add(1), _mm256_permute2x128_si256(acc[0], acc[1], 0x31));
        }
    }
}
