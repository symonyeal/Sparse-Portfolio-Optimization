//! F1 and F1p: cardinality-constrained mean-variance selection, solved by SCIP.
//!
//! F1  (Chang et al., 2000, eq. (16)): min lam w'Sw - (1 - lam) mu'w over X(K); with lam = 1 and
//! ```text
//! a return row mu'w = rho it is their eqs. (8)-(13).
//! ```
//! F1p (Frangioni and Gentile, 2006; Gunluk and Linderoth, 2010, eqs. (17)-(23)): S = Q + diag(pi),
//! ```text
//! pi_i w_i^2 replaced by pi_i t_i with w_i^2 <= t_i z_i. Same optimum, tighter relaxation.
//! ```
//!
//! The quadratic enters through an epigraph v >= w'Sw, because SCIP takes a linear objective. The
//! objective is multiplied by sg = 1 / mean(diag S) and the certificate divided back: SCIP's
//! tolerances are absolute below one.
//!
//! Gunluk and Linderoth take pi = (l_min - 0.001) 1. At the scale of weekly returns l_min is near
//! 1e-4, so the absolute 0.001 makes pi negative; here pi = TH l_min keeps Q positive definite.
//!
//! `qp` returns a point of the continuous frontier (K = N, e = 0) with a KKT certificate built
//! from the active set; it is the exact reference for gate G1.

use crate::crt::{Sense, Sol, limits};
use crate::est::{lmin, ms, scale};
use crate::lp::Lp;
use crate::scip;
use crate::x::{Xk, ret};
use nalgebra::{DMatrix, DVector};
use russcip::Variable;
use russcip::prelude::*;

// Legend
// mu, S  : mean vector and covariance matrix
// lam    : risk-return weight of Chang et al. (2000), in [0, 1]
// rho    : required return with the sense of its row, (rho, true) for mu'w = rho
// p      : true for the perspective formulation F1p
// Q, pi  : perspective split S = Q + diag(pi), Q positive semidefinite, pi >= 0
// v, t   : epigraph variable of the quadratic; perspective variables w_i^2 <= t_i z_i
// tl, gp : time limit (s) and requested relative gap; ft : SCIP feasibility tolerance
// F, y   : free set of an active-set solve; multipliers of the budget and return rows

/// Fraction of the least eigenvalue placed in pi.
pub const TH: f64 = 0.999;
/// SCIP feasibility tolerance of every SCIP solve.
pub const FT: f64 = 1e-9;

/// lam w'Sw - (1 - lam) mu'w at w.
pub fn obj(w: &[f64], mu: &[f64], s: &DMatrix<f64>, lam: f64) -> f64 {
    lam * quad(w, s) - (1.0 - lam) * dot(mu, w)
}

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// w'Sw.
pub fn quad(w: &[f64], s: &DMatrix<f64>) -> f64 {
    let w = DVector::from_column_slice(w);
    w.dot(&(s * &w))
}

/// Frangioni-Gentile diagonal split S = Q + diag(pi), pi = TH * max(0, l_min(S)) 1.
pub fn split(s: &DMatrix<f64>) -> (DMatrix<f64>, Vec<f64>) {
    let pi = vec![TH * lmin(s).max(0.0); s.nrows()];
    (s - DMatrix::from_diagonal(&DVector::from_column_slice(&pi)), pi)
}

/// sum_ij A_ij w_i w_j as a SCIP expression.
fn qx(a: &DMatrix<f64>, w: &[Variable]) -> Expr {
    let n = w.len();
    Expr::sum(
        (0..n)
            .flat_map(|i| (0..n).map(move |j| (i, j)))
            .map(|(i, j)| a[(i, j)] * (Expr::var(&w[i]) * Expr::var(&w[j]))),
    )
}

