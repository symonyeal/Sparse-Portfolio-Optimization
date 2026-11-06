//! A fixed walk-forward industry allocation experiment; protocol: docs/INDUSTRY.md.

use crate::crt::{Sol, St};
use crate::x::{Sn, Xk};
use crate::{bd, data, mad};
use nalgebra::DMatrix;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

// r : monthly decimal returns (T,12); t : holding month; m : past estimation months.
// score : sum of twelve past squared candidate returns; w : unchanged solver allocation.
const N: usize = 12;
const K: usize = 3;
const WARM: usize = 132;
const TL: f64 = 0.5;
const TOL: f64 = 1e-7;
const LABELS: [&str; N] =
    ["NoDur", "Durbl", "Manuf", "Enrgy", "Chems", "BusEq", "Telcm", "Utils", "Shops", "Hlth", "Money", "Other"];

struct Industry {
    mo: Vec<i64>,
    r: DMatrix<f64>,
    vintage: String,
}

fn parse(text: &str) -> Result<Industry, String> {
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    let vintage =
        lines.iter().find(|s| s.contains("CRSP database")).ok_or("industry source vintage missing")?.to_string();
    let blocks: Vec<_> =
        lines.iter().enumerate().filter(|(_, s)| **s == "Average Value Weighted Returns -- Monthly").collect();
    if blocks.len() != 1 {
        return Err("expected one monthly value-weighted industry block".into());
    }
    let h = blocks[0].0 + 1;
    let header: Vec<_> = lines.get(h).ok_or("industry header missing")?.split(',').map(str::trim).collect();
    if header.len() != N + 1 || !header[0].is_empty() || header[1..] != LABELS {
        return Err("industry labels differ from the fixed 12-industry universe".into());
    }
    let (mut mo, mut r) = (Vec::new(), Vec::new());
    for row in &lines[h + 1..] {
        if row.is_empty() {
            break;
        }
        let c: Vec<_> = row.split(',').map(str::trim).collect();
        if c.len() != N + 1 || c[0].len() != 6 || !c[0].bytes().all(|b| b.is_ascii_digit()) {
            return Err("malformed industry monthly row".into());
        }
        let month: i64 = c[0].parse().map_err(|_| "invalid industry month")?;
        if month / 100 < 1 || !(1..=12).contains(&(month % 100)) || mo.last().is_some_and(|&p| data::nx(p) != month) {
            return Err("industry months must be valid, unique and consecutive".into());
        }
        mo.push(month);
        for v in &c[1..] {
            let v: f64 = v.parse().map_err(|_| "invalid industry return")?;
            if !v.is_finite() || v == -99.99 || v == -999.0 || v < -100.0 {
                return Err("missing or invalid industry return".into());
            }
            r.push(v / 100.0);
        }
    }
    if mo.is_empty() {
        return Err("empty industry monthly block".into());
    }
    Ok(Industry { r: DMatrix::from_row_slice(mo.len(), N, &r), mo, vintage })
}

fn read(p: &Path) -> Result<Industry, String> {
    let mut raw = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if p.extension().is_some_and(|s| s.eq_ignore_ascii_case("zip")) {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(raw)).map_err(|e| e.to_string())?;
        if z.len() != 1 {
            return Err("industry ZIP must have one CSV member".into());
        }
        raw = Vec::new();
        z.by_index(0).map_err(|e| e.to_string())?.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    }
    parse(&raw.iter().map(|&c| c as char).collect::<String>())
}

fn allocation(q: &Sol, x: Xk, y: &DMatrix<f64>) -> Result<Vec<f64>, String> {
    if !matches!(q.c.st, St::Opt | St::Limit) {
        return Err(format!("solve {}", q.c.st.name()));
    }
    let (w, z) = q.w.as_ref().zip(q.z.as_ref()).ok_or("no incumbent")?;
    let faults = x.ok(w, z, TOL);
    if !faults.is_empty() {
        return Err(format!("invalid allocation: {}", faults.join("; ")));
    }
    let f = mad::dev(y, w);
    if !f.is_finite() || !q.c.f.is_finite() || (f - q.c.f).abs() > TOL * f.abs().max(1.0) {
        return Err("recomputed MAD disagrees with incumbent objective".into());
    }
    Ok(w.clone())
}

/// Choose from historical one-step returns at [t-12,t); future outcomes cannot enter.
fn choice(r: &[Vec<Option<f64>>; 2], t: usize) -> Option<(usize, [f64; 2])> {
    if t < 12 || r.iter().any(|v| v.len() < t) {
        return None;
    }
    let mut scores = [0.0; 2];
    for j in 0..2 {
        for v in &r[j][t - 12..t] {
            scores[j] += v.as_ref()?.powi(2);
        }
    }
    Some((usize::from(scores[1] < scores[0]), scores))
}

