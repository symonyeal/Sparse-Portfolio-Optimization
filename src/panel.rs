//! The monthly panel and the out-of-sample engine: eligibility, selection at t, buy-and-hold.
//!
//! Selection at t receives `P::upto(t)`, a copy holding only months <= t, so no weight can depend
//! on a later return (the no-look-ahead test perturbs every later return). Evaluation then holds
//! the weights on the full panel. Protocol: specification sec. 8; DeMiguel, Garlappi and Uppal
//! (2009) for the rolling estimation window M.
//!
//! Holding: buy-and-hold for H months. After a recorded delisting month (whose return includes
//! DLRET), proceeds earn the T-bill rate. A missing return in the delisting month itself is an
//! unknown terminal value and is read as a total loss, so the date stays in the comparison sample
//! whatever the delisting turns out to be. Other missing holding returns are refused.

use crate::crt::C;
use crate::data::{Crsp, Rf, nx};
use crate::wc::comb;
use nalgebra::DMatrix;
use rand::Rng;
use serde_json::{Map, Value};

// Legend
// mo      : months YYYYMM (Tm); ids : PERMNO (N)
// r       : combined returns (Tm, N), NaN where the stock has no return that month
// me      : end-of-month market value |PRC| SHROUT (Tm, N), NaN where unknown
// rf      : T-bill return per month (Tm)
// pct, pc : monthly PERMCO labels (Tm, N, 0 when absent) and the last label seen per column
// ls      : explicit delisting-month index per column; Tm means no observed delisting
// j       : original column index of each column in the full panel
// t       : month index of a rebalance date; M : estimation window; m : required returns in it
// nc      : eligible universe size; U : eligible column indices, largest first
// K, w, H : names held, their weights, holding months; W : wealth path, 1 at the end of t

#[derive(Clone, Debug)]
pub struct P {
    pub(crate) mo: Vec<i64>,
    pub(crate) ids: Vec<i64>,
    pub(crate) r: DMatrix<f64>,
    pub(crate) me: DMatrix<f64>,
    pub(crate) rf: Vec<f64>,
    pub(crate) pct: DMatrix<i64>,
    pub(crate) pc: Vec<i64>,
    pub(crate) ls: Vec<usize>,
    pub(crate) j: Vec<usize>,
}

/// An immutable information set made only by cutting a validated full panel.
#[derive(Clone, Debug)]
pub struct History(P);

impl std::ops::Deref for History {
    type Target = P;
    fn deref(&self) -> &P {
        &self.0
    }
}

impl P {
    pub fn months(&self) -> &[i64] {
        &self.mo
    }
    pub fn returns(&self) -> &DMatrix<f64> {
        &self.r
    }
    pub fn new(
        mo: Vec<i64>,
        ids: Vec<i64>,
        pct: DMatrix<i64>,
        r: DMatrix<f64>,
        me: DMatrix<f64>,
        rf: Vec<f64>,
        ls: Option<Vec<usize>>,
        j: Option<Vec<usize>>,
    ) -> Result<P, String> {
        let p = Self::checked(mo, ids, pct, r, me, rf, ls, j)?;
        if p.j.iter().copied().ne(0..p.r.ncols()) {
            return Err("a full panel must use its own column indices".into());
        }
        Ok(p)
    }

