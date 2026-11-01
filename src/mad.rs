//! F2: mean-absolute-deviation selection over X(K), a MILP solved by HiGHS.
//!
//! Konno and Yamazaki (1991), eqs. (3.2)-(3.6), with X(K) in place of 0 <= x_j <= u_j:
//!
//! ```text
//! min (1/S) sum_s d_s   s.t.  d_s + a_s'w >= 0,  d_s - a_s'w >= 0,  mu'w >= rho,  (w, z) in X(K)
//! ```
//!
//! where a_s = r_s - mu and mu is the sample mean (their eq. (3.2)). At an optimum d_s = |a_s'w|.
//! The source's epigraph form replaces the split d+_s - d-_s = y_s of the specification; the two
//! are equivalent and this one is the paper's.

use crate::crt::Sol;
use crate::est::{centre, mom, rr};
use crate::lp::Lp;
use crate::x::{Xk, ret};
use nalgebra::DMatrix;

// Legend
// r      : scenario returns (S, N); a : centred returns a_s = r_s - mu
// d      : epigraph variables d_s >= |a_s'w|
// rho    : required mean return (None: no return row); tl, gp : time limit (s) and gap

/// Solve F2; f is the deviation recomputed from the returns.
pub fn mad(r: &DMatrix<f64>, x: Xk, rho: Option<f64>, tl: Option<f64>, gp: f64) -> Result<Sol, String> {
    rr(r)?;
    let (s, n) = r.shape();
    let (mu, _) = mom(r);
    let a = centre(r, &mu);
    let mut p = Lp::default();
    x.set(&mut p);
    if let Some(rho) = rho {
        ret(&mut p, &mu, rho, false)?;
    }
    let d0 = p.c.len();
    for _ in 0..s {
        p.col(1.0 / s as f64, 0.0, f64::INFINITY, false);
    }
    for sg in [1.0, -1.0] {
        for k in 0..s {
            let mut row: Vec<_> = (0..n).map(|i| (i, sg * a[(k, i)])).collect();
            row.push((d0 + k, 1.0));
            p.row(row, 0.0, f64::INFINITY);
        }
    }
    let (xv, c) = p.solve(tl, gp)?;
    Ok(Sol::of(xv, n, c, |w| dev(r, w)))
}

/// (1/S) sum_s |(r_s - mu)'w|, recomputed from the returns at w.
pub fn dev(r: &DMatrix<f64>, w: &[f64]) -> f64 {
    let (mu, _) = mom(r);
    let a = centre(r, &mu);
    (0..r.nrows()).map(|k| (0..w.len()).map(|i| a[(k, i)] * w[i]).sum::<f64>().abs()).sum::<f64>() / r.nrows() as f64
}
