//! Solve certificates: the status, primal value, dual bound and gap read back from the solver.
//!
//! A certificate is built only from fields the solver returned. A requested tolerance is never
//! reported as a result: HiGHS stops at mip_rel_gap = 1e-4 unless told otherwise, and SCIP at
//! limits/gap = 0. A certificate whose dual bound lies past its primal value by more than the
//! tolerance is a defect and is refused.

use serde_json::{Value, json};

// Legend
// st   : status; sn : objective sense
// f    : primal value read back (objective at the incumbent); NaN when there is none
// bd   : dual bound read back; -inf (min) or +inf (max) when the solver returned none
// g    : achieved relative gap |f - bd| / max(|f|, GA); inf when either side is missing
// rg   : achieved gap as the solver reports it (HiGHS mip_gap, SCIP getGap)
// nd   : branch-and-bound nodes; t : wall seconds; rt : root dual bound (SCIP), when read
// ps   : presolve setting of the solve that produced the certificate
// first: status of a first solve that was repeated with presolve off
// bs   : where the dual bound was read; sg : objective scale divided back out (1 if none)
// tl   : time limit (s); gp : requested relative gap

/// Tolerance of the crossed-bound check, relative to max(1, |f|).
pub const TOL: f64 = 1e-7;
/// Floor of the gap denominator.
pub const GA: f64 = 1e-10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum St {
    Opt,
    Limit,
    Infeasible,
    Unbounded,
    Numerical,
}

impl St {
    pub fn name(self) -> &'static str {
        match self {
            St::Opt => "opt",
            St::Limit => "limit",
            St::Infeasible => "infeasible",
            St::Unbounded => "unbounded",
            St::Numerical => "numerical",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    Min,
    Max,
}

#[derive(Clone, Debug)]
pub struct C {
    pub st: St,
    pub sn: Sense,
    pub f: f64,
    pub bd: f64,
    pub g: f64,
    pub rg: Option<f64>,
    pub nd: Option<i64>,
    pub t: f64,
    pub rt: Option<f64>,
    pub ps: bool,
    pub first: Option<String>,
    pub bs: &'static str,
    pub sg: f64,
    pub msg: String,
}

/// Relative gap |f - bd| / max(|f|, GA).
pub fn gap(f: f64, bd: f64) -> f64 {
    if !(f.is_finite() && bd.is_finite()) {
        return f64::INFINITY;
    }
    (f - bd).abs() / f.abs().max(GA)
}

/// True when the dual bound lies past the primal value by more than tol (relative).
pub fn crossed(sn: Sense, f: f64, bd: f64, tol: f64) -> bool {
    if !(f.is_finite() && bd.is_finite()) {
        return false;
    }
    let s = tol * f.abs().max(1.0);
    match sn {
        Sense::Min => bd > f + s,
        Sense::Max => bd < f - s,
    }
}

impl C {
    /// A certificate from read-back values; refuses crossed bounds and an unproved `opt`.
    pub fn new(st: St, sn: Sense, f: Option<f64>, bd: Option<f64>, t: f64, tol: f64) -> Result<C, String> {
        let f = f.unwrap_or(f64::NAN);
        let bd = bd.unwrap_or(match sn {
            Sense::Min => f64::NEG_INFINITY,
            Sense::Max => f64::INFINITY,
        });
        if crossed(sn, f, bd, tol) {
            return Err(format!("crossed certificate: sense {sn:?}, primal {f:e}, dual bound {bd:e}"));
        }
        if st == St::Opt && !f.is_finite() {
            return Err("status opt without a primal value".into());
        }
        if st == St::Opt && !bd.is_finite() {
            return Err("status opt without a finite dual bound".into());
        }
        Ok(C {
            st,
            sn,
            f,
            bd,
            g: gap(f, bd),
            rg: None,
            nd: None,
            t,
            rt: None,
            ps: true,
            first: None,
            bs: "",
            sg: 1.0,
            msg: String::new(),
        })
    }

    /// A limit reached before any solve started.
    pub fn none(sn: Sense, msg: &str) -> C {
        let mut c = C::new(St::Limit, sn, None, None, 0.0, TOL).unwrap();
        c.msg = msg.into();
        c
    }

    /// The certificate of an objective scaled by sg > 0, in the original units.
    pub fn un(mut self, sg: f64) -> C {
        assert!(sg.is_finite() && sg > 0.0, "objective scale must be finite and positive");
        self.f /= sg;
        self.bd /= sg;
        self.g = gap(self.f, self.bd);
        self.rt = self.rt.map(|r| r / sg);
        self.sg = sg;
        self
    }

    pub fn json(&self) -> Value {
        json!({
            "st": self.st.name(), "sn": if self.sn == Sense::Min { "min" } else { "max" },
            "f": num(self.f), "bd": num(self.bd), "g": num(self.g), "rg": self.rg.and_then(num),
            "nd": self.nd, "t": self.t, "rt": self.rt.and_then(num), "ps": self.ps,
            "first": self.first, "bs": self.bs, "sg": self.sg, "msg": self.msg,
        })
    }
}

/// A formulation's incumbent with its recomputed objective f and certificate c; w and z are
/// None (f NaN) when the solver returned no incumbent.
#[derive(Clone, Debug)]
pub struct Sol {
    pub w: Option<Vec<f64>>,
    pub z: Option<Vec<f64>>,
    pub f: f64,
    pub c: C,
}

impl Sol {
    /// Split a solver point x = [w, z, ...] over N assets, recomputing the objective by obj.
    pub fn of(x: Option<Vec<f64>>, cols: crate::x::Cols, c: C, obj: impl Fn(&[f64]) -> f64) -> Sol {
        match x {
            None => Sol { w: None, z: None, f: f64::NAN, c },
            Some(x) => {
                let w = x[cols.w].to_vec();
                Sol { f: obj(&w), z: Some(x[cols.z].iter().map(|v| v.round()).collect()), w: Some(w), c }
            }
        }
    }
}

/// A JSON number, or null for a nonfinite value.
pub fn num(x: f64) -> Option<f64> {
    x.is_finite().then_some(x)
}

/// Reject nonfinite or negative time and gap limits.
pub fn limits(tl: Option<f64>, gp: f64) -> Result<(), String> {
    if tl.is_some_and(|t| !t.is_finite() || t < 0.0) {
        return Err("tl must be finite and nonnegative".into());
    }
    if !gp.is_finite() || gp < 0.0 {
        return Err("gp must be finite and nonnegative".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossed_and_unproved_certificates_are_refused() {
        assert!(C::new(St::Opt, Sense::Min, Some(1.0), Some(1.1), 0.0, TOL).is_err());
        assert!(C::new(St::Opt, Sense::Max, Some(1.0), Some(0.9), 0.0, TOL).is_err());
        assert!(C::new(St::Opt, Sense::Min, Some(1.0), None, 0.0, TOL).is_err());
        assert!(C::new(St::Opt, Sense::Min, None, Some(1.0), 0.0, TOL).is_err());
        let c = C::new(St::Limit, Sense::Min, Some(2.0), Some(1.0), 0.0, TOL).unwrap();
        assert_eq!(c.g, 0.5);
        let c = c.un(4.0);
        assert_eq!((c.f, c.bd, c.g), (0.5, 0.25, 0.5));
    }

    #[test]
    fn invalid_limits_are_rejected() {
        assert!(limits(Some(-1.0), 0.0).is_err());
        assert!(limits(Some(f64::NAN), 0.0).is_err());
        assert!(limits(None, f64::INFINITY).is_err());
        assert!(limits(Some(0.0), 0.0).is_ok());
    }
}