    fn checked(
        mo: Vec<i64>,
        ids: Vec<i64>,
        pct: DMatrix<i64>,
        r: DMatrix<f64>,
        me: DMatrix<f64>,
        rf: Vec<f64>,
        ls: Option<Vec<usize>>,
        j: Option<Vec<usize>>,
    ) -> Result<P, String> {
        let (tm, n) = r.shape();
        if tm == 0 || me.shape() != (tm, n) || rf.len() != tm || mo.len() != tm || ids.len() != n {
            return Err("panel arrays disagree in shape".into());
        }
        if mo.iter().any(|m| !(1..=12).contains(&(m % 100))) || mo.windows(2).any(|w| w[1] != nx(w[0])) {
            return Err("panel months must be valid and consecutive".into());
        }
        if rf.iter().any(|x| !x.is_finite() || *x < -1.0) {
            return Err("T-bill return missing in a panel month".into());
        }
        if r.iter().any(|x| x.is_infinite() || *x < -1.0) {
            return Err("panel return must be finite or missing and at least -1".into());
        }
        if pct.shape() != (tm, n) {
            return Err("PERMCO labels disagree in shape".into());
        }
        // An absent monthly label is carried from prior observations only.
        let mut pc = vec![0; n];
        for t in 0..tm {
            for i in 0..n {
                if pct[(t, i)] != 0 {
                    pc[i] = pct[(t, i)];
                }
            }
        }
        let ls = ls.unwrap_or(vec![tm; n]);
        let j = j.unwrap_or((0..n).collect());
        if ls.len() != n || j.len() != n || ls.iter().any(|&s| s > tm) {
            return Err("panel column metadata disagree in shape".into());
        }
        if ids.iter().collect::<std::collections::BTreeSet<_>>().len() != n
            || j.iter().collect::<std::collections::BTreeSet<_>>().len() != n
            || me.iter().any(|v| v.is_infinite() || *v < 0.0)
        {
            return Err("panel identities must be unique and market values nonnegative or missing".into());
        }
        Ok(P { mo, ids, r, me, rf, pct, pc, ls, j })
    }

    /// Observed columns, company labels and delistings through month t only.
    pub fn upto(&self, t: usize) -> History {
        assert!(t < self.r.nrows(), "cutoff is outside the panel");
        let k: Vec<usize> = (0..self.r.ncols())
            .filter(|&i| (0..=t).any(|s| self.r[(s, i)].is_finite() || self.me[(s, i)].is_finite()))
            .collect();
        let rows = |m: &DMatrix<f64>| m.rows(0, t + 1).select_columns(&k);
        History(
            P::checked(
                self.mo[..=t].to_vec(),
                k.iter().map(|&i| self.ids[i]).collect(),
                self.pct.rows(0, t + 1).select_columns(&k),
                rows(&self.r),
                rows(&self.me),
                self.rf[..=t].to_vec(),
                Some(k.iter().map(|&i| self.ls[i].min(t + 1)).collect()),
                Some(k.iter().map(|&i| self.j[i]).collect()),
            )
            .unwrap(),
        )
    }

    /// Refuse an incomplete holding horizon.
    pub fn horizon(&self, t: usize, h: usize) -> Result<(), String> {
        if h < 1 || t.checked_add(h).is_none_or(|s| s >= self.r.nrows()) {
            return Err("holding horizon is outside the panel".into());
        }
        Ok(())
    }

    /// Wealth path over months t+1..t+H of weights w on columns cols, bought at the end of t.
    pub fn hold(&self, t: usize, cols: &[usize], w: &[f64], h: usize) -> Result<Vec<f64>, String> {
        self.horizon(t, h)?;
        if cols.len() != w.len()
            || w.iter().any(|x| !x.is_finite())
            || cols.iter().any(|&i| i >= self.r.ncols())
            || cols.iter().collect::<std::collections::BTreeSet<_>>().len() != cols.len()
        {
            return Err("holding columns and weights disagree".into());
        }
        if (w.iter().sum::<f64>() - 1.0).abs() > 1e-9 || w.iter().any(|&x| x < -1e-12) {
            return Err("weights must be nonnegative and sum to one".into());
        }
        let mut v = w.to_vec();
        let mut cash = 0.0;
        let mut wp = Vec::with_capacity(h);
        for s in t + 1..=t + h {
            cash *= 1.0 + self.rf[s];
            for (k, &c) in cols.iter().enumerate() {
                if v[k] <= 0.0 {
                    continue;
                }
                let r = self.r[(s, c)];
                if r.is_finite() {
                    v[k] *= 1.0 + r;
                } else if s > self.ls[c] {
                    // left the panel after its delisting month
                    cash += v[k] * (1.0 + self.rf[s]);
                    v[k] = 0.0;
                } else if s == self.ls[c] {
                    // delisting month with RET and DLRET both missing: total loss
                    v[k] = 0.0;
                } else {
                    return Err("missing return while still listed or without a confirmed delisting".into());
                }
            }
            wp.push(v.iter().sum::<f64>() + cash);
        }
        Ok(wp)
    }

