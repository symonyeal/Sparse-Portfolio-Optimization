//! Public benchmark cases on OR-Library data: G1, the Chang comparison, RQ5 and the F2-F5 checks.
//!
//! Each case writes docs/results/bench/<key>.json as it finishes, so a stopped run loses only the
//! cases in flight; `table` regenerates docs/results/README.md from admitted bundles only.

use crate::bd;
use crate::data::{self, root};
use crate::x::{Sn, Xk, nz};
use crate::{cv, est, it, mad, mv, sk};
use nalgebra::DVector;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Instant;

// Legend
// key, kd, pa : case key, kind and parameters; tl : per-solve time limit (s)
// ip : pinned input hashes; src : current source digest; r : result record

/// Canakgoz and Beasley (2009), Table 1: K for indtrack1-8.
const CB: [usize; 8] = [10, 10, 10, 10, 10, 40, 90, 70];
/// Chang et al. (2000), Table 4, port1, K = 10: median and mean % error of the GA, TS and SA
/// heuristics, measure V.
const CH: [(&str, f64, f64); 3] = [("GA", 1.2181, 1.0974), ("TS", 1.2181, 1.1217), ("SA", 1.2181, 1.0957)];

pub fn out() -> PathBuf {
    root().join("docs").join("results").join("bench")
}

pub fn cases() -> Vec<(String, &'static str, Value)> {
    let mut c: Vec<(String, &str, Value)> = (1..=5).map(|k| (format!("g1-port{k}"), "g1", json!({"k": k}))).collect();
    c.push(("chang-port1".into(), "chang", json!({"k": 1, "K": 10, "e": 0.01})));
    for k in 1..=5 {
        for kk in [5, 10] {
            for f in ["f1", "f1p"] {
                c.push((
                    format!("rq5-port{k}-K{kk}-{f}"),
                    "rq5",
                    json!({"k": k, "K": kk, "e": 0.01, "lam": 0.5, "p": f == "f1p"}),
                ));
            }
        }
    }
    for kk in [5, 10] {
        c.push((format!("f2-indtrack1-K{kk}"), "f2", json!({"k": 1, "K": kk})));
    }
    for kk in [5, 10] {
        c.push((format!("f3-indtrack1-K{kk}"), "f3", json!({"k": 1, "K": kk, "b": 0.95})));
    }
    for k in 1..=8 {
        c.push((format!("f4-indtrack{k}"), "f4", json!({"k": k, "K": CB[k - 1]})));
    }
    c.push(("f5-indtrack1-S36-N6-K2".into(), "f5", json!({"k": 1, "S": 36, "N": 6, "K": 2, "g": 3.0})));
    for kk in [3, 10] {
        c.push((format!("f5-indtrack1-K{kk}"), "f5", json!({"k": 1, "K": kk, "g": 3.0})));
    }
    c
}

/// Solve-time budget of a case in units of the per-solve limit.
pub fn cost(kd: &str) -> f64 {
    match kd {
        "g1" => 0.0,
        "chang" => 50.0,
        "f4" => 2.0,
        _ => 1.0,
    }
}

fn u(pa: &Value, k: &str) -> usize {
    pa[k].as_u64().unwrap() as usize
}

/// Current pinned input hashes of a public case.
pub fn inputs(kd: &str, pa: &Value) -> Result<Value, String> {
    let k = u(pa, "k");
    let mut n = vec![if matches!(kd, "g1" | "chang" | "rq5") { format!("port{k}") } else { format!("indtrack{k}") }];
    if matches!(kd, "g1" | "chang") {
        n.push(format!("portef{k}"));
    }
    let mut ip = json!({});
    for x in n {
        ip[&x] = json!(data::pinned(&x)?);
    }
    Ok(ip)
}

fn with_tl(pa: &Value, tl: f64) -> Value {
    let mut p = pa.clone();
    p["tl"] = json!(tl);
    p
}

