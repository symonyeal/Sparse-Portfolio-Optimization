//! Delisting-adjusted returns, Bessembinder's (2018) wealth creation, and the set T.
//!
//! Wealth creation, Bessembinder (2018), eq. (3): for a stock with beginning-of-month market
//! value I_{t-1}, return R_t and T-bill return rf_t,
//!
//! ```text
//! W_T - W_0 FV_{0,T} = sum_t I_{t-1} (R_t - rf_t) FV_{t,T},   FV_{t,T} = prod_{u=t+1..T} (1 + rf_u),
//! ```
//!
//! summed across a firm's share classes (PERMCO). T is the smallest set of top firms whose wealth
//! creation sums to the net wealth creation of all firms.
//!
//! Delisting: R = (1 + RET)(1 + DLRET) - 1; DLRET alone when RET is missing in the delisting
//! month. A missing DLRET after a performance delisting is filled, when asked, with -30% on
//! NYSE/AMEX (Shumway, 1997: codes 500, 520-584) and -55% on Nasdaq (Shumway and Warther, 1999:
//! codes 500, 505-588). Nothing missing is read as zero.

use std::collections::BTreeMap;

// Legend
// ret, dl  : CRSP RET and DLRET (NaN when missing); dc : DLSTCD (0 if none); ex : EXCHCD
// fill     : apply the Shumway fill to a missing DLRET after a performance delisting
// I, R, rf : beginning-of-month value, return and T-bill return, month by month to the end
// W        : wealth creation per stock or per firm; pc : PERMCO of each stock

/// True where DLSTCD dc is a performance delisting on exchange ex, by the sources' ranges.
pub fn perf(dc: i64, ex: i64) -> bool {
    let na = matches!(ex, 1 | 2) && (dc == 500 || (520..=584).contains(&dc));
    let nq = ex == 3 && (dc == 500 || (505..=588).contains(&dc));
    na || nq
}

/// Shumway's replacement delisting return: -0.30 NYSE/AMEX, -0.55 Nasdaq, none elsewhere.
pub fn sw(ex: i64) -> f64 {
    match ex {
        3 => -0.55,
        1 | 2 => -0.30,
        _ => f64::NAN,
    }
}

/// Combined monthly return and the count of Shumway fills applied.
pub fn comb(ret: &[f64], dl: &[f64], dc: &[i64], ex: &[i64], fill: bool) -> Result<(Vec<f64>, usize), String> {
    if !(ret.len() == dl.len() && dl.len() == dc.len() && dc.len() == ex.len()) {
        return Err("delisting arrays disagree in shape".into());
    }
    if ret.iter().chain(dl).any(|x| x.is_infinite() || *x < -1.0) {
        return Err("return must be missing or finite and at least -1".into());
    }
    let mut n = 0;
    let r = (0..ret.len())
        .map(|k| {
            let mut d = dl[k];
            if fill && d.is_nan() && perf(dc[k], ex[k]) {
                d = sw(ex[k]);
                n += usize::from(d.is_finite());
            }
            match (ret[k].is_nan(), d.is_nan()) {
                (_, true) => ret[k],
                (true, false) => d,
                (false, false) => (1.0 + ret[k]) * (1.0 + d) - 1.0,
            }
        })
        .collect();
    Ok((r, n))
}

/// Forward factor of each month: the product of (1 + rf) over the later months.
pub fn fv(rf: &[f64]) -> Vec<f64> {
    let mut g = vec![1.0; rf.len()];
    for t in (0..rf.len().saturating_sub(1)).rev() {
        g[t] = g[t + 1] * (1.0 + rf[t + 1]);
    }
    g
}

/// Bessembinder eq. (3) for one stock.
pub fn wc(i: &[f64], r: &[f64], rf: &[f64]) -> Result<f64, String> {
    if i.len() != r.len() || r.len() != rf.len() {
        return Err("wealth-creation arrays disagree in shape".into());
    }
    if i.iter().chain(r).chain(rf).any(|x| !x.is_finite()) {
        return Err("wealth creation needs every month's value and return; none is zero".into());
    }
    Ok(fv(rf).iter().enumerate().map(|(t, f)| i[t] * (r[t] - rf[t]) * f).sum())
}

