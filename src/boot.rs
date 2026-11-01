//! RQ1: Bessembinder's (2018) bootstrap of random K-stock portfolios on CRSP (gate G3).
//!
//! Protocol and the G3 tolerance are fixed in docs/PLAN.md: July 1926 to December 2016, share
//! codes 10-12, CRSP delisting returns as supplied (no Shumway fill, as in the source), monthly
//! redraw, value weights by beginning-of-month market value. One bundle per size K; no row-level
//! data is written.

use crate::{bd, bs, data};
use nalgebra::DMatrix;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

// Legend
// rp : bootstrap portfolio returns (n, Tm); i : beginning-of-month market value (Tm, N)
// n : replications; K : stocks per month; L : months linked for a finite-horizon outcome
// prev : the valuation month before the span

pub const SPAN: (i64, i64) = (192607, 201612);
pub const HZ: [(&str, usize); 2] = [("1y", 12), ("10y", 120)];

pub fn out() -> PathBuf {
    data::root().join("docs").join("results").join("boot")
}

/// Linked return per replication over L months from a start drawn uniformly per replication
/// (docs/PLAN.md: the source does not state its windows), with the T-bill and market over the
/// same months.
pub fn outcomes(
    rp: &DMatrix<f64>,
    rf: &[f64],
    vw: &[f64],
    l: usize,
    g: &mut impl Rng,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>), String> {
    let (n, tm) = rp.shape();
    if l < 1 || l > tm || rf.len() != tm || vw.len() != tm {
        return Err("bootstrap horizon or benchmark shape is invalid".into());
    }
    let link = |a: &mut dyn Iterator<Item = f64>| a.fold(1.0, |p, r| p * (1.0 + r)) - 1.0;
    let (mut x, mut f, mut m) = (vec![], vec![], vec![]);
    for q in 0..n {
        let s = g.random_range(0..=tm - l);
        x.push(link(&mut (s..s + l).map(|t| rp[(q, t)])));
        f.push(link(&mut rf[s..s + l].iter().copied()));
        m.push(link(&mut vw[s..s + l].iter().copied()));
    }
    Ok((x, f, m))
}

/// The boot command.
pub fn main(
    crsp: &Path,
    rf: &Path,
    n: usize,
    sizes: &[usize],
    seed: u64,
    missing: bool,
    out: &Path,
    code: &Value,
    cmd: &[String],
) -> Result<(), String> {
    if n < 1 || sizes.iter().any(|&k| k < 1) || sizes.is_empty() {
        return Err("positive draw sizes and replications are required".into());
    }
    let rows = data::crsp(crsp, &[10, 11, 12])?;
    let rfd = data::rf(rf)?;
    let (y, m) = (SPAN.0 / 100, SPAN.0 % 100);
    let prev = if m == 1 { (y - 1) * 100 + 12 } else { y * 100 + m - 1 };
    let pv: Vec<usize> = (0..rows.mo.len()).filter(|&k| rows.mo[k] == prev).collect();
    if pv.is_empty() {
        return Err(format!("CRSP extract needs the preceding month's valuation rows ({prev})"));
    }
    let k: Vec<usize> = (0..rows.mo.len()).filter(|&j| (SPAN.0..=SPAN.1).contains(&rows.mo[j])).collect();
    let mut ms: Vec<i64> = k.iter().map(|&j| rows.mo[j]).collect();
    ms.dedup();
    if ms != data::mseq(SPAN.0, SPAN.1) {
        return Err("CRSP extract must cover the complete registered replication span".into());
    }
    let (p0, _) = crate::panel::panel(&rows.pick(&k), &rfd, false)?;
    let first: std::collections::HashMap<i64, f64> =
        pv.iter().map(|&j| (rows.permno[j], rows.prc[j].abs() * rows.shrout[j])).collect();
    let (tm, nn) = p0.r.shape();
    let i =
        DMatrix::from_fn(
            tm,
            nn,
            |t, c| {
                if t == 0 { *first.get(&p0.ids[c]).unwrap_or(&f64::NAN) } else { p0.me[(t - 1, c)] }
            },
        );
    if !(0..nn).any(|c| i[(0, c)].is_finite() && i[(0, c)] > 0.0 && p0.r[(0, c)].is_finite()) {
        return Err("CRSP extract needs usable preceding-month valuations".into());
    }
    let vw = bs::vw(&p0.r, &i)?;
    let link = |r: &[f64]| r.iter().fold(1.0, |p, x| p * (1.0 + x)) - 1.0;
    let ip = json!({"crsp": rows.sha, "rf": rfd.sha, "rf_vintage": rfd.vintage, "plan": data::sha256(&data::root().join("docs").join("PLAN.md"))?});
    for &kk in sizes {
        let key = format!("boot-K{kk}");
        let pa = json!({"K": kk, "n": n, "seed": seed, "span": [SPAN.0, SPAN.1], "valuation": prev});
        if missing && bd::valid(out, &key, &pa, code["source"].as_str().unwrap_or(""), &ip).is_some() {
            continue;
        }
        let mut g = StdRng::seed_from_u64(seed.wrapping_mul(1_000_003).wrapping_add(kk as u64));
        let rp = bs::draw(&p0.r, &i, kk, n, &mut g)?;
        let life: Vec<f64> = rp.row_iter().map(|q| link(&q.iter().copied().collect::<Vec<_>>())).collect();
        let mut r = json!({"life": bs::stats(&life, &vec![link(&p0.rf); n], &vec![link(&vw); n])?});
        for (h, l) in HZ {
            let (x, f, m) = outcomes(&rp, &p0.rf, &vw, l, &mut g)?;
            r[h] = bs::stats(&x, &f, &m)?;
        }
        println!("{key} {}", r["life"]);
        bd::write(out, &bd::bundle(&key, &pa, r, &ip, cmd, code))?;
    }
    Ok(())
}