/// Admit a case through the same code, parameter and input checks for tables and resume.
pub fn admit(key: &str, kd: &str, pa: &Value, tl: f64, src: &str) -> Option<Value> {
    bd::valid(&out(), key, &with_tl(pa, tl), src, &inputs(kd, pa).ok()?)
}

/// Run one case; returns its result record.
pub fn run(kd: &str, pa: &Value, tl: f64) -> Result<Value, String> {
    let k = u(pa, "k");
    let tl = Some(tl);
    match kd {
        "g1" => {
            let p = data::port(k, None)?;
            let ef = data::portef(k, None)?.ef;
            let (mu, s) = (&p.mu, &p.s);
            let lo = mu.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = mu.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let it = 5 * p.n;
            let mut f0 = vec![1.0; p.n];
            let (mut dv, mut kt, mut sep) = (vec![], vec![], vec![]);
            for &(x, v) in &ef {
                let q = mv::qp(mu, s, x, &f0, 1e-12, it)?;
                f0 = q.w.iter().map(|&w| w + f64::from(u8::from(w > 0.0))).collect();
                dv.push(q.f - v);
                let mut fs = vec![q.f];
                kt.push(q.kt.iter().copied().fold(0.0, f64::max));
                for a in [lo.max(x - 5e-11), hi.min(x + 5e-11)] {
                    let q = mv::qp(mu, s, a, &f0, 1e-12, it)?;
                    fs.push(q.f);
                    kt.push(q.kt.iter().copied().fold(0.0, f64::max));
                }
                let (fmin, fmax) =
                    fs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &f| (a.min(f), b.max(f)));
                sep.push(0f64.max(fmin - v - 5e-11).max(v - 5e-11 - fmax));
            }
            let mx = |x: &[f64]| x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let ms = mx(&sep);
            let kk = mx(&kt);
            Ok(json!({
                "n": dv.len(), "max_abs": mx(&dv.iter().map(|d| d.abs()).collect::<Vec<_>>()),
                "max_pos": mx(&dv), "min_neg": dv.iter().copied().fold(f64::INFINITY, f64::min),
                "half_unit": dv.iter().filter(|d| d.abs() <= 5e-11).count() as f64 / dv.len() as f64,
                "max_rel": mx(&dv.iter().zip(&ef).map(|(d, e)| d.abs() / e.1).collect::<Vec<_>>()),
                "kkt": kk, "rectangles": sep.iter().filter(|&&s| s == 0.0).count(),
                "max_separation": ms, "pass": ms == 0.0 && kk < 1e-12,
            }))
        }
        "chang" => {
            let p = data::port(k, None)?;
            let ef = data::portef(k, None)?.ef;
            let x = Xk::new(p.n, u(pa, "K"), pa["e"].as_f64().unwrap(), 1.0, Sn::Eq)?;
            let mut pts = vec![];
            for e in 1..=50 {
                let l = (e - 1) as f64 / 49.0; // Chang et al. (2000): E = 50
                let s = mv::f1(&p.mu, &p.s, x, l, None, false, tl, 0.0)?;
                let (sd, m) =
                    s.w.as_ref().map_or((f64::NAN, f64::NAN), |w| (mv::quad(w, &p.s).sqrt(), mv::dot(&p.mu, w)));
                pts.push(
                    json!({"lam": l, "st": s.c.st.name(), "g": s.c.g, "y": m, "x": sd, "pd": mv::pdev(&ef, sd, m)}),
                );
            }
            let mut v: Vec<f64> = pts.iter().filter_map(|q| q["pd"].as_f64()).collect();
            v.sort_by(f64::total_cmp);
            let n = v.len();
            let med = (n > 0).then(|| if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) });
            let mean = (n > 0).then(|| v.iter().sum::<f64>() / n as f64);
            Ok(
                json!({"pts": pts, "median": med, "mean": mean, "n": n, "all_opt": pts.iter().all(|q| q["st"] == "opt")}),
            )
        }
        "rq5" => {
            let p = data::port(k, None)?;
            let x = Xk::new(p.n, u(pa, "K"), pa["e"].as_f64().unwrap(), 1.0, Sn::Eq)?;
            let s = mv::f1(&p.mu, &p.s, x, pa["lam"].as_f64().unwrap(), None, pa["p"] == true, tl, 0.0)?;
            Ok(json!({"c": s.c.json(), "f": s.f, "N": p.n}))
        }
        _ => {
            let d = data::indtrack(k, None, k == 7 || k == 8)?;
            let (mut r, mut ri) = d.lr();
            if let (Some(s), Some(n)) = (pa["S"].as_u64(), pa["N"].as_u64()) {
                r = r.view((0, 0), (s as usize, n as usize)).into_owned();
                ri = ri.rows(0, s as usize).into_owned();
            }
            let n = r.ncols();
            let kk = u(pa, "K");
            match kd {
                "f2" | "f3" => {
                    let x = Xk::new(n, kk, 0.0, 1.0, Sn::Le)?;
                    let s =
                        if kd == "f2" { mad::mad(&r, x, None, tl, 0.0)? } else { cv::cv(&r, x, 0.95, None, tl, 0.0)? };
                    let ok = s.w.as_ref().map(|w| x.ok(w, s.z.as_ref().unwrap(), 1e-7).is_empty());
                    Ok(json!({"c": s.c.json(), "recomputed": s.f, "diff": (s.c.f - s.f).abs(), "ok": ok}))
                }
                "f4" => {
                    let tr = 145; // Canakgoz and Beasley (2009), sec. 4.1: in-sample [1, 145]
                    let x = Xk::new(n, kk, 0.01, 1.0, Sn::Eq)?;
                    let (r1, i1) = (r.rows(0, tr).into_owned(), ri.rows(0, tr).into_owned());
                    let (r2, i2) = (r.rows(tr, r.nrows() - tr).into_owned(), ri.rows(tr, ri.len() - tr).into_owned());
                    let s = it::it(&r1, &i1, x, it::EA, tl, 0.0)?;
                    let mut o =
                        json!({"c1": s.c1.json(), "c2": s.c2.as_ref().map(|c| c.json()), "N": d.n, "drop": d.drop});
                    if let (Some(w), Some(a), Some(b)) = (&s.w, s.a, s.b) {
                        let (a2, b2) = est::ols(&(&r2 * DVector::from_column_slice(w)), &i2)?;
                        o["a_in"] = json!(a);
                        o["b_in"] = json!(b);
                        o["a_out"] = json!(a2);
                        o["b_out"] = json!(b2);
                        o["te_in"] = json!(it::te(&r1, &i1, w));
                        o["te_out"] = json!(it::te(&r2, &i2, w));
                        o["names"] = json!(nz(w));
                    }
                    Ok(o)
                }
                "f5" => {
                    let g = pa["g"].as_f64().unwrap();
                    let s = sk::f5(&r, Xk::new(n, kk, 0.0, 1.0, Sn::Le)?, g, sk::kap(g), None, tl, 0.0)?;
                    Ok(json!({"c": s.c.json(), "f": s.f, "N": n, "S": r.nrows()}))
                }
                _ => Err(format!("unknown case kind {kd}")),
            }
        }
    }
}