/// Wealth creation summed by PERMCO.
pub fn firm(w: &[f64], pc: &[i64]) -> BTreeMap<i64, f64> {
    let mut o = BTreeMap::new();
    for (x, p) in w.iter().zip(pc) {
        *o.entry(*p).or_insert(0.0) += x;
    }
    o
}

/// Indices of the smallest set of largest W whose sum reaches sum(W) (Bessembinder's T).
pub fn top(w: &[f64]) -> Result<Vec<usize>, String> {
    if w.iter().any(|x| !x.is_finite()) {
        return Err("wealth creation must be a finite vector".into());
    }
    let s: f64 = w.iter().sum();
    if w.is_empty() || s <= 0.0 {
        return Ok(vec![]);
    }
    let mut o: Vec<usize> = (0..w.len()).collect();
    o.sort_by(|&a, &b| w[b].total_cmp(&w[a]));
    let mut c = 0.0;
    for (k, &i) in o.iter().enumerate() {
        c += w[i];
        if c >= s - 1e-9 * s.abs().max(1.0) {
            return Ok(o[..=k].to_vec());
        }
    }
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::cum;

    #[test]
    fn delisting_combination_and_shumway_fill() {
        let n = f64::NAN;
        let ret = [0.1, n, 0.1, n, n, 0.0];
        let dl = [n, -0.2, 0.5, n, n, n];
        let dc = [0, 550, 550, 552, 560, 100];
        let ex = [1, 1, 1, 2, 3, 3];
        let (r, k) = comb(&ret, &dl, &dc, &ex, false).unwrap();
        assert_eq!((r[0], r[1], k), (0.1, -0.2, 0));
        assert!((r[2] - 0.65).abs() < 1e-15 && r[3].is_nan() && r[4].is_nan());
        let (r, k) = comb(&ret, &dl, &dc, &ex, true).unwrap();
        assert_eq!((r[3], r[4], r[5], k), (-0.30, -0.55, 0.0, 2));
        assert!(comb(&[-1.5], &[n], &[0], &[1], false).is_err());
    }

    #[test]
    fn wealth_creation_identity_against_accounts() {
        // W_T - I_0 FV_0 for a fully reinvested stock equals sum_t I_{t-1}(R_t - rf_t) FV_t.
        let r = [0.05, -0.02, 0.1, 0.03];
        let rf = [0.01, 0.012, 0.009, 0.011];
        let i0 = 100.0;
        let w = cum(&r);
        let i: Vec<f64> = std::iter::once(i0).chain(w[..3].iter().map(|x| i0 * x)).collect();
        let lhs = i0 * w[3] - i0 * cum(&rf)[3];
        assert!((wc(&i, &r, &rf).unwrap() - lhs).abs() < 1e-11);
        assert!(wc(&[1.0, f64::NAN], &[0.0, 0.0], &[0.0, 0.0]).is_err());
    }

    #[test]
    fn top_set_and_firm_sums() {
        let f = firm(&[5.0, 1.0, -2.0, 3.0], &[7, 7, 8, 9]);
        assert_eq!(f.into_iter().collect::<Vec<_>>(), [(7, 6.0), (8, -2.0), (9, 3.0)]);
        assert_eq!(top(&[6.0, -2.0, 3.0]).unwrap(), [0, 2]);
        assert_eq!(top(&[8.0, -2.0, 1.0]).unwrap(), [0]);
        assert_eq!(top(&[3.0, 2.0, 1.0]).unwrap(), [0, 1, 2]);
        assert!(top(&[-1.0, 0.5]).unwrap().is_empty());
        assert!(top(&[]).unwrap().is_empty());
    }
}
