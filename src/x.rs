//! The feasible set X(K) of a cardinality-constrained long-only portfolio, written once.
//!
//! Chang, Meade, Beasley and Sharaiha (2000), eqs. (10)-(13), with the sense of (11) a parameter:
//!
//! ```text
//! X(K) = { (w, z) : 1'w = 1,  e z_i <= w_i <= d z_i,  1'z (<= or =) K,  z in {0,1}^N }
//! ```
//!
//! Columns are ordered x = [w, z, extra...]: `set` writes the first 2N into an empty program and
//! a formulation appends its own after them. The return row mu'w (>= or =) rho is `ret`.
//!
//! With e = 0 and sense `Eq`, z_i = 1 marks an admitted name, not a positive holding: a
//! solution may carry w_i = 0 with z_i = 1. `ok` therefore checks the rows, and `nz` counts
//! holdings.

use crate::lp::Lp;

// Legend
// N      : number of assets; K : cardinality limit
// e, d   : floor eps and ceiling delta on a held weight, 0 <= e <= d <= 1
// sn     : sense of the cardinality row; Chang et al. (2000) impose `Eq`
// mu,rho : mean return vector and required mean return

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sn {
    Le,
    Eq,
}

#[derive(Clone, Copy, Debug)]
pub struct Xk {
    pub n: usize,
    pub k: usize,
    pub e: f64,
    pub d: f64,
    pub sn: Sn,
}

impl Xk {
    /// Validated parameters of X(K).
    pub fn new(n: usize, k: usize, e: f64, d: f64, sn: Sn) -> Result<Xk, String> {
        if n < 1 || !(1..=n).contains(&k) {
            return Err(format!("need integer 1 <= K <= N, got K={k}, N={n}"));
        }
        if !(e.is_finite() && d.is_finite() && 0.0 <= e && e <= d && d <= 1.0) {
            return Err("need 0 <= e <= d <= 1".into());
        }
        Ok(Xk { n, k, e, d, sn })
    }

    /// Write the columns [w, z] and the rows of X(K) into the empty program p.
    pub fn set(&self, p: &mut Lp) {
        assert!(p.c.is_empty(), "X(K) owns the first 2N columns");
        let n = self.n;
        let inf = f64::INFINITY;
        for _ in 0..n {
            p.col(0.0, 0.0, self.d, false);
        }
        for _ in 0..n {
            p.col(0.0, 0.0, 1.0, true);
        }
        p.row((0..n).map(|i| (i, 1.0)).collect(), 1.0, 1.0);
        for i in 0..n {
            p.row(vec![(i, 1.0), (n + i, -self.e)], 0.0, inf);
        }
        for i in 0..n {
            p.row(vec![(i, 1.0), (n + i, -self.d)], -inf, 0.0);
        }
        let k = self.k as f64;
        p.row((n..2 * n).map(|i| (i, 1.0)).collect(), if self.sn == Sn::Eq { k } else { -inf }, k);
    }

    /// Violated conditions of (w, z) in X(K), checked from the definition; empty if none.
    pub fn ok(&self, w: &[f64], z: &[f64], tol: f64) -> Vec<String> {
        if w.len() != self.n || z.len() != self.n {
            return vec!["w and z must have length N".into()];
        }
        if w.iter().chain(z).any(|x| !x.is_finite()) {
            return vec!["w and z must be finite".into()];
        }
        let mut v = Vec::new();
        if z.iter().any(|&x| (x - x.round()).abs() > tol || (x.round() != 0.0 && x.round() != 1.0)) {
            v.push("z not binary".into());
        }
        let s: f64 = w.iter().sum();
        if (s - 1.0).abs() > tol {
            v.push(format!("1'w = {s:.12}"));
        }
        if w.iter().zip(z).any(|(&w, &z)| w < self.e * z.round() - tol) {
            v.push("floor e z_i <= w_i violated".into());
        }
        if w.iter().zip(z).any(|(&w, &z)| w > self.d * z.round() + tol) {
            v.push("ceiling w_i <= d z_i violated".into());
        }
        let k = z.iter().map(|x| x.round()).sum::<f64>() as usize;
        if (self.sn == Sn::Le && k > self.k) || (self.sn == Sn::Eq && k != self.k) {
            v.push(format!("1'z = {k}, K = {} ({:?})", self.k, self.sn));
        }
        v
    }
}

/// The return row mu'w >= rho (`eq` false) or mu'w = rho over the w columns of p.
pub fn ret(p: &mut Lp, mu: &[f64], rho: f64, eq: bool) -> Result<(), String> {
    if mu.iter().any(|x| !x.is_finite()) || !rho.is_finite() {
        return Err("mu must be finite and rho must be finite".into());
    }
    p.row(mu.iter().copied().enumerate().collect(), rho, if eq { rho } else { f64::INFINITY });
    Ok(())
}

/// Number of positive holdings.
pub fn nz(w: &[f64]) -> usize {
    w.iter().filter(|&&x| x > 1e-9).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_match_definition() {
        let x = Xk::new(3, 2, 0.1, 0.8, Sn::Le).unwrap();
        let mut p = Lp::default();
        x.set(&mut p);
        assert_eq!((p.c.len(), p.a.len()), (6, 8));
        let (w, z) = ([0.5, 0.5, 0.0], [1.0, 1.0, 0.0]);
        assert!(x.ok(&w, &z, 1e-9).is_empty());
        let xz: Vec<f64> = w.iter().chain(&z).copied().collect();
        for (a, (lo, hi)) in p.a.iter().zip(p.lo.iter().zip(&p.hi)) {
            let v: f64 = a.iter().map(|&(j, c)| c * xz[j]).sum();
            assert!(*lo - 1e-12 <= v && v <= *hi + 1e-12);
        }
        assert!(!x.ok(&[0.9, 0.1, 0.0], &z, 1e-9).is_empty());
        assert!(!x.ok(&[0.4, 0.3, 0.3], &[1.0; 3], 1e-9).is_empty());
    }

    #[test]
    fn exact_cardinality_admits_zero_weight_with_zero_floor() {
        let x = Xk::new(3, 2, 0.0, 1.0, Sn::Eq).unwrap();
        assert!(x.ok(&[1.0, 0.0, 0.0], &[1.0, 1.0, 0.0], 1e-9).is_empty());
        assert_eq!(nz(&[1.0, 0.0, 0.0]), 1);
    }

    #[test]
    fn bad_parameters_and_points_are_refused() {
        assert!(Xk::new(3, 4, 0.0, 1.0, Sn::Le).is_err());
        assert!(Xk::new(3, 0, 0.0, 1.0, Sn::Le).is_err());
        assert!(Xk::new(3, 2, 0.5, 0.4, Sn::Le).is_err());
        let x = Xk::new(2, 1, 0.0, 1.0, Sn::Le).unwrap();
        assert!(!x.ok(&[f64::NAN, 1.0], &[0.0, 1.0], 1e-9).is_empty());
    }
}
