//! Gates G1 and G2, F1 = F1p, and the independent checks of F2-F5 on OR-Library data.
//!
//! G2 and the F5 lattice check compare against values pinned from routes that are no longer
//! maintained: the extended cutting-plane method (Westerlund and Pettersson, 1995; HiGHS master,
//! pf/ecp.py) and the exhaustive step-0.01 lattice over every support of size <= K (pf/sk.py,
//! `grid`), both run at revision 1ca7e76 with a 60 s limit.

use nalgebra::{DMatrix, DVector};
use pf::crt::St;
use pf::x::{Sn, Xk};
use pf::{cv, data, est, it, mad, mv, sk};

fn p1() -> (Vec<f64>, DMatrix<f64>) {
    let p = data::port(1, None).unwrap();
    (p.mu, p.s)
}

fn r1() -> (DMatrix<f64>, DVector<f64>) {
    data::indtrack(1, None, false).unwrap().lr()
}

#[test]
fn g1_frontier_certificates_and_the_published_discrepancy() {
    let (mu, s) = p1();
    let ef = data::portef(1, None).unwrap().ef;
    let (lo, hi) = mu.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &m| (a.min(m), b.max(m)));
    let h = 5e-11;
    let (mut f0, mut kt, mut miss, mut sep) = (vec![1.0; 31], 0.0f64, 0, 0.0f64);
    for &(r, v) in &ef {
        let (a, b) = ((r - h).max(lo), (r + h).min(hi));
        assert!(a <= b, "printed mean {r} has no feasible rounding interval");
        let mut fs = vec![];
        for m in [a, r.clamp(a, b), b] {
            let q = mv::qp(&mu, &s, m, &f0, 1e-12, 5 * 31).unwrap();
            f0 = q.w.iter().map(|&w| w + f64::from(u8::from(w > 0.0))).collect();
            fs.push(q.f);
            kt = q.kt.iter().copied().fold(kt, f64::max);
        }
        let (fmin, fmax) = fs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &f| (a.min(f), b.max(f)));
        if !(fmax >= v - h && fmin <= v + h) {
            miss += 1;
            sep = sep.max((fmin - v - h).max(v - h - fmax));
        }
    }
    assert!(kt < 1e-12, "KKT residual {kt}");
    // Published portef1: 142 of 2,000 printed rounding rectangles miss the certified frontier.
    assert_eq!(miss, 142);
    assert!((sep - 8.96e-11).abs() < 1e-13, "separation {sep}");
}

#[test]
fn g1_scip_route_at_k_equal_n() {
    let (mu, s) = p1();
    let ef = data::portef(1, None).unwrap().ef;
    for i in [0, 999, 1999] {
        let q = mv::qp(&mu, &s, ef[i].0, &[1.0; 31], 1e-12, 200).unwrap();
        let x = Xk::new(31, 31, 0.0, 1.0, Sn::Le).unwrap();
        let a = mv::f1(&mu, &s, x, 1.0, Some((ef[i].0, true)), false, None, 0.0).unwrap();
        assert!(a.c.st == St::Opt && a.c.bd <= q.f * (1.0 + 1e-12));
        assert!((a.f - q.f).abs() <= 1e-7 * q.f);
    }
}

#[test]
fn f1_equals_f1p_and_lies_in_x() {
    let (mu, s) = p1();
    for sn in [Sn::Le, Sn::Eq] {
        for lam in [0.5, 1.0] {
            let x = Xk::new(31, 10, 0.01, 1.0, sn).unwrap();
            let a = mv::f1(&mu, &s, x, lam, None, false, None, 0.0).unwrap();
            let b = mv::f1(&mu, &s, x, lam, None, true, None, 0.0).unwrap();
            assert!(a.c.st == St::Opt && b.c.st == St::Opt);
            assert!((a.f - b.f).abs() <= 1e-7 * a.f.abs().max(1e-12));
            for q in [&a, &b] {
                assert!(x.ok(q.w.as_ref().unwrap(), q.z.as_ref().unwrap(), 1e-7).is_empty());
            }
        }
    }
}

