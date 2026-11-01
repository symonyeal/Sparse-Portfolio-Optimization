//! Estimators from return scenarios: sample moments, Ledoit-Wolf shrinkage, index regression,
//! and the validation of a mean-covariance pair.

use nalgebra::{DMatrix, DVector};

// Legend
// r      : scenario returns (S, N), each row a scenario with probability 1/S
// mu, Sg : sample mean (N) and covariance with divisor S (N, N)
// y      : returns (T, N) of an estimation window
// dl     : shrinkage intensity in [0, 1]

/// A nonempty finite scenario matrix.
pub fn rr(r: &DMatrix<f64>) -> Result<(), String> {
    if r.is_empty() {
        return Err("r must be a nonempty two-dimensional scenario matrix".into());
    }
    if r.iter().any(|x| !x.is_finite()) {
        return Err("r must contain only finite returns".into());
    }
    Ok(())
}

/// A finite mean vector and symmetric positive-semidefinite covariance matrix.
pub fn ms(mu: &[f64], s: &DMatrix<f64>) -> Result<(), String> {
    let n = mu.len();
    if n == 0 || s.shape() != (n, n) {
        return Err("mu and S must have shapes (N,) and (N,N), N >= 1".into());
    }
    if mu.iter().chain(s.iter()).any(|x| !x.is_finite()) {
        return Err("mu and S must be finite".into());
    }
    let a = s.amax().max(f64::MIN_POSITIVE);
    if (0..n).any(|i| (0..n).any(|j| (s[(i, j)] - s[(j, i)]).abs() > 1e-12 * a)) {
        return Err("S must be symmetric".into());
    }
    if lmin(s) < -1e-12 * a {
        return Err("S must be positive semidefinite".into());
    }
    Ok(())
}

/// Least eigenvalue of a symmetric matrix.
pub fn lmin(s: &DMatrix<f64>) -> f64 {
    s.clone().symmetric_eigenvalues().min()
}

/// Objective scale 1 / mean(diag S); deterministic returns retain scale one.
pub fn scale(s: &DMatrix<f64>) -> f64 {
    let a = s.diagonal().mean();
    if a > f64::MIN_POSITIVE { 1.0 / a } else { 1.0 }
}

/// Sample mean and covariance with divisor S.
pub fn mom(r: &DMatrix<f64>) -> (Vec<f64>, DMatrix<f64>) {
    let mu: Vec<f64> = r.column_iter().map(|c| c.mean()).collect();
    let a = centre(r, &mu);
    let s = a.transpose() * &a / r.nrows() as f64;
    (mu, s)
}

/// r - 1 mu'.
pub fn centre(r: &DMatrix<f64>, mu: &[f64]) -> DMatrix<f64> {
    DMatrix::from_fn(r.nrows(), r.ncols(), |s, i| r[(s, i)] - mu[i])
}

/// Ledoit and Wolf (2004) shrinkage of the sample covariance toward constant correlation.
///
/// Appendix A, eq. (3): target F; Appendix B, eq. (5): intensity max(0, min(k/T, 1)) with
/// k = (pi - rho) / gamma. S has divisor T, the normalisation of the appendix's estimators
/// pi_ij and theta. Constant columns get zero rows and columns. Returns (matrix, intensity).
pub fn shrink(y: &DMatrix<f64>) -> Result<(DMatrix<f64>, f64), String> {
    rr(y)?;
    let (t, n) = y.shape();
    let act: Vec<usize> = (0..n).filter(|&i| y.column(i).max() > y.column(i).min()).collect();
    if act.len() < n {
        let mut out = DMatrix::zeros(n, n);
        if act.is_empty() {
            return Ok((out, 0.0));
        }
        let (sub, dl) = shrink(&y.select_columns(&act))?;
        for (a, &i) in act.iter().enumerate() {
            for (b, &j) in act.iter().enumerate() {
                out[(i, j)] = sub[(a, b)];
            }
        }
        return Ok((out, dl));
    }
    let (mu, s) = mom(y);
    if n == 1 {
        return Ok((s, 0.0));
    }
    let x = centre(y, &mu);
    let tf = t as f64;
    let sd: Vec<f64> = (0..n).map(|i| s[(i, i)].sqrt()).collect();
    let rb = ((0..n).flat_map(|i| (0..n).map(move |j| (i, j))).map(|(i, j)| s[(i, j)] / (sd[i] * sd[j])).sum::<f64>()
        - n as f64)
        / (n * (n - 1)) as f64;
    let f = DMatrix::from_fn(n, n, |i, j| if i == j { s[(i, i)] } else { rb * sd[i] * sd[j] });
    let x2 = x.component_mul(&x);
    let p = (x2.transpose() * &x2) / tf - s.component_mul(&s);
    let pi = p.sum();
    let th = (x2.component_mul(&x).transpose() * &x) / tf - DMatrix::from_fn(n, n, |i, j| s[(i, i)] * s[(i, j)]);
    let mut q = 0.0;
    for i in 0..n {
        for j in 0..n {
            q += sd[j] / sd[i] * th[(i, j)] + sd[i] / sd[j] * th[(j, i)];
        }
    }
    let rho = p.trace() + rb / 2.0 * (q - 2.0 * th.trace());
    let ga = (&f - &s).norm_squared();
    if ga == 0.0 {
        return Ok((s, 0.0));
    }
    let dl = ((pi - rho) / ga / tf).clamp(0.0, 1.0);
    Ok((&f * dl + &s * (1.0 - dl), dl))
}