/// Solve F1 (p false) or F1p (p true) with SCIP.
pub fn f1(
    mu: &[f64],
    s: &DMatrix<f64>,
    x: Xk,
    lam: f64,
    rho: Option<(f64, bool)>,
    p: bool,
    tl: Option<f64>,
    gp: f64,
) -> Result<Sol, String> {
    ms(mu, s)?;
    limits(tl, gp)?;
    if !(0.0..=1.0).contains(&lam) {
        return Err("lam must lie in [0, 1]".into());
    }
    let n = mu.len();
    let sg = scale(s);
    let ss = s * sg;
    let mut lp = Lp::default();
    x.set(&mut lp);
    for i in 0..n {
        lp.c[i] = -(1.0 - lam) * sg * mu[i];
    }
    if let Some((r, eq)) = rho {
        ret(&mut lp, mu, r, eq)?;
    }
    let v = lp.col(lam, 0.0, f64::INFINITY, false);
    let t0 = lp.c.len();
    if p {
        for _ in 0..n {
            lp.col(0.0, 0.0, f64::INFINITY, false);
        }
    }
    let mut sp = scip::open(&lp, Sense::Min, FT, gp)?;
    let w = &sp.v[..n];
    let e = if p {
        let (q, pi) = split(&ss);
        let t = &sp.v[t0..t0 + n];
        for i in 0..n {
            let wi = Expr::var(&w[i]);
            let c = wi.clone() * wi - Expr::var(&t[i]) * Expr::var(&sp.v[n + i]);
            sp.m.add(cons().expression(c).le(0.0));
        }
        qx(&q, w) + Expr::sum((0..n).map(|i| pi[i] * Expr::var(&t[i])))
    } else {
        qx(&ss, w)
    };
    let e = e - Expr::var(&sp.v[v]);
    sp.m.add(cons().expression(e).le(0.0));
    let (xv, c) = scip::solve(sp, Sense::Min, tl)?;
    Ok(Sol::of(xv, n, c.un(sg), |w| obj(w, mu, s, lam)))
}

/// Minimum-variance point at mean r, with its certificate.
pub struct Qp {
    pub w: Vec<f64>,
    pub f: f64,
    /// KKT residuals: stationarity, primal, dual sign, lower and upper bound.
    pub kt: [f64; 5],
}

/// Min w'Sw s.t. mu'w = r, 1'w = 1, 0 <= w <= 1, by an active-set solve from w0's support.
///
/// For a convex QP a point satisfying stationarity, primal feasibility and the multiplier signs
/// is optimal; the residuals are returned. The KKT systems are solved by least squares, as the
/// free set may make them singular.
pub fn qp(mu: &[f64], s: &DMatrix<f64>, r: f64, w0: &[f64], tol: f64, it: usize) -> Result<Qp, String> {
    let n = mu.len();
    let mut f: Vec<usize> = (0..n).filter(|&i| w0[i] > 1e-7).collect();
    let a = |k: usize, i: usize| if k == 0 { 1.0 } else { mu[i] };
    let b = [1.0, r];
    for _ in 0..it {
        let m = f.len();
        let mm = DMatrix::from_fn(m + 2, m + 2, |i, j| match (i < m, j < m) {
            (true, true) => 2.0 * s[(f[i], f[j])],
            (true, false) => -a(j - m, f[i]),
            (false, true) => a(i - m, f[j]),
            _ => 0.0,
        });
        let q = DVector::from_fn(m + 2, |i, _| if i < m { 0.0 } else { b[i - m] });
        let sv = mm.svd(true, true);
        let cut = f64::EPSILON * (m + 2) as f64 * sv.singular_values.max();
        let x = sv.solve(&q, cut).map_err(|e| e.to_string())?;
        let mut w = vec![0.0; n];
        for (k, &i) in f.iter().enumerate() {
            w[i] = x[k];
        }
        let y = [x[m], x[m + 1]];
        let sw = s * DVector::from_column_slice(&w);
        let g: Vec<f64> = (0..n).map(|i| 2.0 * sw[i] - a(0, i) * y[0] - a(1, i) * y[1]).collect();
        let neg: Vec<usize> = f.iter().copied().filter(|&i| w[i] < -tol).collect();
        if let Some(&k) = neg.iter().min_by(|&&i, &&j| w[i].total_cmp(&w[j])) {
            f.retain(|&i| i != k);
            continue;
        }
        let off: Vec<usize> = (0..n).filter(|i| !f.contains(i)).collect();
        let gm = g.iter().fold(1.0f64, |m, v| m.max(v.abs()));
        if let Some(&k) = off.iter().min_by(|&&i, &&j| g[i].total_cmp(&g[j]))
            && g[k] < -tol * gm
        {
            let p = f.partition_point(|&i| i < k);
            f.insert(p, k);
            continue;
        }
        let pr = (0..2).map(|k| ((0..n).map(|i| a(k, i) * w[i]).sum::<f64>() - b[k]).abs()).fold(0.0, f64::max);
        let kt = [
            f.iter().map(|&i| g[i].abs()).fold(0.0, f64::max),
            pr,
            off.iter().map(|&i| -g[i]).fold(0.0, f64::max),
            w.iter().map(|v| -v).fold(0.0, f64::max),
            w.iter().map(|v| v - 1.0).fold(0.0, f64::max),
        ];
        let fv = quad(&w, s);
        return Ok(Qp { w, f: fv, kt });
    }
    Err("active-set solve did not settle".into())
}

