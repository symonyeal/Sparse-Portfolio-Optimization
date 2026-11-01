//! Evaluation: underperformance probabilities, hit rate, terminal-wealth metrics, inference.
//!
//! Primary metric (specification sec. 8; docs/PLAN.md): the share of start dates at which a
//! strategy's buy-and-hold wealth over H falls below B1's, and below B5's. Inference: the
//! stationary bootstrap (Politis and Romano, 1994) for differences over overlapping windows, and
//! the HAC test of Ledoit and Wolf (2008, sec. 3.1) for a difference of Sharpe ratios.
//!
//! The set T of top wealth creators uses data after every selection date; it enters here and
//! nowhere in selection (a test asserts that the selection modules never name it).

use crate::cv::cvar;
use rand::Rng;

// Legend
// ws, wb : terminal wealth of a strategy and of a benchmark per start date
// hs     : held PERMCOs per start date; tt : PERMCOs of the top wealth creators
// p      : stationary-bootstrap restart probability (mean block length 1/p)
// b      : bootstrap replications; al : two-sided level
// ri, rn : excess returns of two strategies over the same months
// v      : (mu_i, mu_n, gamma_i, gamma_n), first and uncentred second moments
// u      : scalar influence y gradient; bw : kernel bandwidth S_T; d : Sharpe difference
// su, sd : roundoff scales of the influence and of the Sharpe difference

fn vector(x: &[f64]) -> Result<(), String> {
    if x.is_empty() || x.iter().any(|v| !v.is_finite()) {
        return Err("observations must be a nonempty finite vector".into());
    }
    Ok(())
}

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

/// Standard deviation with divisor n - ddof.
fn std(x: &[f64], ddof: usize) -> f64 {
    let m = mean(x);
    (x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (x.len() - ddof) as f64).sqrt()
}

/// Share of start dates with strategy wealth below the benchmark's.
pub fn under(ws: &[f64], wb: &[f64]) -> Result<f64, String> {
    vector(ws)?;
    vector(wb)?;
    if ws.len() != wb.len() {
        return Err("wealth observations must be paired".into());
    }
    Ok(ws.iter().zip(wb).filter(|(a, b)| a < b).count() as f64 / ws.len() as f64)
}

/// Share of portfolios holding at least one firm of T.
pub fn hit(hs: &[Vec<i64>], tt: &[i64]) -> Result<f64, String> {
    if hs.is_empty() {
        return Err("hit rate needs at least one portfolio".into());
    }
    Ok(hs.iter().filter(|h| h.iter().any(|p| tt.contains(p))).count() as f64 / hs.len() as f64)
}

/// Standardised third moment.
pub fn skew(x: &[f64]) -> Result<f64, String> {
    vector(x)?;
    let (m, s) = (mean(x), std(x, 0));
    if s == 0.0 {
        return Err("skewness is undefined for constant observations".into());
    }
    Ok(x.iter().map(|v| ((v - m) / s).powi(3)).sum::<f64>() / x.len() as f64)
}

/// Mean over standard deviation (ddof 1) of excess returns.
pub fn sharpe(r: &[f64]) -> Result<f64, String> {
    vector(r)?;
    if r.len() < 2 || std(r, 1) == 0.0 {
        return Err("Sharpe ratio needs at least two nonconstant returns".into());
    }
    Ok(mean(r) / std(r, 1))
}

/// beta-CVaR of the terminal-wealth loss 1 - W, equal weights.
pub fn cvw(w: &[f64], b: f64) -> Result<f64, String> {
    vector(w)?;
    if !(0.0 < b && b < 1.0) {
        return Err("CVaR beta must be between zero and one".into());
    }
    Ok(cvar(&w.iter().map(|x| 1.0 - x).collect::<Vec<_>>(), b))
}

/// Indices of one stationary-bootstrap resample of 0..n-1 (Politis and Romano, 1994).
pub fn sbi(n: usize, p: f64, g: &mut impl Rng) -> Result<Vec<usize>, String> {
    if n < 1 || !(0.0 < p && p <= 1.0) {
        return Err("bootstrap needs a positive size and restart probability in (0, 1]".into());
    }
    let mut i = vec![g.random_range(0..n)];
    for t in 1..n {
        i.push(if g.random::<f64>() < p { g.random_range(0..n) } else { (i[t - 1] + 1) % n });
    }
    Ok(i)
}

