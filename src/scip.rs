//! SCIP models over the columns and rows of an `Lp`, and their certificates.
//!
//! A formulation writes its linear part once into an `Lp` (X(K) included), opens it here as a
//! SCIP model, adds its nonlinear constraints over `v`, and solves. A reported infeasibility is
//! rechecked with presolve off within the deadline, as for HiGHS.

use crate::crt::{C, Sense, St, TOL, limits};
use crate::lp::Lp;
use russcip::prelude::*;
use russcip::{ParamSetting, ProblemCreated, Variable, ffi};
use std::time::Instant;

// Legend
// m  : SCIP model in the problem stage; v : its variables, one per column of the Lp
// ft : SCIP feasibility tolerance (numerics/feastol); gp : requested relative gap
// tl : time limit (s)

pub struct Sp {
    pub m: Model<ProblemCreated>,
    pub v: Vec<Variable>,
}

/// A SCIP model holding the columns (with objective) and rows of p.
pub fn open(p: &Lp, sn: Sense, ft: f64, gp: f64) -> Result<Sp, String> {
    limits(None, gp)?;
    if (0..p.c.len()).any(|j| p.int[j] && (p.lb[j] != 0.0 || p.ub[j] != 1.0)) {
        return Err("SCIP adapter supports binary integer columns only".into());
    }
    let mut m = Model::default().hide_output();
    m = if sn == Sense::Min { m.minimize() } else { m.maximize() };
    let r = |e| format!("SCIP parameter: {e:?}");
    m = m.set_real_param("numerics/feastol", ft).map_err(r)?;
    m = m.set_real_param("limits/gap", gp).map_err(r)?;
    m = m.set_real_param("limits/absgap", 0.0).map_err(r)?;
    let v: Vec<Variable> = (0..p.c.len())
        .map(|j| {
            let b = var().obj(p.c[j]);
            m.add(if p.int[j] && p.lb[j] == 0.0 && p.ub[j] == 1.0 { b.bin() } else { b.cont(p.lb[j]..=p.ub[j]) })
        })
        .collect();
    for (a, (&lo, &hi)) in p.a.iter().zip(p.lo.iter().zip(&p.hi)) {
        let a: Vec<_> = a.iter().filter(|t| t.1 != 0.0).collect();
        m.add(cons().coefs(a.iter().map(|t| &v[t.0]).collect(), a.iter().map(|t| t.1).collect()).bounds(lo, hi));
    }
    Ok(Sp { m, v })
}

/// Solve, rechecking infeasibility without presolve; returns the values of v and the certificate.
pub fn solve(sp: Sp, sn: Sense, tl: Option<f64>) -> Result<(Option<Vec<f64>>, C), String> {
    limits(tl, 0.0)?;
    if tl == Some(0.0) {
        return Ok((None, C::none(sn, "time limit before solve")));
    }
    let Sp { mut m, v } = sp;
    if let Some(l) = tl {
        m = m.set_real_param("limits/time", l).map_err(|e| format!("SCIP parameter: {e:?}"))?;
    }
    let t0 = Instant::now();
    let mut s = m.solve();
    let first = s.status();
    let mut ps = true;
    if matches!(first, Status::Infeasible | Status::Inforunbd) {
        let rem = tl.map(|l| (l - t0.elapsed().as_secs_f64()).max(0.0));
        if rem == Some(0.0) {
            let mut c = C::none(sn, "no time for infeasibility recheck");
            c.t = t0.elapsed().as_secs_f64();
            c.first = Some(format!("{first:?}"));
            return Ok((None, c));
        }
        let mut m = s.free_transform().set_presolving(ParamSetting::Off);
        if let Some(l) = rem {
            m = m.set_real_param("limits/time", l).map_err(|e| format!("SCIP parameter: {e:?}"))?;
        }
        s = m.solve();
        ps = false;
    }
    let st = match s.status() {
        Status::Optimal => St::Opt,
        Status::TimeLimit
        | Status::NodeLimit
        | Status::TotalNodeLimit
        | Status::StallNodeLimit
        | Status::GapLimit
        | Status::MemoryLimit
        | Status::SolutionLimit
        | Status::BestSolutionLimit
        | Status::UserInterrupt => St::Limit,
        Status::Infeasible => St::Infeasible,
        Status::Unbounded => St::Unbounded,
        // An ambiguous status does not certify infeasibility; the raw status is kept in msg.
        _ => St::Numerical,
    };
    let x = s.best_sol().map(|b| v.iter().map(|x| b.val(x)).collect::<Vec<f64>>());
    let p = s.scip_ptr();
    // Safety: p is the live SCIP of the solved model s, which outlives these reads.
    let (pb, bd, rg, nd, rt, inf) = unsafe {
        (
            ffi::SCIPgetPrimalbound(p),
            ffi::SCIPgetDualbound(p),
            ffi::SCIPgetGap(p),
            ffi::SCIPgetNNodes(p),
            ffi::SCIPgetDualboundRoot(p),
            ffi::SCIPinfinity(p),
        )
    };
    let fin = |b: f64| (b.abs() < inf).then_some(b);
    let mut c = C::new(st, sn, x.as_ref().map(|_| pb), fin(bd), t0.elapsed().as_secs_f64(), TOL)?;
    c.nd = Some(nd);
    c.rt = fin(rt);
    c.rg = Some(rg).filter(|g| g.is_finite() && g.abs() < inf);
    c.ps = ps;
    c.first = Some(format!("{first:?}"));
    c.bs = "SCIP getDualbound";
    c.msg = format!("{:?}", s.status());
    Ok((x, c))
}