    /// B1: the value-weighted market of every stock with a return and a market value.
    pub fn b1(&self, t: usize, h: usize) -> Result<Vec<f64>, String> {
        self.horizon(t, h)?;
        let r = crate::bs::vw(&self.r.rows(t + 1, h).into_owned(), &self.me.rows(t, h).into_owned())?;
        Ok(cum(&r))
    }

    /// B5: the one-month T-bill rolled.
    pub fn b5(&self, t: usize, h: usize) -> Result<Vec<f64>, String> {
        self.horizon(t, h)?;
        Ok(cum(&self.rf[t + 1..=t + h]))
    }
}

/// Wealth prod(1 + r) after each period.
pub fn cum(r: &[f64]) -> Vec<f64> {
    r.iter()
        .scan(1.0, |a, x| {
            *a *= 1.0 + x;
            Some(*a)
        })
        .collect()
}

/// A panel from CRSP rows and the T-bill series; returns (panel, number of Shumway fills).
pub fn panel(c: &Crsp, rf: &Rf, fill: bool) -> Result<(P, usize), String> {
    let mut mo = c.mo.clone();
    mo.sort();
    mo.dedup();
    let mut ids = c.permno.clone();
    ids.sort();
    ids.dedup();
    let (r0, n) = comb(&c.ret, &c.dlret, &c.dl, &c.exchcd, fill)?;
    let (tm, m) = (mo.len(), ids.len());
    let mut r = DMatrix::from_element(tm, m, f64::NAN);
    let mut me = r.clone();
    let mut pct = DMatrix::zeros(tm, m);
    let mut ls = vec![tm; m];
    for k in 0..c.mo.len() {
        let (i, j) = (mo.binary_search(&c.mo[k]).unwrap(), ids.binary_search(&c.permno[k]).unwrap());
        r[(i, j)] = r0[k];
        me[(i, j)] = c.prc[k].abs() * c.shrout[k];
        pct[(i, j)] = c.permco[k];
        if c.dl[k] != 0 {
            ls[j] = i;
        }
    }
    let f: Option<Vec<f64>> = mo.iter().map(|m| rf.mo.binary_search(m).ok().map(|k| rf.rf[k])).collect();
    let f = f.ok_or("T-bill series does not cover every panel month")?;
    Ok((P::new(mo, ids, pct, r, me, f, Some(ls), None)?, n))
}

/// Columns with a market value at the last month of p and >= m returns in its last M months,
/// not delisted, the nc largest by that market value.
pub fn elig(p: &History, mw: usize, m: usize, nc: usize) -> Result<Vec<usize>, String> {
    if mw.min(m).min(nc) < 1 || m > mw {
        return Err("eligibility needs positive sizes and m <= M".into());
    }
    let t = p.r.nrows() - 1;
    if t + 1 < mw {
        return Ok(vec![]);
    }
    let v = p.me.row(t);
    let mut u: Vec<usize> = (0..p.r.ncols())
        .filter(|&i| {
            let n = (t + 1 - mw..=t).filter(|&s| p.r[(s, i)].is_finite()).count();
            v[i].is_finite() && v[i] > 0.0 && n >= m && p.ls[i] > t
        })
        .collect();
    u.sort_by(|&a, &b| v[b].total_cmp(&v[a]));
    u.truncate(nc);
    Ok(u)
}

