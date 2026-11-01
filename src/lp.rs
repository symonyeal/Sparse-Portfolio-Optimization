//! A mixed-integer linear program over rows lo <= a x <= hi and its HiGHS solve with certificate.
//!
//! A reported infeasibility is read back from a second solve with presolve off: HiGHS presolve
//! declared feasible programs infeasible in Electricity-Clearing-and-Pricing.

use crate::crt::{C, Sense as Obj, St, TOL, limits};
use highs::{HighsModelStatus, HighsSolutionStatus, RowProblem, Sense};
use std::time::Instant;

// Legend
// c       : objective; lb, ub : column bounds; int : integrality of each column
// a       : sparse rows (column, coefficient); lo, hi : row bounds (infinite when absent)
// tl, gp  : time limit (s) and requested relative gap; ps : presolve on

pub type Row = Vec<(usize, f64)>;

#[derive(Clone, Debug, Default)]
pub struct Lp {
    pub c: Vec<f64>,
    pub lb: Vec<f64>,
    pub ub: Vec<f64>,
    pub int: Vec<bool>,
    pub a: Vec<Row>,
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
}

impl Lp {
    /// Append a column; returns its index.
    pub fn col(&mut self, c: f64, lb: f64, ub: f64, int: bool) -> usize {
        self.c.push(c);
        self.lb.push(lb);
        self.ub.push(ub);
        self.int.push(int);
        self.c.len() - 1
    }

    pub fn row(&mut self, a: Row, lo: f64, hi: f64) {
        self.a.push(a);
        self.lo.push(lo);
        self.hi.push(hi);
    }

    /// Minimise c'x with HiGHS; returns the incumbent (if any) and the certificate.
    pub fn solve(&self, tl: Option<f64>, gp: f64) -> Result<(Option<Vec<f64>>, C), String> {
        limits(tl, gp)?;
        if tl == Some(0.0) {
            return Ok((None, C::none(Obj::Min, "time limit before solve")));
        }
        let n = self.c.len();
        if !self.int.iter().any(|&b| b) {
            return Err("no integer column: the certificate needs HiGHS's MIP dual bound".into());
        }
        if self.c.iter().any(|x| !x.is_finite())
            || self.a.iter().flatten().any(|&(j, v)| j >= n || !v.is_finite())
            || self.lb.iter().chain(&self.ub).chain(&self.lo).chain(&self.hi).any(|x| x.is_nan())
        {
            return Err("objective and row coefficients must be finite; bounds must not be NaN".into());
        }
        let t0 = Instant::now();
        let (x, mut c) = self.run(tl, gp, true)?;
        if c.st != St::Infeasible {
            return Ok((x, c));
        }
        let t = t0.elapsed().as_secs_f64();
        let (x, mut c2) = self.run(tl.map(|l| (l - t).max(0.0)), gp, false)?;
        c2.t += c.t;
        c2.first = Some(std::mem::take(&mut c.msg));
        Ok((x, c2))
    }

    fn run(&self, tl: Option<f64>, gp: f64, ps: bool) -> Result<(Option<Vec<f64>>, C), String> {
        let mut p = RowProblem::new();
        let cols: Vec<_> = (0..self.c.len())
            .map(|j| p.add_column_with_integrality(self.c[j], self.lb[j]..=self.ub[j], self.int[j]))
            .collect();
        for (a, (&lo, &hi)) in self.a.iter().zip(self.lo.iter().zip(&self.hi)) {
            p.add_row(lo..=hi, a.iter().filter(|t| t.1 != 0.0).map(|&(j, v)| (cols[j], v)));
        }
        let mut m = p.try_optimise(Sense::Minimise).map_err(|e| format!("HiGHS model: {e:?}"))?;
        m.make_quiet();
        m.set_option("mip_rel_gap", gp);
        m.set_option("presolve", if ps { "on" } else { "off" });
        if let Some(l) = tl {
            m.set_option("time_limit", l);
        }
        let t0 = Instant::now();
        let s = m.try_solve().map_err(|e| format!("HiGHS solve: {e:?}"))?;
        let t = t0.elapsed().as_secs_f64();
        let ms = s.status();
        let st = match ms {
            HighsModelStatus::Optimal => St::Opt,
            HighsModelStatus::ReachedTimeLimit
            | HighsModelStatus::ReachedIterationLimit
            | HighsModelStatus::ReachedSolutionLimit
            | HighsModelStatus::ObjectiveBound
            | HighsModelStatus::ObjectiveTarget => St::Limit,
            HighsModelStatus::Infeasible => St::Infeasible,
            HighsModelStatus::Unbounded => St::Unbounded,
            _ => St::Numerical,
        };
        let has = s.primal_solution_status() == HighsSolutionStatus::Feasible;
        let x = has.then(|| s.get_solution().columns().to_vec());
        let f = has.then(|| s.objective_value());
        // HiGHS reports a missing bound as an infinite value.
        let bd = s.double_info_value(c"mip_dual_bound").ok().filter(|b| b.abs() < 1e20);
        let mut c = C::new(st, Obj::Min, f, bd, t, TOL)?;
        let mut nd = -1i64;
        // SAFETY: the pointer is the live solved model; the key is NUL-terminated. The highs
        // crate reads this int64 counter as a 32-bit value and fails, hence the raw call.
        let ok = unsafe { highs_sys::Highs_getInt64InfoValue(s.as_ptr(), c"mip_node_count".as_ptr(), &mut nd) };
        c.nd = (ok == 0 && nd >= 0).then_some(nd);
        c.rg = Some(s.mip_gap()).filter(|g| g.is_finite());
        c.ps = ps;
        c.bs = "mip_dual_bound";
        c.msg = format!("{ms:?}");
        Ok((x, c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn knap() -> Lp {
        // min -5a - 4b - 3c s.t. 2a + 3b + c <= 5, 4a + b + 2c <= 11, 3a + 4b + 2c <= 8; binary
        let mut p = Lp::default();
        for c in [-5.0, -4.0, -3.0] {
            p.col(c, 0.0, 1.0, true);
        }
        for (r, h) in [([2.0, 3.0, 1.0], 5.0), ([4.0, 1.0, 2.0], 11.0), ([3.0, 4.0, 2.0], 8.0)] {
            p.row(r.iter().enumerate().map(|(j, &v)| (j, v)).collect(), f64::NEG_INFINITY, h);
        }
        p
    }

    #[test]
    fn milp_is_read_back() {
        let (x, c) = knap().solve(None, 0.0).unwrap();
        assert_eq!(c.st, St::Opt);
        assert_eq!(c.f, -9.0);
        assert!((c.bd - c.f).abs() < 1e-9 && c.bs == "mip_dual_bound");
        assert_eq!(x.unwrap().iter().map(|v| v.round()).collect::<Vec<_>>(), [1.0, 1.0, 0.0]);
    }

    #[test]
    fn infeasibility_is_read_back_with_presolve_off() {
        let mut p = knap();
        p.row(vec![(0, 1.0), (1, 1.0), (2, 1.0)], 4.0, f64::INFINITY);
        let (x, c) = p.solve(None, 0.0).unwrap();
        assert!(x.is_none());
        assert_eq!(c.st, St::Infeasible);
        assert!(!c.ps && c.first.is_some());
    }

    #[test]
    fn zero_deadline_starts_no_solve() {
        let (x, c) = knap().solve(Some(0.0), 0.0).unwrap();
        assert!(x.is_none() && c.st == St::Limit && c.f.is_nan());
    }
}
