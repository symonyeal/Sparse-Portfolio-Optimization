//! F4: regression-based index tracking over X(K), restricted to zero transaction costs.
//!
//! Canakgoz and Beasley (2009), eqs. (1)-(11) and (16)-(26). Each stock's log return is regressed
//! on the index log return with an intercept, r_it = a_i + b_i R_t + e_it, and the portfolio's
//! coefficients are a'w and b'w. The model is lexicographic:
//!
//! ```text
//! stage 1   min D   s.t.  D >= a'w,  D >= -a'w,                            (w, z) in X(K)
//! stage 2   min E   s.t.  E >= b'w - 1,  E >= 1 - b'w,  |a'w - a*| <= ea,  (w, z) in X(K)
//! stage 3   min sum_i G_i  (transaction cost), with a and b held
//! ```
//!
//! Restriction, set by the owner on 2 October 2026: the budget is the shared fully invested one,
//! so c = f_is = f_ib = 0, G = 0, w = xV/C, and stage 3 is constant and omitted. The source's
//! cost rows (3)-(6) are therefore absent, not replaced. The source holds 1'z = K; ea is a
//! project setting (the paper says only that eps is small).

use crate::crt::{C, St};
use crate::est::ols;
use crate::lp::Lp;
use crate::x::Xk;
use nalgebra::{DMatrix, DVector};

// Legend
// r, ri  : stock log returns (T, N) and index log returns (T)
// a, b   : OLS intercepts and slopes of each stock on the index
// a1     : signed intercept a'w fixed after stage 1; ea : its stage-2 tolerance
// f1, f2 : objectives |a'w| and |b'w - 1| recomputed by regressing r w on ri

/// Stage-2 tolerance replacing the equality a'w = a* (Canakgoz-Beasley eq. (23)).
pub const EA: f64 = 1e-9;

/// The lexicographic result: the last stage's incumbent, its regression, both certificates.
#[derive(Clone, Debug)]
pub struct It {
    pub w: Option<Vec<f64>>,
    pub a: Option<f64>,
    pub b: Option<f64>,
    pub f1: Option<f64>,
    pub f2: Option<f64>,
    pub c1: C,
    pub c2: Option<C>,
    pub st: St,
}

/// Solve stages 1 and 2; the default sense of X(K) is the source's 1'z = K.
pub fn it(r: &DMatrix<f64>, ri: &DVector<f64>, x: Xk, ea: f64, tl: Option<f64>, gp: f64) -> Result<It, String> {
    if !ea.is_finite() || ea < 0.0 {
        return Err("ea must be finite and nonnegative".into());
    }
    if r.nrows() != ri.len() {
        return Err("stock and index returns must have the same observations".into());
    }
    let n = r.ncols();
    let (mut a, mut b) = (vec![0.0; n], vec![0.0; n]);
    for i in 0..n {
        (a[i], b[i]) = ols(&r.column(i).into_owned(), ri)?;
    }
    let mut p = Lp::default();
    x.set(&mut p);
    let d = p.col(1.0, 0.0, f64::INFINITY, false);
    // D >= q'w - t and D >= t - q'w
    let ab = |p: &mut Lp, q: &[f64], t: f64| {
        let mut lo: Vec<_> = (0..n).map(|i| (i, -q[i])).collect();
        lo.push((d, 1.0));
        p.row(lo, -t, f64::INFINITY);
        let mut hi: Vec<_> = (0..n).map(|i| (i, q[i])).collect();
        hi.push((d, 1.0));
        p.row(hi, t, f64::INFINITY);
    };
    let mut p1 = p.clone();
    ab(&mut p1, &a, 0.0);
    let (x1, c1) = p1.solve(tl, gp)?;
    let mut o = It { w: None, a: None, b: None, f1: None, f2: None, c1: c1.clone(), c2: None, st: c1.st };
    let Some(x1) = x1 else { return Ok(o) };
    let checked = |xs: &[f64], c: &C, stage: usize| -> Result<(f64, f64, f64), String> {
        let (w, z) = (&xs[..n], &xs[n..2 * n]);
        if !x.ok(w, z, 1e-7).is_empty() {
            return Err(format!("F4 stage {stage} incumbent violates X(K)"));
        }
        let (ai, bi) = ols(&(r * DVector::from_column_slice(w)), ri)?;
        let act = if stage == 1 { ai.abs() } else { (bi - 1.0).abs() };
        let close = |u: f64, v: f64, rt: f64| (u - v).abs() <= 1e-8 + rt * v.abs();
        if !c.f.is_finite()
            || !close(xs[d], c.f, 1e-5)
            || c.f < act - 1e-8
            || (c.st == St::Opt && !close(act, c.f, 1e-7))
        {
            return Err(format!("F4 stage {stage} objective disagrees with independent regression"));
        }
        Ok((ai, bi, act))
    };
    let (a1, b1, f1) = checked(&x1, &c1, 1)?;
    o = It { w: Some(x1[..n].to_vec()), a: Some(a1), b: Some(b1), f1: Some(f1), ..o };
    if c1.st != St::Opt {
        return Ok(o);
    }
    let mut p2 = p;
    ab(&mut p2, &b, 1.0);
    p2.row(a.iter().copied().enumerate().collect(), a1 - ea, a1 + ea);
    let (x2, c2) = p2.solve(tl, gp)?;
    o.st = c2.st;
    o.c2 = Some(c2.clone());
    let Some(x2) = x2 else { return Ok(o) };
    let (a2, b2, f2) = checked(&x2, &c2, 2)?;
    if (a2 - a1).abs() > ea + 1e-8 {
        return Err("F4 stage 2 violates the fixed stage-1 intercept".into());
    }
    Ok(It { w: Some(x2[..n].to_vec()), a: Some(a2), b: Some(b2), f2: Some(f2), ..o })
}

/// Tracking error: standard deviation (1/T) of r w - ri.
pub fn te(r: &DMatrix<f64>, ri: &DVector<f64>, w: &[f64]) -> f64 {
    let e = r * DVector::from_column_slice(w) - ri;
    let m = e.mean();
    (e.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / e.len() as f64).sqrt()
}
