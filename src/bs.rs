//! Bessembinder's (2018) bootstrap of random K-stock portfolios (sec. 4, Table 4).
//!
//! For each month, K stocks are drawn at random from those with a return that month; the month's
//! portfolio return is value-weighted by beginning-of-month market value; months are linked. The
//! procedure is repeated n times (20,000 in the source) and each linked return is compared with
//! zero, the T-bill and the value-weighted market over the same months.
//!
//! This redraws the names every month. The specification's B4 holds K names bought once, equally
//! weighted (`panel::b4`); that is an extension, not the source's protocol.

use nalgebra::DMatrix;
use rand::Rng;
use serde_json::{Value, json};

// Legend
// r  : returns (Tm, N), NaN where a stock has no return
// i  : beginning-of-month market value (Tm, N), NaN where unknown
// K  : stocks drawn per month; n : replications; rp : portfolio returns (n, Tm)
// x, f, m : linked returns of the replications, the T-bill and the market over the same months

fn arrays(r: &DMatrix<f64>, i: &DMatrix<f64>) -> Result<(), String> {
    if r.shape() != i.shape() || r.is_empty() {
        return Err("return and value matrices disagree in shape".into());
    }
    if r.iter().chain(i.iter()).any(|x| x.is_infinite()) || r.iter().any(|x| *x < -1.0) {
        return Err("return or market value is invalid".into());
    }
    Ok(())
}

/// Monthly value-weighted returns of K stocks drawn afresh each month, n replications.
pub fn draw(r: &DMatrix<f64>, i: &DMatrix<f64>, k: usize, n: usize, g: &mut impl Rng) -> Result<DMatrix<f64>, String> {
    arrays(r, i)?;
    if k < 1 || n < 1 {
        return Err("draw sizes must be positive integers".into());
    }
    let tm = r.nrows();
    let mut rp = DMatrix::zeros(n, tm);
    for t in 0..tm {
        let a: Vec<usize> =
            (0..r.ncols()).filter(|&j| r[(t, j)].is_finite() && i[(t, j)].is_finite() && i[(t, j)] > 0.0).collect();
        if a.len() < k {
            return Err(format!("month {t}: {} stocks available, {k} drawn", a.len()));
        }
        for q in 0..n {
            let c = rand::seq::index::sample(g, a.len(), k);
            let (num, den) = c.iter().fold((0.0, 0.0), |(u, v), x| (u + i[(t, a[x])] * r[(t, a[x])], v + i[(t, a[x])]));
            rp[(q, t)] = num / den;
        }
    }
    Ok(rp)
}

/// Value-weighted market return each month over stocks with a return and a value.
pub fn vw(r: &DMatrix<f64>, i: &DMatrix<f64>) -> Result<Vec<f64>, String> {
    arrays(r, i)?;
    (0..r.nrows())
        .map(|t| {
            let (num, den) = (0..r.ncols())
                .filter(|&j| r[(t, j)].is_finite() && i[(t, j)].is_finite() && i[(t, j)] > 0.0)
                .fold((0.0, 0.0), |(u, v), j| (u + i[(t, j)] * r[(t, j)], v + i[(t, j)]));
            if den <= 0.0 {
                return Err("market return unavailable in a month without eligible value".into());
            }
            Ok(num / den)
        })
        .collect()
}

/// Linked-return statistics against paired T-bill and market outcomes.
pub fn stats(x: &[f64], f: &[f64], m: &[f64]) -> Result<Value, String> {
    if x.is_empty() || x.len() != f.len() || f.len() != m.len() || x.iter().chain(f).chain(m).any(|v| !v.is_finite()) {
        return Err("bootstrap outcomes must be nonempty finite paired vectors".into());
    }
    let n = x.len() as f64;
    let mean = x.iter().sum::<f64>() / n;
    let sd = (x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n).sqrt();
    let sk = (sd > 0.0).then(|| x.iter().map(|v| ((v - mean) / sd).powi(3)).sum::<f64>() / n);
    let mut s = x.to_vec();
    s.sort_by(f64::total_cmp);
    let k = s.len();
    let med = if k % 2 == 1 { s[k / 2] } else { 0.5 * (s[k / 2 - 1] + s[k / 2]) };
    let share = |p: &dyn Fn(usize) -> bool| (0..k).filter(|&j| p(j)).count() as f64 / n;
    Ok(json!({
        "mean": mean, "med": med, "skew": sk,
        "gt0": share(&|j| x[j] > 0.0), "gtf": share(&|j| x[j] > f[j]), "gtvw": share(&|j| x[j] > m[j]),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn draw_of_all_stocks_is_the_market() {
        let n = f64::NAN;
        let r = DMatrix::from_row_slice(2, 3, &[0.1, -0.05, n, 0.02, 0.03, 0.04]);
        let i = DMatrix::from_row_slice(2, 3, &[2.0, 1.0, 5.0, 1.0, 1.0, 2.0]);
        let mut g = rand::rngs::StdRng::seed_from_u64(0);
        assert!(draw(&r, &i, 3, 4, &mut g).is_err());
        let rp = draw(&r.columns(0, 2).into_owned(), &i.columns(0, 2).into_owned(), 2, 4, &mut g).unwrap();
        let m = vw(&r, &i).unwrap();
        assert!((m[0] - 0.05).abs() < 1e-15);
        assert!(rp.row_iter().all(|q| (q[0] - m[0]).abs() < 1e-15 && (q[1] - 0.025).abs() < 1e-15));
    }

    #[test]
    fn bad_returns_and_shapes_are_rejected() {
        assert!(vw(&DMatrix::from_element(1, 1, -1.5), &DMatrix::from_element(1, 1, 1.0)).is_err());
        assert!(vw(&DMatrix::from_element(1, 2, 0.0), &DMatrix::from_element(1, 1, 1.0)).is_err());
        assert!(vw(&DMatrix::from_element(1, 1, 0.1), &DMatrix::from_element(1, 1, f64::NAN)).is_err());
        assert!(stats(&[0.1], &[0.0, 0.0], &[0.0]).is_err());
        let s = stats(&[0.1, -0.1, 0.3], &[0.0; 3], &[0.2; 3]).unwrap();
        assert_eq!((s["med"].as_f64(), s["gtvw"].as_f64()), (Some(0.1), Some(1.0 / 3.0)));
    }
}
