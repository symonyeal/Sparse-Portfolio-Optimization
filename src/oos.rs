//! RQ2-RQ4: out-of-sample selection on CRSP under the pre-registered plan (docs/PLAN.md).
//!
//! For each rebalance date the selection sees `P::upto(t)` only; the holding is evaluated on the
//! full panel. One bundle per (strategy, K, M, m, Nc, H, fill) holds terminal wealths and held
//! PERMCOs per date, never row-level data. The set T of top wealth creators is computed
//! afterwards (`wc`) and used only in `ev`.

use crate::crt::Sol;
use crate::panel::{Choice, P, b2, b3, b4, run};
use crate::x::{Sn, Xk};
use crate::{bd, cv, data, est, mad, mv, sk};
use nalgebra::DMatrix;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

// Legend
// pt : panel cut at t; U : eligible columns; s : strategy
// K, M, m, nc, H : names, window, required returns, universe size, holding months
// tl : per-solve time limit; y : common observed scenarios of the window

/// F1's risk-return weight; F5's variance aversion and skewness preference (docs/PLAN.md).
pub const LAM: f64 = 0.5;
pub const GAM: f64 = 3.0;
pub const KAP: f64 = 12.0;
pub const ST: [&str; 7] = ["b2", "b3", "b4", "f1", "f2", "f3", "f5"];

pub fn out() -> PathBuf {
    data::root().join("docs").join("results").join("oos")
}

/// Settings of one run.
#[derive(Clone, Debug)]
pub struct Run {
    pub k: usize,
    pub mw: usize,
    pub m: usize,
    pub nc: usize,
    pub h: usize,
    pub fill: bool,
    pub tl: f64,
    pub seed: u64,
}

/// Common observed scenarios within the last M months of columns U, with at least m rows.
pub fn window(pt: &P, u: &[usize], mw: usize, m: usize) -> Result<DMatrix<f64>, String> {
    if !(1..=mw).contains(&m) {
        return Err("history threshold must be between 1 and M".into());
    }
    let t = pt.r.nrows();
    let rows: Vec<usize> = (t.saturating_sub(mw)..t).filter(|&s| u.iter().all(|&i| pt.r[(s, i)].is_finite())).collect();
    if u.is_empty() || rows.len() < m {
        return Err("not enough common observed scenarios for the history threshold".into());
    }
    Ok(DMatrix::from_fn(rows.len(), u.len(), |a, b| pt.r[(rows[a], u[b])]))
}

/// Selection on a cut panel by strategy s.
pub fn select(s: &str, pt: &P, u: &[usize], c: &Run, g: &mut StdRng) -> Result<Choice, String> {
    let plain = |(cols, w): (Vec<usize>, Vec<f64>)| Choice { cols, w: Some(w), c: None, meta: Map::new() };
    if s == "b2" {
        return Ok(plain(b2(u)));
    }
    if u.len() < c.k {
        return Err("eligible universe has fewer than K names".into());
    }
    match s {
        "b3" => return Ok(plain(b3(pt, u, c.k))),
        "b4" => return Ok(plain(b4(u, c.k, g))),
        _ => {}
    }
    let y = window(pt, u, c.mw, c.m)?;
    let mut mu: Vec<f64> = y.column_iter().map(|x| x.mean()).collect();
    let x = Xk::new(u.len(), c.k, 0.0, 1.0, Sn::Le)?;
    let tl = Some(c.tl);
    let mut meta = Map::new();
    meta.insert("scenarios".into(), json!(y.nrows()));
    let q: Sol = match s {
        "f1" | "f5" => {
            let (sh, dl) = est::shrink(&y)?;
            meta.insert("shrink".into(), json!(dl));
            meta.insert("rho".into(), Value::Null);
            if s == "f1" {
                mv::f1(&mu, &sh, x, LAM, None, false, tl, 0.0)?
            } else {
                sk::f5(&y, x, GAM, KAP, Some(&sh), tl, 0.0)?
            }
        }
        "f2" | "f3" => {
            mu.sort_by(f64::total_cmp);
            let n = mu.len();
            let rho = if n % 2 == 1 { mu[n / 2] } else { 0.5 * (mu[n / 2 - 1] + mu[n / 2]) };
            meta.insert("rho".into(), json!(rho));
            if s == "f2" { mad::mad(&y, x, Some(rho), tl, 0.0)? } else { cv::cv(&y, x, 0.95, Some(rho), tl, 0.0)? }
        }
        _ => return Err(format!("unknown strategy {s:?}")),
    };
    let w = q.w.map(|w| {
        let w: Vec<f64> = w.iter().map(|v| v.max(0.0)).collect();
        let t: f64 = w.iter().sum();
        w.iter().map(|v| v / t).collect()
    });
    Ok(Choice { cols: u.to_vec(), w, c: Some(q.c), meta })
}