fn turnover(prev: &[f64], now: &[f64], r: &[f64]) -> Option<f64> {
    let gross: f64 = prev.iter().zip(r).map(|(w, r)| w * (1.0 + r)).sum();
    if gross <= 0.0 {
        return None;
    }
    Some(0.5 * prev.iter().zip(now).zip(r).map(|((p, w), r)| (w - p * (1.0 + r) / gross).abs()).sum::<f64>())
}

fn summary(r: &[Option<f64>], turns: &[f64]) -> Value {
    let failed = r.iter().filter(|r| r.is_none()).count();
    if failed > 0 {
        return json!({"months": r.len(), "failed": failed, "statistics": null});
    }
    let r: Vec<_> = r.iter().map(|r| r.unwrap()).collect();
    let (mut wealth, mut peak, mut drawdown) = (1.0_f64, 1.0_f64, 0.0_f64);
    for v in &r {
        wealth *= 1.0 + v;
        peak = peak.max(wealth);
        drawdown = drawdown.max(1.0 - wealth / peak);
    }
    let mean = r.iter().sum::<f64>() / r.len() as f64;
    let var = r.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (r.len() - 1) as f64;
    json!({"months": r.len(), "failed": 0, "statistics": {"terminal_wealth": wealth,
        "annualized_volatility": (12.0 * var).sqrt(), "max_drawdown": drawdown,
        "mean_one_way_turnover": if turns.is_empty() { None } else { Some(turns.iter().sum::<f64>() / turns.len() as f64) },
        "turnover_pairs": turns.len()}})
}