/// Intercept and slope of y on x by least squares with an intercept.
pub fn ols(y: &DVector<f64>, x: &DVector<f64>) -> Result<(f64, f64), String> {
    if x.len() < 2 || y.len() != x.len() {
        return Err("returns require matching observations and a one-dimensional index".into());
    }
    if x.iter().chain(y.iter()).any(|v| !v.is_finite()) {
        return Err("stock and index returns must be finite".into());
    }
    if x.max() == x.min() {
        return Err("constant index returns: the slope is undefined".into());
    }
    let (xm, ym) = (x.mean(), y.mean());
    let sxy: f64 = x.iter().zip(y.iter()).map(|(a, b)| (a - xm) * (b - ym)).sum();
    let sxx: f64 = x.iter().map(|a| (a - xm) * (a - xm)).sum();
    let b = sxy / sxx;
    Ok((ym - b * xm, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Appendix B by explicit loops over (i, j, t).
    fn shrink_loops(y: &DMatrix<f64>) -> (DMatrix<f64>, f64) {
        let (t, n) = y.shape();
        let tf = t as f64;
        let mu: Vec<f64> = (0..n).map(|i| (0..t).map(|k| y[(k, i)]).sum::<f64>() / tf).collect();
        let x = |k: usize, i: usize| y[(k, i)] - mu[i];
        let s = DMatrix::from_fn(n, n, |i, j| (0..t).map(|k| x(k, i) * x(k, j)).sum::<f64>() / tf);
        let sd = |i: usize| s[(i, i)].sqrt();
        let mut rb = 0.0;
        for i in 0..n {
            for j in i + 1..n {
                rb += s[(i, j)] / (sd(i) * sd(j));
            }
        }
        rb *= 2.0 / (n * (n - 1)) as f64;
        let pij = |i: usize, j: usize| (0..t).map(|k| (x(k, i) * x(k, j) - s[(i, j)]).powi(2)).sum::<f64>() / tf;
        let th = |i: usize, j: usize| {
            (0..t).map(|k| (x(k, i).powi(2) - s[(i, i)]) * (x(k, i) * x(k, j) - s[(i, j)])).sum::<f64>() / tf
        };
        let (mut pi, mut rho, mut ga) = (0.0, 0.0, 0.0);
        for i in 0..n {
            for j in 0..n {
                pi += pij(i, j);
                let fij = if i == j { s[(i, i)] } else { rb * sd(i) * sd(j) };
                ga += (fij - s[(i, j)]).powi(2);
                if i == j {
                    rho += pij(i, i);
                } else {
                    rho += rb / 2.0 * (sd(j) / sd(i) * th(i, j) + sd(i) / sd(j) * th(j, i));
                }
            }
        }
        let dl = ((pi - rho) / ga / tf).clamp(0.0, 1.0);
        let f = DMatrix::from_fn(n, n, |i, j| if i == j { s[(i, i)] } else { rb * sd(i) * sd(j) });
        (&f * dl + &s * (1.0 - dl), dl)
    }

    #[test]
    fn shrinkage_against_appendix_loops() {
        let y = DMatrix::from_fn(24, 5, |t, i| ((t * 7 + i * 13) as f64 * 0.37).sin() * 0.05 + 0.001 * i as f64);
        let (a, da) = shrink(&y).unwrap();
        let (b, db) = shrink_loops(&y);
        assert!((da - db).abs() < 1e-12 && da > 0.0 && da < 1.0);
        assert!((a - b).amax() < 1e-15);
    }

    #[test]
    fn shrink_handles_one_asset_and_constant_series() {
        let y = DMatrix::from_column_slice(4, 1, &[0.1, -0.2, 0.3, 0.0]);
        let (s, dl) = shrink(&y).unwrap();
        assert_eq!(dl, 0.0);
        assert!((s[(0, 0)] - mom(&y).1[(0, 0)]).abs() == 0.0);
        let y = DMatrix::from_row_slice(3, 2, &[0.1, 0.0, -0.2, 0.0, 0.3, 0.0]);
        let (s, _) = shrink(&y).unwrap();
        assert_eq!(s[(1, 1)], 0.0);
        assert_eq!(s[(0, 1)], 0.0);
    }

    #[test]
    fn regression_inputs_are_checked() {
        let x = DVector::from_vec(vec![0.01, 0.02, -0.01, 0.03]);
        let y = &x * 0.8 + DVector::repeat(4, 0.002);
        let (a, b) = ols(&y, &x).unwrap();
        assert!((a - 0.002).abs() < 1e-15 && (b - 0.8).abs() < 1e-14);
        assert!(ols(&y, &DVector::repeat(4, 0.01)).is_err());
        assert!(ols(&DVector::from_vec(vec![f64::NAN; 4]), &x).is_err());
        assert!(ols(&y.rows(0, 3).into_owned(), &x).is_err());
    }

    #[test]
    fn indefinite_or_asymmetric_covariance_is_refused() {
        let mu = [0.01, 0.02];
        assert!(ms(&mu, &DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 2.0, 1.0])).is_err());
        assert!(ms(&mu, &DMatrix::from_row_slice(2, 2, &[1.0, 0.1, 0.0, 1.0])).is_err());
        assert!(ms(&mu, &DMatrix::from_row_slice(2, 2, &[1.0, 1.0, 1.0, 1.0])).is_ok());
        assert_eq!(scale(&DMatrix::zeros(2, 2)), 1.0);
    }
}
