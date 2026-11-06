//! F3: CVaR selection over X(K), a MILP solved by HiGHS.
//!
//! Rockafellar and Uryasev (2000), eq. (17) with rows (15)-(16) and X(K) in place of their (11):
//!
//! ```text
//! min al + 1/((1 - b) S) sum_s u_s
//! s.t. u_s >= -r_s'w - al,  u_s >= 0,  mu'w >= rho,  (w, z) in X(K)
//! ```
//!
//! The loss of scenario s is -r_s'w. `cvar` recomputes beta-CVaR at w by sorting the S losses,
//! with the fractional atom at the beta-quantile when (1 - b) S is not an integer; by their
//! Theorem 1 it equals the optimal value of the program at that w.

use crate::crt::Sol;
use crate::est::{mom, rr};
use crate::lp::Lp;
use crate::x::{Xk, ret};
use nalgebra::DMatrix;

// Legend
// r      : scenario returns (S, N), each scenario with probability 1/S
// b      : CVaR level beta in (0, 1); al : VaR variable alpha (free)
// u      : tail excess u_s = max(0, loss_s - alpha)
// L      : scenario losses, sorted ascending in `cvar`; m : tail mass in scenarios

/// Solve F3; f is the CVaR recomputed from the sorted losses.
pub fn cv(r: &DMatrix<f64>, x: Xk, b: f64, rho: Option<f64>, tl: Option<f64>, gp: f64) -> Result<Sol, String> {
    rr(r)?;
    x.dimension(r.ncols())?;
    if !(0.0 < b && b < 1.0) {
        return Err("b must lie in (0, 1)".into());
    }
    let (s, n) = r.shape();
    let mut p = Lp::default();
    let cols = x.set(&mut p);
    if let Some(rho) = rho {
        ret(&mut p, &cols, &mom(r).0, rho, false)?;
    }
    let al = p.col(1.0, f64::NEG_INFINITY, f64::INFINITY, false);
    for k in 0..s {
        let u = p.col(1.0 / ((1.0 - b) * s as f64), 0.0, f64::INFINITY, false);
        let mut row: Vec<_> = (0..n).map(|i| (cols.w.start + i, r[(k, i)])).collect();
        row.extend([(al, 1.0), (u, 1.0)]); // r_s'w + al + u_s >= 0
        p.row(row, 0.0, f64::INFINITY);
    }
    let (xv, c) = p.solve(tl, gp)?;
    Ok(Sol::of(xv, cols, c, |w| cvar(&losses(r, w), b)))
}

/// Scenario losses -r_s'w.
pub fn losses(r: &DMatrix<f64>, w: &[f64]) -> Vec<f64> {
    (0..r.nrows()).map(|k| -(0..w.len()).map(|i| r[(k, i)] * w[i]).sum::<f64>()).collect()
}

/// beta-CVaR of equally weighted losses L, from the sorted losses.
pub fn cvar(l: &[f64], b: f64) -> f64 {
    assert!(0.0 < b && b < 1.0, "b must lie in (0, 1)");
    let mut l = l.to_vec();
    l.sort_by(f64::total_cmp);
    let s = l.len();
    let m = (1.0 - b) * s as f64;
    let k = m.floor() as usize;
    let tt: f64 = l[s - k..].iter().sum();
    let fr = m - k as f64; // fractional atom at the beta-quantile
    let atom = if fr > 0.0 { l[s - k - 1] * fr } else { 0.0 };
    (tt + atom) / m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cvar_by_sorting_hand_case() {
        // losses 1..10, b = 0.75: mass 2.5 = 10 + 9 + 0.5 * 8
        let l: Vec<f64> = (1..=10).map(f64::from).collect();
        assert!((cvar(&l, 0.75) - 23.0 / 2.5).abs() < 1e-15);
        assert_eq!(cvar(&l, 0.9), 10.0);
    }

    #[test]
    fn tiny_fractional_tail_atom_is_kept() {
        let l = [0.0, 1.0, 2.0];
        let b = 1.0 - 0.1 / 3.0;
        assert!((cvar(&l, b) - 2.0).abs() < 1e-12);
    }
}
