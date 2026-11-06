//! F5: mean-variance-skewness selection over X(K), a nonconvex MINLP solved by SCIP.
//!
//! Harvey, Liechty, Liechty and Mueller (2010), eqs. (3)-(6): utility linear in the first three
//! moments, taken with plug-in sample moments (their "parameter utility"):
//!
//! ```text
//! max mu'w - (g/2) w'Sw + (k/6) (1/S) sum_s y_s^3,   y_s = (r_s - mu)'w,   (w, z) in X(K)
//! ```
//!
//! (1/S) sum_s y_s^3 is the sample third central moment of r'w, w'M3(w kron w) in Jondeau and
//! Rockinger (2006). The default k = g(g + 1) is U'''(1)/U'(1) for CRRA utility with relative
//! risk aversion g, at unit wealth; Jondeau and Rockinger expand about the expected wealth
//! 1 + mu'w instead, so the two agree only to first order in mu'w. The cubic makes the objective
//! nonconvex; only SCIP's spatial branch-and-bound dual bound certifies it.

use crate::crt::{Sense, Sol, limits};
use crate::est::{centre, mom, ms, rr, scale};
use crate::lp::Lp;
use crate::mv::{FT, dot, quad};
use crate::scip;
use crate::x::Xk;
use nalgebra::DMatrix;
use russcip::prelude::*;

// Legend
// r      : scenario returns (S, N); a : centred returns
// mu, Sg : sample mean and covariance (1/S) of r, unless Sg is supplied (e.g. shrinkage)
// g, k   : variance aversion gamma and skewness preference kappa
// y      : centred portfolio returns y_s = a_s'w, bounded by the signs of a_s over 0 <= w <= 1
// q      : epigraph of the objective; sg : objective scale 1 / mean(diag Sg)

/// CRRA third-to-first derivative ratio at unit wealth: g (g + 1).
pub fn kap(g: f64) -> f64 {
    g * (g + 1.0)
}

/// mu'w - (g/2) w'Sg w + (k/6) mean(y^3) at w.
pub fn obj(w: &[f64], r: &DMatrix<f64>, g: f64, k: f64, mu: &[f64], sg: &DMatrix<f64>) -> f64 {
    let (m0, _) = mom(r);
    let a = centre(r, &m0);
    let m3 = (0..r.nrows()).map(|s| (0..w.len()).map(|i| a[(s, i)] * w[i]).sum::<f64>().powi(3)).sum::<f64>()
        / r.nrows() as f64;
    dot(mu, w) - 0.5 * g * quad(w, sg) + k / 6.0 * m3
}

/// Solve F5 with SCIP (maximisation); Sg defaults to the sample covariance.
pub fn f5(
    r: &DMatrix<f64>,
    x: Xk,
    g: f64,
    k: f64,
    sg: Option<&DMatrix<f64>>,
    tl: Option<f64>,
    gp: f64,
) -> Result<Sol, String> {
    rr(r)?;
    x.dimension(r.ncols())?;
    limits(tl, gp)?;
    if !g.is_finite() || !k.is_finite() {
        return Err("g and k must be finite".into());
    }
    let (s, n) = r.shape();
    let (mu, s0) = mom(r);
    let sm = sg.unwrap_or(&s0);
    ms(&mu, sm)?;
    let a = centre(r, &mu);
    let c = scale(sm);
    let mut p = Lp::default();
    let cols = x.set(&mut p);
    let y0 = p.c.len();
    for t in 0..s {
        let lo: f64 = a.row(t).iter().map(|v| v.min(0.0)).sum();
        let hi: f64 = a.row(t).iter().map(|v| v.max(0.0)).sum();
        let y = p.col(0.0, lo, hi, false);
        let mut row: Vec<_> = (0..n).map(|i| (cols.w.start + i, -a[(t, i)])).collect();
        row.push((y, 1.0));
        p.row(row, 0.0, 0.0);
    }
    let q = p.col(1.0, f64::NEG_INFINITY, f64::INFINITY, false);
    let mut sp = scip::open(&p, Sense::Max, FT, gp)?;
    let v = &sp.v;
    let w = &v[cols.w.clone()];
    let lin = Expr::sum((0..n).map(|i| c * mu[i] * Expr::var(&w[i])));
    let qd = Expr::sum(
        (0..n)
            .flat_map(|i| (0..n).map(move |j| (i, j)))
            .map(|(i, j)| sm[(i, j)] * (Expr::var(&w[i]) * Expr::var(&w[j]))),
    );
    // y*y*y, not pow(y, 3): SCIP 10 handles the monomial far better here (35 against 1228
    // nodes on the 36 x 6 lattice case, and an 11x weaker 60 s bound on indtrack1, K = 3).
    let cub = Expr::sum((0..s).map(|t| {
        let y = Expr::var(&v[y0 + t]);
        y.clone() * y.clone() * y
    }));
    let e = lin - 0.5 * g * c * qd + (k * c / (6.0 * s as f64)) * cub - Expr::var(&v[q]);
    sp.m.add(cons().expression(e).ge(0.0));
    let (xv, cert) = scip::solve(sp, Sense::Max, tl)?;
    Ok(Sol::of(xv, cols, cert.un(c), |w| obj(w, r, g, k, &mu, sm)))
}