/// B2: 1/N over the eligible universe (DeMiguel et al., 2009).
pub fn b2(u: &[usize]) -> (Vec<usize>, Vec<f64>) {
    (u.to_vec(), vec![1.0 / u.len() as f64; u.len()])
}

/// B3: the K largest eligible names, cap-weighted.
pub fn b3(p: &History, u: &[usize], k: usize) -> (Vec<usize>, Vec<f64>) {
    let c = u[..k].to_vec();
    let t = p.me.nrows() - 1;
    let v: Vec<f64> = c.iter().map(|&i| p.me[(t, i)]).collect();
    let s: f64 = v.iter().sum();
    (c, v.iter().map(|x| x / s).collect())
}

/// B4 (extension of Bessembinder's bootstrap): K eligible names at random, equal-weighted.
pub fn b4(u: &[usize], k: usize, g: &mut impl Rng) -> (Vec<usize>, Vec<f64>) {
    let c = rand::seq::index::sample(g, u.len(), k).into_iter().map(|i| u[i]).collect();
    (c, vec![1.0 / k as f64; k])
}

/// What a selection rule returns: columns of the cut panel with weights, or no weights (an
/// unavailable selection), its certificate and the metadata recorded with the date.
pub struct Choice {
    pub cols: Vec<usize>,
    pub w: Option<Vec<f64>>,
    pub c: Option<C>,
    pub meta: Map<String, Value>,
}

/// One rebalance date of a run.
pub struct Q {
    pub t: usize,
    pub mo: i64,
    pub cols: Vec<usize>,
    pub pc: Vec<i64>,
    pub w: Vec<f64>,
    pub wp: Vec<f64>,
    pub eligible: usize,
    pub status: String,
    pub reason: Option<String>,
    pub c: Option<C>,
    pub meta: Map<String, Value>,
}