/// Chang et al. (2000), sec. 5.2.2: percentage deviation of the point (std x, mean y) from the
/// linearly interpolated frontier ef = (mean, variance) rows; the lesser of the horizontal and
/// vertical deviations, NaN where neither bracket exists.
///
/// The source prints 100(y* - y**)/y** for the vertical direction, which is negative for a
/// point below the frontier; both directions are taken in absolute value here.
pub fn pdev(ef: &[(f64, f64)], x: f64, y: f64) -> f64 {
    let mut o: Vec<(f64, f64)> = ef.iter().map(|&(m, v)| (m, v.sqrt())).collect();
    o.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (fy, fx): (Vec<f64>, Vec<f64>) = o.into_iter().unzip();
    let mut d = f64::NAN;
    if fy[0] <= y && y <= fy[fy.len() - 1] {
        let x2 = interp(y, &fy, &fx);
        d = (100.0 * (x - x2) / x2).abs();
    }
    if fx[0] <= x && x <= fx[fx.len() - 1] {
        let y2 = interp(x, &fx, &fy);
        d = d.min((100.0 * (y - y2) / y2).abs());
    }
    d
}

/// numpy.interp for a point inside [xp_0, xp_last].
fn interp(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    let j = xp.partition_point(|&v| v <= x).saturating_sub(1);
    if j + 1 >= xp.len() {
        return fp[xp.len() - 1];
    }
    fp[j] + (x - xp[j]) * (fp[j + 1] - fp[j]) / (xp[j + 1] - xp[j])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crt::St;
    use crate::x::Sn;

    #[test]
    fn perspective_split_of_singular_covariance_is_zero() {
        let s = DMatrix::from_row_slice(2, 2, &[1.0, 1.0, 1.0, 1.0]);
        let (q, pi) = split(&s);
        assert!(pi.iter().all(|&p| p.abs() < 1e-15) && (q - &s).amax() < 1e-15);
        let mu = [0.01, 0.02];
        let x = Xk::new(2, 1, 0.0, 1.0, Sn::Le).unwrap();
        let a = f1(&mu, &s, x, 0.5, None, true, Some(10.0), 0.0).unwrap();
        assert_eq!(a.c.st, St::Opt);
        assert_eq!(a.w.unwrap(), [0.0, 1.0]);
    }

    #[test]
    fn zero_deadline_starts_no_solve() {
        let s = DMatrix::identity(2, 2);
        let x = Xk::new(2, 1, 0.0, 1.0, Sn::Le).unwrap();
        let a = f1(&[0.01, 0.02], &s, x, 1.0, None, false, Some(0.0), 0.0).unwrap();
        assert!(a.w.is_none() && a.c.st == St::Limit);
    }

    #[test]
    fn indefinite_covariance_is_rejected() {
        let s = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 2.0, 1.0]);
        let x = Xk::new(2, 1, 0.0, 1.0, Sn::Le).unwrap();
        assert!(f1(&[0.0, 0.0], &s, x, 1.0, None, false, None, 0.0).is_err());
    }

    #[test]
    fn deviation_from_interpolated_frontier() {
        let ef = [(0.0, 1.0), (1.0, 4.0)];
        assert!((pdev(&ef, 1.5, 0.5) - 0.0).abs() < 1e-12);
        // horizontal |1.8 - 1.5| / 1.5 = 20%, vertical |0.5 - 0.8| / 0.8 = 37.5%
        assert!((pdev(&ef, 1.8, 0.5) - 20.0).abs() < 1e-9);
        assert!(pdev(&ef, 5.0, 3.0).is_nan());
    }
}
