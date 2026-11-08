//! `pf eval`: the primary estimands of docs/PLAN.md on one panel: the share of start dates at
//! which a strategy's terminal wealth is below B1, and below B5. Dates whose selection or
//! benchmark is unavailable are counted, not dropped silently.

use crate::{data, ev, oos, panel};
use serde_json::Value;
use std::path::Path;

// Legend
// r : an oos record; W, B1, B5 : terminal wealth of the strategy and of the two benchmarks
// n, na : start dates, dates with an unavailable W; u1, u5 : shares below B1 and B5 over paired dates

/// One strategy's estimands.
#[derive(Debug, PartialEq)]
pub struct Sum {
    pub n: usize,
    pub na: usize,
    pub u1: Option<(f64, usize)>,
    pub u5: Option<(f64, usize)>,
}

fn col(r: &Value, k: &str) -> Vec<Option<f64>> {
    r[k].as_array().map_or(vec![], |a| a.iter().map(Value::as_f64).collect())
}

/// Underperformance shares of record r, each with its number of paired dates.
pub fn summary(r: &Value) -> Result<Sum, String> {
    let (w, b1, b5) = (col(r, "W"), col(r, "B1"), col(r, "B5"));
    if w.len() != b1.len() || w.len() != b5.len() {
        return Err("record columns disagree in length".into());
    }
    let share = |b: &[Option<f64>]| -> Result<Option<(f64, usize)>, String> {
        let (ws, wb): (Vec<f64>, Vec<f64>) = w.iter().zip(b).filter_map(|(a, b)| a.zip(*b)).unzip();
        if ws.is_empty() { Ok(None) } else { Ok(Some((ev::under(&ws, &wb)?, ws.len()))) }
    };
    Ok(Sum { n: w.len(), na: w.iter().filter(|x| x.is_none()).count(), u1: share(&b1)?, u5: share(&b5)? })
}

fn cell(u: Option<(f64, usize)>) -> String {
    u.map_or("n/a".into(), |(p, n)| format!("{p:.4} ({n})"))
}

/// Run the strategies in only (all if empty) on the panel and print one row each.
pub fn main(crsp: &Path, rf: &Path, only: &[String], c: &oos::Run) -> Result<(), String> {
    if let Some(s) = only.iter().find(|s| !oos::ST.contains(&s.as_str())) {
        return Err(format!("unknown strategy {s:?}"));
    }
    let (p0, nf) = panel::panel(&data::crsp(crsp, &[10, 11, 12])?, &data::rf(rf)?, c.fill)?;
    println!("strategy  dates  unavailable  below B1 (paired)  below B5 (paired)");
    for s in oos::ST.iter().filter(|s| only.is_empty() || only.iter().any(|o| o == *s)) {
        let m = summary(&oos::record(s, &p0, c, nf)?)?;
        println!("{s:<8}  {:>5}  {:>11}  {:>17}  {:>17}", m.n, m.na, cell(m.u1), cell(m.u5));
    }
    Ok(())
}