/// Run the fixed protocol on a French ZIP/CSV and atomically write a complete JSON result.
/// Input contains revised historical returns, not historical publication vintages.
pub fn main(input: &Path, out: &Path) -> Result<(), String> {
    let parent = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let name = out.file_name().ok_or("industry output needs a filename")?;
    let out = parent.join(name);
    let _run = bd::lease(&parent.join(format!("{}.run", name.to_string_lossy())))?;
    let d = read(input)?;
    let n = d.mo.len();
    if n < WARM + 2 {
        return Err("industry analysis needs at least 134 consecutive months".into());
    }
    let x = Xk::new(N, K, 0.0, 1.0, Sn::Le)?;
    let mut past = [vec![None; n], vec![None; n]];
    let mut paths: [Vec<Option<f64>>; 4] = std::array::from_fn(|_| Vec::new());
    let mut turns: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::new());
    let mut prev: [Option<Vec<f64>>; 4] = std::array::from_fn(|_| None);
    let mut rows = Vec::new();
    let mut validation = Vec::new();
    for t in 120..n {
        let r: Vec<_> = d.r.row(t).iter().copied().collect();
        let mut records = Vec::new();
        let mut weights: [Option<Vec<f64>>; 4] = std::array::from_fn(|_| None);
        // Scores are evaluated before observing this holding month's return.
        let selected = choice(&past, t);
        for (j, m) in [60, 120].into_iter().enumerate() {
            let y = d.r.rows(t - m, m).into_owned();
            let q = mad::mad(&y, x, None, Some(TL), 0.0);
            let (w, raw, c, err) = match q {
                Ok(q) => match allocation(&q, x, &y) {
                    Ok(w) => (Some(w), json!({"w": q.w, "z": q.z, "mad": q.f}), q.c.json(), None),
                    Err(e) => (None, json!({"w": q.w, "z": q.z, "mad": q.f}), q.c.json(), Some(e)),
                },
                Err(e) => (None, Value::Null, Value::Null, Some(e)),
            };
            past[j][t] = w.as_ref().map(|w| w.iter().zip(&r).map(|(w, r)| w * r).sum());
            records.push(json!({"window": m, "w": w, "raw": raw,
                "return": past[j][t], "c": c, "error": err}));
            weights[j] = w;
        }
        if t < WARM {
            validation.push(json!({"month": d.mo[t], "candidates": records}));
            continue;
        }
        weights[2] = selected.and_then(|(j, _)| weights[j].clone());
        weights[3] = Some(vec![1.0 / N as f64; N]);
        let mut monthly = Vec::new();
        for j in 0..4 {
            let ret = weights[j].as_ref().map(|w| w.iter().zip(&r).map(|(w, r)| w * r).sum::<f64>());
            paths[j].push(ret);
            let turn = prev[j].as_ref().zip(weights[j].as_ref()).and_then(|(p, w)| {
                let old: Vec<_> = d.r.row(t - 1).iter().copied().collect();
                turnover(p, w, &old)
            });
            if let Some(turn) = turn {
                turns[j].push(turn);
            }
            monthly.push(json!({"return": ret, "one_way_turnover": turn}));
            prev[j] = weights[j].clone();
        }
        rows.push(json!({"month": d.mo[t], "candidates": records,
            "adaptive": {"window": selected.map(|(j, _)| [60,120][j]),
                "scores": selected.map(|(_, s)| s),
                "error": if selected.is_none() { Some("validation history unavailable") }
                    else if weights[2].is_none() { Some("chosen candidate unavailable") } else { None }},
            "paths": monthly}));
    }
    let stats: Vec<_> = (0..4).map(|j| summary(&paths[j], &turns[j])).collect();
    let result = json!({"schema": 1, "protocol": "industry-20261105", "labels": LABELS,
        "methods": ["fixed60", "fixed120", "adaptive", "equal12"],
        "input": {"sha256": data::sha256(input)?, "vintage": d.vintage,
            "url": "https://mba.tuck.dartmouth.edu/pages/faculty/ken.french/ftp/12_Industry_Portfolios_CSV.zip",
            "block": "Average Value Weighted Returns -- Monthly", "months": n,
            "first": d.mo[0], "last": d.mo[n-1]},
        "code": bd::identity(&data::root()), "protocol_sha256": data::sha256(&data::root().join("docs/INDUSTRY.md"))?,
        "limitations": ["One revised data vintage, not historical publication vintages",
            "Aggregate changing-membership research portfolios, not individual stocks or directly tradable funds",
            "Gross returns exclude trading costs, taxes and fees",
            "Registered CRSP individual-stock questions remain unresolved",
            "Time-limited incumbents can vary across runs or machines; descriptive results establish no empirical superiority"],
        "parameters": {"K": K, "windows": [60,120], "validation": 12, "warmup": WARM,
            "objective": "MAD", "return_floor": null, "loss": "squared monthly return",
            "tie": "shorter window", "time_limit_seconds": TL, "relative_gap": 0.0,
            "feasibility_tolerance": TOL, "weight_repair": false},
        "paired_complete": paths.iter().all(|r| r.iter().all(Option::is_some)),
        "summary": stats, "validation": validation, "rows": rows});
    bd::atomic(&out, &(serde_json::to_string_pretty(&result).map_err(|e| e.to_string())? + "\n").into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(rows: &str) -> String {
        format!(
            "This file was created using the 202609 CRSP database.\nAverage Value Weighted Returns -- Monthly\n,{}\n{rows}\n\nAverage Equal Weighted Returns -- Monthly\ninvalid",
            LABELS.join(",")
        )
    }

    #[test]
    fn industry_block_units_and_invalid_rows() {
        let row = "192607,1.44,2,3,4,5,6,7,8,9,10,11,12";
        let d = parse(&fixture(row)).unwrap();
        assert_eq!(d.mo, [192607]);
        assert!((d.r[(0, 0)] - 0.0144).abs() < 1e-15);
        for bad in [
            row.replace("1.44", "-99.99"),
            row.replace("1.44", "NaN"),
            format!("{row}\n{row}"),
            row.replace("192607", "192613"),
        ] {
            assert!(parse(&fixture(&bad)).is_err());
        }
        assert!(parse(&fixture(row).replace("NoDur", "Unknown")).is_err());
    }

    #[test]
    fn adaptation_uses_only_prior_realized_returns() {
        let mut r = [vec![Some(0.01); 30], vec![Some(0.02); 30]];
        let first = choice(&r, 12);
        assert_eq!(first.unwrap().0, 0);
        for x in &mut r[0][12..] {
            *x = Some(100.0);
        }
        for x in &mut r[1][12..] {
            *x = None;
        }
        assert_eq!(choice(&r, 12), first);
        assert!(choice(&r, 13).is_none());
        r[0][0] = None;
        assert!(choice(&r, 12).is_none());
        assert_eq!(choice(&[vec![Some(0.01); 12], vec![Some(0.01); 12]], 12).unwrap().0, 0);
    }

    #[test]
    fn drifted_turnover_and_path_statistics() {
        assert!((turnover(&[0.5, 0.5], &[0.5, 0.5], &[1.0, 0.0]).unwrap() - 1.0 / 6.0).abs() < 1e-12);
        let s = summary(&[Some(0.1), Some(-0.1)], &[0.2]);
        assert!((s["statistics"]["terminal_wealth"].as_f64().unwrap() - 0.99).abs() < 1e-12);
        assert!((s["statistics"]["max_drawdown"].as_f64().unwrap() - 0.1).abs() < 1e-12);
        assert!(summary(&[Some(0.1), None], &[])["statistics"].is_null());
    }

    #[test]
    fn accepted_incumbent_keeps_the_point_and_recomputes_mad() {
        let y = DMatrix::from_fn(60, N, |t, i| if i == 0 { 0.01 } else { ((t * (i + 1) % 13) as f64 - 6.0) / 100.0 });
        let x = Xk::new(N, K, 0.0, 1.0, Sn::Le).unwrap();
        let q = mad::mad(&y, x, None, Some(TL), 0.0).unwrap();
        let w = allocation(&q, x, &y).unwrap();
        assert_eq!(Some(&w), q.w.as_ref());
        assert!(mad::dev(&y, &w) < 1e-7);
        let mut bad = q.clone();
        bad.w.as_mut().unwrap()[0] = 0.5;
        assert!(allocation(&bad, x, &y).is_err());
        bad = q.clone();
        bad.c.f = 1.0;
        assert!(allocation(&bad, x, &y).is_err());
    }
}
