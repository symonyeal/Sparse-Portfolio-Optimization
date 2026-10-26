//! Buy-and-hold account recursion used by pf.bt.hold.
// Legend: r = row-major returns (h,n); rf = T-bill returns (h);
// ls = delisting index relative to first holding month; w = initial weights (n);
// v = asset account values; c = cash account; out = values after each month (h,n);
// cash = cash after each month (h). The Python wrapper performs NumPy's row sums.

use std::ffi::c_char;
use std::slice;

static SOURCE: &str = concat!(include_str!("lib.rs"), include_str!("../Cargo.toml"), "\0");

#[unsafe(no_mangle)]
pub extern "C" fn pf_source() -> *const c_char {
    SOURCE.as_ptr().cast()
}

/// # Safety
/// Buffers must be non-null, aligned, non-overlapping, and valid for the stated
/// lengths. Inputs remain immutable. The caller owns the output and cash buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pf_hold(
    r: *const f64,
    rf: *const f64,
    ls: *const i64,
    w: *const f64,
    h: usize,
    n: usize,
    out: *mut f64,
    cash: *mut f64,
) -> i32 {
    let Some(len) = h.checked_mul(n) else {
        return 2;
    };
    if h == 0
        || n == 0
        || len > isize::MAX as usize / 8
        || r.is_null()
        || rf.is_null()
        || ls.is_null()
        || w.is_null()
        || out.is_null()
        || cash.is_null()
    {
        return 2;
    }
    // Safety: buffer validity is the FFI caller's contract, checked by pf.native.
    let (r, rf, ls, w, out, cash) = unsafe {
        (
            slice::from_raw_parts(r, len),
            slice::from_raw_parts(rf, h),
            slice::from_raw_parts(ls, n),
            slice::from_raw_parts(w, n),
            slice::from_raw_parts_mut(out, len),
            slice::from_raw_parts_mut(cash, h),
        )
    };
    let mut v = w.to_vec();
    let mut c = 0.0;
    for s in 0..h {
        c *= 1.0 + rf[s];
        for k in 0..n {
            if v[k] > 0.0 {
                let q = r[s * n + k];
                if q.is_finite() {
                    v[k] *= 1.0 + q;
                } else if s as i64 > ls[k] {
                    c += v[k] * (1.0 + rf[s]);
                    v[k] = 0.0;
                } else {
                    return 1;
                }
            }
            out[s * n + k] = v[k];
        }
        cash[s] = c;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delisting_and_missing() {
        let r = [0.1, -1.0, f64::NAN, f64::NAN];
        let mut out = [0.0; 4];
        let mut cash = [0.0; 2];
        let rf = [0.01; 2];
        let w = [0.5; 2];
        let ls = [0_i64, 2];
        let st = unsafe {
            pf_hold(
                r.as_ptr(),
                rf.as_ptr(),
                ls.as_ptr(),
                w.as_ptr(),
                2,
                2,
                out.as_mut_ptr(),
                cash.as_mut_ptr(),
            )
        };
        assert_eq!(st, 0);
        assert_eq!(out, [0.55, 0.0, 0.0, 0.0]);
        assert_eq!(cash, [0.0, 0.55 * 1.01]);
        let ls = [2_i64; 2];
        let st = unsafe {
            pf_hold(
                r.as_ptr(),
                rf.as_ptr(),
                ls.as_ptr(),
                w.as_ptr(),
                2,
                2,
                out.as_mut_ptr(),
                cash.as_mut_ptr(),
            )
        };
        assert_eq!(st, 1);
    }
}