/// Mean of x and its stationary-bootstrap percentile interval (numpy's linear quantile).
pub fn sbci(x: &[f64], p: f64, b: usize, g: &mut impl Rng, al: f64) -> Result<(f64, (f64, f64)), String> {
    vector(x)?;
    if b < 1 || !(0.0 < al && al < 1.0) {
        return Err("bootstrap replications and interval level are invalid".into());
    }
    let mut m = (0..b)
        .map(|_| sbi(x.len(), p, g).map(|i| i.iter().map(|&k| x[k]).sum::<f64>() / x.len() as f64))
        .collect::<Result<Vec<_>, _>>()?;
    m.sort_by(f64::total_cmp);
    let q = |a: f64| {
        let h = a * (m.len() - 1) as f64;
        let (k, f) = (h.floor() as usize, h - h.floor());
        if k + 1 < m.len() { m[k] + f * (m[k + 1] - m[k]) } else { m[k] }
    };
    Ok((mean(x), (q(al / 2.0), q(1.0 - al / 2.0))))
}

/// Parzen kernel.
pub fn parzen(x: f64) -> f64 {
    let x = x.abs();
    if x <= 0.5 {
        1.0 - 6.0 * x * x + 6.0 * x.powi(3)
    } else if x <= 1.0 {
        2.0 * (1.0 - x).powi(3)
    } else {
        0.0
    }
}

/// Sharpe difference with its HAC standard error, two-sided p-value and 95% interval.
pub struct Lw {
    pub d: f64,
    pub se: f64,
    pub p: f64,
    pub ci: [f64; 2],
}