/// Select on p0.upto(t), hold on p0, and retain unavailable or failed dates.
pub fn run(
    p0: &P,
    mut sel: impl FnMut(&History, &[usize]) -> Result<Choice, String>,
    dates: impl IntoIterator<Item = usize>,
    mw: usize,
    m: usize,
    nc: usize,
    h: usize,
) -> Result<Vec<Q>, String> {
    let mut out = Vec::new();
    for t in dates {
        p0.horizon(t, h)?;
        let pt = p0.upto(t);
        let u = elig(&pt, mw, m, nc)?;
        let mut q = Q {
            t,
            mo: p0.mo[t],
            cols: vec![],
            pc: vec![],
            w: vec![],
            wp: vec![],
            eligible: u.len(),
            status: "unavailable".into(),
            reason: None,
            c: None,
            meta: Map::new(),
        };
        if u.is_empty() {
            q.reason = Some("empty eligible universe".into());
            out.push(q);
            continue;
        }
        match sel(&pt, &u) {
            Err(e) => q.reason = Some(e),
            Ok(ch) => {
                let inconsistent = ch.c.as_ref().is_some_and(|c| {
                    if ch.w.is_some() {
                        !matches!(c.st, crate::crt::St::Opt | crate::crt::St::Limit)
                            || !c.f.is_finite()
                            || crate::crt::crossed(c.sn, c.f, c.bd, crate::crt::TOL)
                    } else {
                        c.f.is_finite() || c.st == crate::crt::St::Opt
                    }
                });
                if inconsistent {
                    q.reason = Some("selection weights and incumbent certificate disagree".into());
                    out.push(q);
                    continue;
                }
                if ch.cols.iter().any(|i| !u.contains(i))
                    || ch.cols.iter().collect::<std::collections::BTreeSet<_>>().len() != ch.cols.len()
                    || ch.w.as_ref().is_some_and(|w| w.len() != ch.cols.len())
                {
                    q.reason = Some("selection must use unique eligible columns with paired weights".into());
                    out.push(q);
                    continue;
                }
                q.meta = ch.meta;
                q.c = ch.c;
                if let Some(c) = &q.c {
                    q.status = c.st.name().into();
                }
                if let Some(w) = ch.w {
                    let orig: Vec<usize> = ch.cols.iter().map(|&i| pt.j[i]).collect();
                    match p0.hold(t, &orig, &w, h) {
                        Err(e) => {
                            q.status = "unavailable".into();
                            q.reason = Some(e);
                        }
                        Ok(wp) => {
                            q.pc = ch.cols.iter().map(|&i| pt.pc[i]).collect();
                            q.cols = orig;
                            q.w = w;
                            q.wp = wp;
                            if q.c.is_none() {
                                q.status = "ok".into();
                            }
                        }
                    }
                } else {
                    q.reason = Some("selection has no incumbent weights".into());
                }
            }
        }
        out.push(q);
    }
    Ok(out)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn a_full_panel_cannot_remap_selected_asset_identity() {
        for j in [vec![1, 0, 2], vec![0, 1, 99]] {
            let p = small();
            assert!(P::new(p.mo, p.ids, p.pct, p.r, p.me, p.rf, Some(p.ls), Some(j)).is_err());
        }
    }

    #[test]
    fn weights_without_a_certified_incumbent_are_unavailable() {
        let p = small();
        let o = run(
            &p,
            |_, _| {
                Ok(Choice {
                    cols: vec![0],
                    w: Some(vec![1.0]),
                    c: Some(C::none(crate::crt::Sense::Min, "no solve")),
                    meta: Map::new(),
                })
            },
            [0],
            1,
            1,
            3,
            1,
        )
        .unwrap();
        assert_eq!(o[0].status, "unavailable");
        assert!(o[0].reason.is_some());
        assert!(o[0].wp.is_empty());
        assert!(o[0].cols.is_empty());
    }

    /// Four months, three stocks: stock 1 delists in month 2, stock 2 enters in month 1.
    pub fn small() -> P {
        let n = f64::NAN;
        let r = DMatrix::from_row_slice(4, 3, &[0.1, 0.0, n, 0.02, -0.5, 0.05, 0.01, n, 0.03, -0.02, n, 0.04]);
        let me = DMatrix::from_row_slice(4, 3, &[10.0, 5.0, n, 11.0, 2.0, 3.0, 12.0, n, 4.0, 13.0, n, 5.0]);
        let mut pct = DMatrix::from_row_slice(4, 3, &[1, 2, 0, 1, 2, 3, 1, 0, 3, 1, 0, 3]);
        pct[(3, 0)] = 1;
        P::new(
            vec![199001, 199002, 199003, 199004],
            vec![11, 12, 13],
            pct,
            r,
            me,
            vec![0.01; 4],
            Some(vec![4, 1, 4]),
            None,
        )
        .unwrap()
    }

    #[test]
    fn hold_moves_delisted_proceeds_to_cash() {
        let p = small();
        let w = p.hold(0, &[0, 1], &[0.5, 0.5], 2).unwrap();
        assert!((w[0] - (0.5 * 1.02 + 0.5 * 0.5)).abs() < 1e-15);
        assert!((w[1] - (0.5 * 1.02 * 1.01 + 0.25 * 1.01)).abs() < 1e-15);
    }

    #[test]
    fn missing_return_in_the_delisting_month_is_a_total_loss() {
        let mut p = small();
        p.r[(1, 1)] = f64::NAN;
        let w = p.hold(0, &[0, 1], &[0.5, 0.5], 2).unwrap();
        assert!((w[0] - 0.5 * 1.02).abs() < 1e-15);
        assert!((w[1] - 0.5 * 1.02 * 1.01).abs() < 1e-15);
    }

    #[test]
    fn missing_return_while_listed_is_refused() {
        let mut p = small();
        p.ls[1] = 4;
        assert!(p.hold(0, &[1], &[1.0], 2).is_err());
        assert!(p.hold(0, &[0], &[1.0], 4).is_err(), "incomplete horizon");
    }

    #[test]
    fn cutoff_hides_future_entrant_and_delisted_name_cannot_be_bought() {
        let n = f64::NAN;
        let r = DMatrix::from_row_slice(3, 2, &[0.1, n, 0.1, n, 0.1, 0.2]);
        let me = DMatrix::from_row_slice(3, 2, &[1.0, n, 1.0, n, 1.0, 2.0]);
        let p = P::new(vec![199001, 199002, 199003], vec![1, 2], DMatrix::zeros(3, 2), r, me, vec![0.0; 3], None, None)
            .unwrap();
        assert_eq!(p.upto(1).ids, [1]);
        // stock 1 delisted in month 1; a later price and return do not make it eligible
        let mut p = small();
        p.r[(2, 1)] = 0.0;
        p.me[(2, 1)] = 99.0;
        assert_eq!(elig(&p.upto(2), 2, 1, 5).unwrap(), [0, 2]);
    }

    #[test]
    fn no_look_ahead() {
        let p = small();
        let mut q = p.clone();
        for s in 2..4 {
            for i in 0..3 {
                if q.r[(s, i)].is_finite() {
                    q.r[(s, i)] += 0.3;
                }
                q.me[(s, i)] *= 7.0;
            }
        }
        let a = p.upto(1);
        let b = q.upto(1);
        assert_eq!(
            a.r.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            b.r.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(elig(&a, 2, 1, 3).unwrap(), elig(&b, 2, 1, 3).unwrap());
    }

    #[test]
    fn missing_calendar_month_is_rejected() {
        let r = DMatrix::from_element(2, 1, 0.1);
        assert!(
            P::new(vec![199001, 199003], vec![1], DMatrix::zeros(2, 1), r.clone(), r, vec![0.0; 2], None, None)
                .is_err()
        );
    }

    #[test]
    fn missing_selection_dates_remain_in_the_sample() {
        let p = small();
        let o = run(&p, |_, _| Err("solver failed".into()), [0, 1], 1, 1, 3, 2).unwrap();
        assert_eq!(o.len(), 2);
        assert!(o.iter().all(|q| q.status == "unavailable" && q.reason.is_some()));
        let o = run(
            &p,
            |_, u| Ok(Choice { cols: u[..1].to_vec(), w: Some(vec![1.0]), c: None, meta: Map::new() }),
            [0],
            1,
            1,
            3,
            2,
        )
        .unwrap();
        assert_eq!((o[0].status.as_str(), o[0].cols.clone(), o[0].pc.clone()), ("ok", vec![0], vec![1]));
    }

    #[test]
    fn invalid_selection_never_indexes_the_panel() {
        let p = small();
        for cols in [vec![999], vec![0, 0], vec![2]] {
            let o = run(
                &p,
                |_, _| Ok(Choice { cols: cols.clone(), w: Some(vec![1.0; cols.len()]), c: None, meta: Map::new() }),
                [0],
                1,
                1,
                3,
                1,
            )
            .unwrap();
            assert_eq!(o[0].status, "unavailable");
            assert!(o[0].reason.as_ref().unwrap().contains("unique eligible columns"));
        }
        assert!(p.hold(0, &[999], &[1.0], 1).is_err());
        assert!(p.hold(0, &[0, 0], &[0.5, 0.5], 1).is_err());
    }

    #[test]
    fn cutoff_columns_map_back_to_the_holding_panel() {
        let p = small();
        let q = p.upto(0);
        assert_eq!(q.j, [0, 1]);
        let o = run(
            &p,
            |_, u| Ok(Choice { cols: vec![u[1]], w: Some(vec![1.0]), c: None, meta: Map::new() }),
            [2],
            1,
            1,
            3,
            1,
        )
        .unwrap();
        assert_eq!(o[0].cols, [2]);
        assert!((o[0].wp[0] - 1.04).abs() < 1e-15);
    }
}