/// Run strategy s on the panel and return its bundle record.
pub fn record(s: &str, p0: &P, c: &Run, fills: usize) -> Result<Value, String> {
    let mut g = StdRng::seed_from_u64(c.seed);
    let tm = p0.r.nrows();
    let dates = (c.mw - 1)..tm.saturating_sub(c.h);
    let o = run(p0, |pt, u| select(s, pt, u, c, &mut g), dates, c.mw, c.m, c.nc, c.h)?;
    let (mut b1, mut err) = (vec![], vec![]);
    for q in &o {
        match p0.b1(q.t, c.h) {
            Ok(w) => {
                b1.push(json!(w[w.len() - 1]));
                err.push(Value::Null);
            }
            Err(e) => {
                b1.push(Value::Null);
                err.push(json!(e));
            }
        }
    }
    let b5: Vec<f64> = o.iter().map(|q| p0.b5(q.t, c.h).map(|w| w[w.len() - 1])).collect::<Result<_, _>>()?;
    let sel: Vec<Value> = o
        .iter()
        .map(|q| {
            let mut m = q.meta.clone();
            m.insert("status".into(), json!(q.status));
            m.insert("eligible".into(), json!(q.eligible));
            if let Some(c) = &q.c {
                m.insert("c".into(), c.json());
            }
            if let Some(r) = &q.reason {
                m.insert("reason".into(), json!(r));
            }
            Value::Object(m)
        })
        .collect();
    Ok(json!({
        "mo": o.iter().map(|q| q.mo).collect::<Vec<_>>(),
        "W": o.iter().map(|q| q.wp.last().copied()).collect::<Vec<_>>(),
        "pc": o.iter().map(|q| q.pc.iter().zip(&q.w).filter(|(_, w)| **w > 0.0).map(|(p, _)| *p).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "selection": sel, "B1": b1, "B1_errors": err, "B5": b5, "fills": fills,
    }))
}

/// Key and parameters of a run's bundle.
pub fn key(s: &str, c: &Run) -> (String, Value) {
    let k =
        format!("oos-{s}-K{}-M{}-m{}-Nc{}-H{}-{}", c.k, c.mw, c.m, c.nc, c.h, if c.fill { "fill" } else { "nofill" });
    let mut pa = json!({"s": s, "K": c.k, "M": c.mw, "m": c.m, "Nc": c.nc, "H": c.h, "fill": c.fill, "tl": c.tl, "seed": c.seed});
    if s == "f5" {
        pa["g"] = json!(GAM);
        pa["k"] = json!(KAP);
    }
    (k, pa)
}

/// The oos command: every strategy in only (all if empty), one bundle each.
pub fn main(
    crsp: &Path,
    rf: &Path,
    only: &[String],
    c: &Run,
    missing: bool,
    out: &Path,
    code: &Value,
    cmd: &[String],
) -> Result<(), String> {
    if c.k.min(c.mw).min(c.m).min(c.nc).min(c.h) < 1 || c.m > c.mw || c.k > c.nc || !c.tl.is_finite() || c.tl <= 0.0 {
        return Err("positive sizes and time, m <= M and K <= Nc are required".into());
    }
    if let Some(s) = only.iter().find(|s| !ST.contains(&s.as_str())) {
        return Err(format!("unknown strategy {s:?}"));
    }
    let rows = data::crsp(crsp, &[10, 11, 12])?;
    let rfd = data::rf(rf)?;
    let (p0, nf) = crate::panel::panel(&rows, &rfd, c.fill)?;
    let ip = json!({"crsp": rows.sha, "rf": rfd.sha, "rf_vintage": rfd.vintage, "plan": data::sha256(&data::root().join("docs").join("PLAN.md"))?});
    let all: Vec<String> = ST.iter().map(|s| s.to_string()).collect();
    for s in if only.is_empty() { &all } else { only } {
        let (k, pa) = key(s, c);
        if missing && bd::valid(out, &k, &pa, code["source"].as_str().unwrap_or(""), &ip).is_some() {
            continue;
        }
        let r = record(s, &p0, c, nf)?;
        let n = r["mo"].as_array().map_or(0, |a| a.len());
        bd::write(out, &bd::bundle(&k, &pa, r, &ip, cmd, code))?;
        println!("{k} {n} dates");
    }
    Ok(())
}