/// Run a case and write its bundle; returns the wall seconds.
pub fn one(key: &str, kd: &str, pa: &Value, tl: f64, code: &Value, cmd: &[String]) -> Result<f64, String> {
    let t0 = Instant::now();
    let ip = inputs(kd, pa)?;
    let mut r = run(kd, pa, tl)?;
    let w = t0.elapsed().as_secs_f64();
    r["wall"] = json!(w);
    bd::write(&out(), &bd::bundle(key, &with_tl(pa, tl), r, &ip, cmd, code))?;
    Ok(w)
}

/// Python-style scientific notation: mantissa, sign and two-digit exponent.
pub fn sci(v: Option<f64>, p: usize) -> String {
    match v {
        Some(v) if v.is_finite() => {
            let s = format!("{v:.p$e}");
            let (m, e) = s.split_once('e').unwrap();
            let e: i32 = e.parse().unwrap();
            format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
        }
        _ => "n/a".into(),
    }
}

fn fx(v: Option<f64>, p: usize) -> String {
    v.filter(|x| x.is_finite()).map_or("n/a".into(), |v| format!("{v:.p$}"))
}

/// Regenerate docs/results/README.md from admitted bundles.
pub fn table() -> Result<(), String> {
    let src = bd::source(&root());
    let mut b: Vec<(String, Value)> = vec![];
    for (key, kd, pa) in cases() {
        let tl = bd::read(&out().join(format!("{key}.json"))).and_then(|v| v["bundle"]["pa"]["tl"].as_f64());
        if let Some(tl) = tl.filter(|t| t.is_finite() && *t > 0.0)
            && let Some(v) = admit(&key, kd, &pa, tl, &src)
        {
            b.push((key, v));
        }
    }
    let mut l = vec![
        "# Benchmark results".to_string(),
        String::new(),
        "Generated by `pf bench` from admitted bundles in `docs/results/bench/`. Do not edit by hand.".into(),
        String::new(),
    ];
    if b.is_empty() {
        l.extend(["No current benchmark bundles are admitted.".into(), String::new()]);
    }
    let mut sec = |t: &str, hd: &[&str], rows: Vec<Vec<String>>| {
        if !rows.is_empty() {
            l.extend([
                format!("## {t}"),
                String::new(),
                format!("| {} |", hd.join(" | ")),
                format!("|{}", "---|".repeat(hd.len())),
            ]);
            l.extend(rows.into_iter().map(|x| format!("| {} |", x.join(" | "))));
            l.push(String::new());
        }
    };
    let f = |v: &Value| v.as_f64();
    let st = |c: &Value| c["st"].as_str().unwrap_or("-").to_string();
    let pick = |p: &'static str| b.iter().filter(move |(k, _)| k.starts_with(p));
    sec(
        "G1: continuous frontier against OR-Library portef",
        &[
            "case",
            "points",
            "max abs diff",
            "range of diff",
            "max rel diff",
            "within 5e-11",
            "rounding rectangles",
            "separation",
            "pass",
            "max KKT residual",
        ],
        pick("g1-")
            .map(|(k, v)| {
                vec![
                    k.clone(),
                    v["n"].to_string(),
                    sci(f(&v["max_abs"]), 3),
                    format!("{} .. {}", sgn(sci(f(&v["min_neg"]), 2)), sgn(sci(f(&v["max_pos"]), 2))),
                    sci(f(&v["max_rel"]), 3),
                    format!("{:.2}%", 100.0 * f(&v["half_unit"]).unwrap_or(f64::NAN)),
                    v["rectangles"].to_string(),
                    sci(f(&v["max_separation"]), 2),
                    if v["pass"] == true { "True" } else { "False" }.into(),
                    sci(f(&v["kkt"]), 1),
                ]
            })
            .collect(),
    );
    let ch: Vec<Vec<String>> = pick("chang-port1")
        .flat_map(|(_, v)| {
            let mut r = vec![vec![
                "SCIP, 50 lambda".into(),
                fx(f(&v["median"]), 4),
                fx(f(&v["mean"]), 4),
                format!("{} (all opt: {})", v["n"], if v["all_opt"] == true { "True" } else { "False" }),
            ]];
            r.extend(CH.iter().map(|(h, m, a)| {
                vec![format!("Chang {h} heuristic, V"), format!("{m:.4}"), format!("{a:.4}"), "50".into()]
            }));
            r
        })
        .collect();
    sec(
        "Supported points against Chang et al. (2000) Table 4, port1, K = 10",
        &["route", "median % error", "mean % error", "points"],
        ch,
    );
    sec(
        "RQ5: F1 and F1p by SCIP, lambda = 0.5, e = 0.01, 1'z = K",
        &["case", "status", "gap", "nodes", "root bound", "objective", "seconds"],
        pick("rq5")
            .map(|(k, v)| {
                let c = &v["c"];
                vec![
                    k.clone(),
                    st(c),
                    sci(f(&c["g"]), 1),
                    c["nd"].to_string(),
                    sci(f(&c["rt"]), 6),
                    sci(f(&v["f"]), 10),
                    fx(f(&c["t"]), 2),
                ]
            })
            .collect(),
    );
    sec(
        "F2 and F3 on indtrack1 returns: solver value against independent recomputation",
        &["case", "status", "objective", "recomputed", "abs diff", "in X(K)"],
        b.iter()
            .filter(|(k, _)| k.starts_with("f2") || k.starts_with("f3"))
            .map(|(k, v)| {
                let c = &v["c"];
                let ok = v["ok"].as_bool().map_or("None".into(), |x| if x { "True" } else { "False" }.to_string());
                vec![k.clone(), st(c), sci(f(&c["f"]), 10), sci(f(&v["recomputed"]), 10), sci(f(&v["diff"]), 1), ok]
            })
            .collect(),
    );
    sec(
        "F4 restricted (no transaction costs), Canakgoz-Beasley windows [1,145], [146,290]",
        &["case", "N", "K", "stage 1", "stage 2", "a in", "b in", "a out", "b out", "TE out"],
        pick("f4-")
            .map(|(k, v)| {
                vec![
                    k.clone(),
                    v["N"].to_string(),
                    v["bundle"]["pa"]["K"].to_string(),
                    st(&v["c1"]),
                    if v["c2"].is_null() { "-".into() } else { st(&v["c2"]) },
                    sci(f(&v["a_in"]), 2),
                    fx(f(&v["b_in"]), 5),
                    fx(f(&v["a_out"]), 5),
                    fx(f(&v["b_out"]), 5),
                    sci(f(&v["te_out"]), 3),
                ]
            })
            .collect(),
    );
    sec(
        "F5 (g = 3, k = g(g+1)) by SCIP spatial branch-and-bound",
        &["case", "status", "objective", "dual bound", "gap", "nodes", "seconds"],
        pick("f5-")
            .map(|(k, v)| {
                let c = &v["c"];
                vec![
                    k.clone(),
                    st(c),
                    sci(f(&v["f"]), 10),
                    sci(f(&c["bd"]), 10),
                    sci(f(&c["g"]), 1),
                    c["nd"].to_string(),
                    fx(f(&c["t"]), 1),
                ]
            })
            .collect(),
    );
    let mut rev: Vec<String> =
        b.iter().filter_map(|(_, v)| v["bundle"]["code"]["commit"].as_str().map(|c| c[..10].to_string())).collect();
    rev.sort();
    rev.dedup();
    let mut hs: Vec<String> =
        b.iter().filter_map(|(_, v)| v["bundle"]["host"]["node"].as_str().map(String::from)).collect();
    hs.sort();
    hs.dedup();
    let none = |v: &[String]| {
        if v.is_empty() { "none".into() } else { v.join(", ") }
    };
    l.push(format!("Revisions: {}. Hosts: {}.", none(&rev), none(&hs)));
    l.push(String::new());
    let p = root().join("docs").join("results").join("README.md");
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    bd::atomic(&p, l.join("\n").as_bytes())
}

/// Explicit sign for a formatted number.
fn sgn(s: String) -> String {
    if s.starts_with('-') || s == "n/a" { s } else { format!("+{s}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_style_scientific_notation() {
        assert_eq!(sci(Some(2.025e-10), 3), "2.025e-10");
        assert_eq!(sci(Some(0.0), 1), "0.0e+00");
        assert_eq!(sci(Some(-6.7287989454e-3), 10), "-6.7287989454e-03");
        assert_eq!(sci(None, 1), "n/a");
        assert_eq!(sgn(sci(Some(1.22e-10), 2)), "+1.22e-10");
    }

    #[test]
    fn case_keys_are_unique_and_inputs_pinned() {
        let c = cases();
        let mut k: Vec<_> = c.iter().map(|x| x.0.clone()).collect();
        k.sort();
        k.dedup();
        assert_eq!(k.len(), c.len());
        for (_, kd, pa) in &c {
            assert!(inputs(kd, pa).is_ok());
        }
    }
}