#[test]
fn g2_scip_against_pinned_cutting_plane_values() {
    // (lam, ECP status, ECP dual bound, ECP incumbent), port1, K = 10, e = 0.01, 1'z = K.
    let ecp = [
        (0.0, St::Opt, -0.01035858, -0.010358579999999997),
        (0.25, St::Opt, -0.006728798945411937, -0.006728798945411936),
        (0.5, St::Limit, -0.003303997063770671, -0.0033039963152746037),
        (0.75, St::Limit, -0.0009050178389085541, -0.0009050172402752844),
        (1.0, St::Limit, 0.000642256725459966, 0.0006422574222707702),
    ];
    let (mu, s) = p1();
    let x = Xk::new(31, 10, 0.01, 1.0, Sn::Eq).unwrap();
    for (lam, st, bd, f) in ecp {
        let a = mv::f1(&mu, &s, x, lam, None, false, None, 0.0).unwrap();
        assert_eq!(a.c.st, St::Opt);
        if st == St::Opt {
            assert!((a.f - f).abs() <= 1e-6 * a.f.abs(), "lam {lam}");
        }
        // SCIP's 1e-9 feasibility tolerance can carry its value just below ECP's bound.
        let tol = 1e-8 * a.f.abs();
        assert!(bd - tol <= a.f && a.f <= f + tol, "lam {lam}: {} outside [{bd}, {f}]", a.f);
    }
}

#[test]
fn f2_deviation_recomputed() {
    let (r, _) = r1();
    let mut m: Vec<f64> = r.column_iter().map(|c| c.mean()).collect();
    m.sort_by(f64::total_cmp);
    let rho = m[15];
    for sn in [Sn::Le, Sn::Eq] {
        let x = Xk::new(31, 10, 0.0, 1.0, sn).unwrap();
        let a = mad::mad(&r, x, Some(rho), None, 0.0).unwrap();
        let w = a.w.as_ref().unwrap();
        assert_eq!(a.c.st, St::Opt);
        assert!((a.c.f - a.f).abs() <= 1e-9 * a.f);
        assert!(x.ok(w, a.z.as_ref().unwrap(), 1e-7).is_empty());
        let mu: Vec<f64> = r.column_iter().map(|c| c.mean()).collect();
        assert!(mv::dot(&mu, w) >= rho - 1e-9);
    }
}

#[test]
fn f3_cvar_identity() {
    let (r, _) = r1();
    for b in [0.95, 0.9537] {
        let x = Xk::new(31, 10, 0.0, 1.0, Sn::Le).unwrap();
        let a = cv::cv(&r, x, b, None, None, 0.0).unwrap();
        assert_eq!(a.c.st, St::Opt);
        assert!((a.c.f - a.f).abs() <= 1e-9 * a.f.abs());
        assert!(x.ok(a.w.as_ref().unwrap(), a.z.as_ref().unwrap(), 1e-7).is_empty());
    }
}

#[test]
fn f4_stages_and_regression_identity() {
    let (r, ri) = r1();
    let (r1, i1) = (r.rows(0, 145).into_owned(), ri.rows(0, 145).into_owned());
    let x = Xk::new(31, 10, 0.01, 1.0, Sn::Eq).unwrap();
    let s = it::it(&r1, &i1, x, it::EA, None, 0.0).unwrap();
    assert!(s.c1.st == St::Opt && s.st == St::Opt);
    let w = s.w.as_ref().unwrap();
    let (a, b) = est::ols(&(&r1 * DVector::from_column_slice(w)), &i1).unwrap();
    assert!((a - s.a.unwrap()).abs() <= 1e-12 && (b - s.b.unwrap()).abs() <= 1e-12);
    assert!(s.a.unwrap().abs() <= 1e-9 + 1e-12);
    assert!((s.b.unwrap() - 1.0).abs() <= s.c2.as_ref().unwrap().f + 1e-12);
    let t = r.nrows() - 145;
    assert!(it::te(&r.rows(145, t).into_owned(), &ri.rows(145, t).into_owned(), w).is_finite());
}

#[test]
fn f5_dual_bound_against_pinned_lattice() {
    // Best lattice value (step 0.01, supports of size <= 2) on the first 36 scenarios of the
    // first 6 indtrack1 stocks, attained at w = e_2.
    let lattice = 0.012602516072573254;
    let (r, _) = r1();
    let y = r.view((0, 0), (36, 6)).into_owned();
    let x = Xk::new(6, 2, 0.0, 1.0, Sn::Le).unwrap();
    let a = sk::f5(&y, x, 3.0, sk::kap(3.0), None, Some(60.0), 0.0).unwrap();
    assert_eq!(a.c.st, St::Opt);
    assert!(a.c.bd >= lattice - 1e-12 && a.f >= lattice - 1e-10);
    assert!(x.ok(a.w.as_ref().unwrap(), a.z.as_ref().unwrap(), 1e-7).is_empty());
    assert_eq!(sk::kap(3.0), 12.0);
}
