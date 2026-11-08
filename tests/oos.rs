//! The out-of-sample engine on a synthetic CRSP-format panel, against pinned values.
//!
//! tests/data/panel.csv holds 40 synthetic firms over 60 months (1990-1994): entrants, a shared
//! PERMCO, delistings with and without DLRET, a missing-code cell, a series that ends without a
//! delisting and a non-common share code. tests/data/rf.csv is a synthetic RF series in the
//! French file layout. The pins in tests/data/oos.json are the terminal wealth, held PERMCOs
//! and selection status at every date from the Python implementation at revision 1ca7e76
//! (K = 3, M = 12, m = 6, Nc = 8, H = 6), with and without the Shumway fill. The data carry no
//! empirical content.

use pf::{data, oos, panel};
use serde_json::Value;
use std::path::PathBuf;

fn at(f: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(f)
}

#[test]
fn strategies_reproduce_the_pinned_paths() {
    let pins: Value = serde_json::from_str(&std::fs::read_to_string(at("oos.json")).unwrap()).unwrap();
    let rows = data::crsp(&at("panel.csv"), &[10, 11, 12]).unwrap();
    let rf = data::rf(&at("rf.csv")).unwrap();
    let pa = &pins["pa"];
    let u = |k: &str| pa[k].as_u64().unwrap() as usize;
    for (key, b) in pins["runs"].as_object().unwrap() {
        let (s, fill) = key.split_once('-').unwrap();
        let fill = fill == "fill";
        let (p0, nf) = panel::panel(&rows, &rf, fill).unwrap();
        let c = oos::Run { k: u("K"), mw: u("M"), m: u("m"), nc: u("Nc"), h: u("H"), fill, tl: 20.0, seed: 0 };
        let r = oos::record(s, &p0, &c, nf).unwrap();
        assert_eq!(r["fills"], b["fills"], "{key}");
        assert_eq!(r["pc"], b["pc"], "{key}");
        let st: Vec<&Value> = r["selection"].as_array().unwrap().iter().map(|x| &x["status"]).collect();
        assert_eq!(st, b["status"].as_array().unwrap().iter().collect::<Vec<_>>(), "{key}");
        // SCIP solves F1 to within its 1e-9 feasibility tolerance; the other routes agree exactly.
        let tol = if s == "f1" { 1e-6 } else { 1e-12 };
        let (w, p) = (r["W"].as_array().unwrap(), b["W"].as_array().unwrap());
        assert_eq!(w.len(), p.len(), "{key}");
        for (a, p) in w.iter().zip(p) {
            match (a.as_f64(), p.as_f64()) {
                (Some(a), Some(p)) => assert!((a - p).abs() <= tol * p.abs(), "{key}: W {a} against {p}"),
                _ => assert_eq!(a, p, "{key}"),
            }
        }
    }
}

#[test]
fn eval_counts_the_underperformance_shares_on_the_fixture() {
    let rows = data::crsp(&at("panel.csv"), &[10, 11, 12]).unwrap();
    let rf = data::rf(&at("rf.csv")).unwrap();
    let (p0, nf) = panel::panel(&rows, &rf, false).unwrap();
    let c = oos::Run { k: 3, mw: 12, m: 6, nc: 8, h: 6, fill: false, tl: 20.0, seed: 0 };
    let r = oos::record("b3", &p0, &c, nf).unwrap();
    let m = pf::eval::summary(&r).unwrap();
    let f = |k: &str| -> Vec<Option<f64>> { r[k].as_array().unwrap().iter().map(Value::as_f64).collect() };
    let (w, b1, b5) = (f("W"), f("B1"), f("B5"));
    let by = |b: &Vec<Option<f64>>| {
        let p: Vec<(f64, f64)> = w.iter().zip(b).filter_map(|(a, b)| a.zip(*b)).collect();
        (p.iter().filter(|(a, b)| a < b).count() as f64 / p.len() as f64, p.len())
    };
    assert_eq!(m.n, w.len());
    assert_eq!(m.na, w.iter().filter(|x| x.is_none()).count());
    assert_eq!(m.u1, Some(by(&b1)));
    assert_eq!(m.u5, Some(by(&b5)));
    assert!(m.n > 0 && m.u1.unwrap().1 > 0);
}