/// Ledoit and Wolf (2008), eqs. (1)-(5), Parzen kernel. bw defaults to 4 (T/100)^(2/9), a
/// project setting; the paper leaves the bandwidth to Andrews (1991) and prefers its
/// studentized bootstrap.
pub fn lw(ri: &[f64], rn: &[f64], bw: Option<f64>) -> Result<Lw, String> {
    vector(ri)?;
    vector(rn)?;
    if ri.len() != rn.len() || ri.len() <= 4 || std(ri, 0) == 0.0 || std(rn, 0) == 0.0 {
        return Err("Sharpe inference needs paired nonconstant returns and more than 4 months".into());
    }
    let t = ri.len();
    let tf = t as f64;
    let sq = |x: &[f64]| x.iter().map(|v| v * v).sum::<f64>() / tf;
    let v = [mean(ri), mean(rn), sq(ri), sq(rn)];
    let y: Vec<[f64; 4]> =
        (0..t).map(|k| [ri[k] - v[0], rn[k] - v[1], ri[k] * ri[k] - v[2], rn[k] * rn[k] - v[3]]).collect();
    let bw = bw.unwrap_or(4.0 * (tf / 100.0).powf(2.0 / 9.0));
    if !bw.is_finite() || bw <= 0.0 {
        return Err("HAC bandwidth must be finite and positive".into());
    }
    let [a, b, c, d] = v;
    let gr = [
        c / (c - a * a).powf(1.5),
        -d / (d - b * b).powf(1.5),
        -0.5 * a / (c - a * a).powf(1.5),
        0.5 * b / (d - b * b).powf(1.5),
    ];
    let u: Vec<f64> = y.iter().map(|q| (0..4).map(|k| q[k] * gr[k]).sum()).collect();
    let dd = a / (c - a * a).sqrt() - b / (d - b * b).sqrt();
    let eps = f64::EPSILON;
    let su: Vec<f64> = y.iter().map(|q| (0..4).map(|k| q[k].abs() * gr[k].abs()).sum()).collect();
    let sd = (a / std(ri, 0)).abs() + (b / std(rn, 0)).abs();
    // Both the difference and its influence must vanish at floating-point precision.
    if u.iter().zip(&su).all(|(x, s)| x.abs() <= 16.0 * eps * s) && dd.abs() <= 16.0 * eps * sd {
        return Ok(Lw { d: 0.0, se: 0.0, p: 1.0, ci: [0.0, 0.0] });
    }
    let (mut hac, mut ab, mut nq) = (0.0, 0.0, 0usize);
    for j in 0..t {
        let mut k = parzen(j as f64 / bw);
        if k == 0.0 {
            continue;
        }
        k *= if j == 0 { 1.0 } else { 2.0 };
        hac += k * (j..t).map(|s| u[s] * u[s - j]).sum::<f64>() / tf;
        ab += k * (j..t).map(|s| (u[s] * u[s - j]).abs()).sum::<f64>() / tf;
        nq += 1;
    }
    if hac < -eps * (t + nq + 1) as f64 * ab {
        return Err("HAC variance is negative".into());
    }
    let hac = hac.max(0.0) * tf / (tf - 4.0);
    let se = (hac / tf).sqrt();
    let z = if se > 0.0 {
        dd.abs() / se
    } else if dd == 0.0 {
        0.0
    } else {
        f64::INFINITY
    };
    let q = 1.959963984540054;
    Ok(Lw { d: dd, se, p: libm::erfc(z / std::f64::consts::SQRT_2), ci: [dd - q * se, dd + q * se] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn underperformance_and_hit_rate() {
        assert_eq!(under(&[1.0, 2.0, 3.0, 1.0], &[2.0, 2.0, 1.0, 1.5]).unwrap(), 0.5);
        assert!(under(&[1.0], &[1.0, 2.0]).is_err());
        assert_eq!(hit(&[vec![1, 2], vec![3], vec![]], &[2, 9]).unwrap(), 1.0 / 3.0);
        assert!((cvw(&[1.0, 0.5, 2.0, 0.0], 0.5).unwrap() - 0.75).abs() < 1e-15);
    }

    #[test]
    fn stationary_bootstrap_and_sharpe_test() {
        let mut g = rand::rngs::StdRng::seed_from_u64(1);
        let i = sbi(50, 0.1, &mut g).unwrap();
        assert!(i.len() == 50 && i.iter().all(|&k| k < 50));
        assert_eq!(sbi(5, 1e-300, &mut g).unwrap().windows(2).filter(|w| w[1] != (w[0] + 1) % 5).count(), 0);
        let x: Vec<f64> = (0..40).map(|k| (k as f64 * 0.7).sin()).collect();
        let (m, (lo, hi)) = sbci(&x, 0.2, 400, &mut g, 0.05).unwrap();
        assert!(lo < m && m < hi);
        let a: Vec<f64> = (0..120).map(|k| 0.01 + 0.05 * (k as f64 * 1.3).sin()).collect();
        let b: Vec<f64> = (0..120).map(|k| 0.005 + 0.04 * (k as f64 * 0.9).cos()).collect();
        let s = lw(&a, &b, None).unwrap();
        assert!((s.d - (sharpe(&a).unwrap() - sharpe(&b).unwrap()) * (120.0f64 / 119.0).sqrt()).abs() < 1e-12);
        assert!(s.se > 0.0 && 0.0 < s.p && s.p < 1.0 && s.ci[0] < s.d && s.d < s.ci[1]);
    }

    #[test]
    fn same_or_rescaled_strategy_has_zero_sharpe_difference() {
        let r = [0.1, 0.2, -0.1, 0.05, 0.15, -0.15, 0.07, 0.01];
        for c in [1.0, 1.01, 10.0] {
            let s = lw(&r, &r.map(|x| c * x), None).unwrap();
            assert_eq!((s.d, s.se, s.p, s.ci), (0.0, 0.0, 1.0, [0.0, 0.0]), "{c}");
        }
        let mut b = r.map(|x| 1.01 * x);
        b[0] += 1e-9;
        let s = lw(&r, &b, None).unwrap();
        assert!(s.d != 0.0 && s.se > 0.0);
        assert!(lw(&[1.0; 10], &(0..10).map(f64::from).collect::<Vec<_>>(), None).is_err());
        assert!(lw(&r[..4], &r[..4], None).is_err());
        assert!(under(&[1.0, f64::NAN], &[1.0, 2.0]).is_err());
        assert!(sbi(10, 0.0, &mut rand::rngs::StdRng::seed_from_u64(0)).is_err());
    }
}
