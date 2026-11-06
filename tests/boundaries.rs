//! Public-boundary counterexamples from the architecture review.
use nalgebra::{DMatrix, DVector};
use pf::{
    crt::Sense,
    cv, it,
    lp::Lp,
    mad, mv, scip, sk,
    x::{Sn, Xk},
};

#[test]
fn every_formulation_rejects_a_different_asset_universe_before_solving() {
    let x = Xk::new(3, 1, 0.0, 1.0, Sn::Le).unwrap();
    let r = DMatrix::from_row_slice(3, 2, &[0.01, 0.02, -0.01, 0.03, 0.02, -0.01]);
    let s = DMatrix::identity(2, 2);
    assert!(mv::f1(&[0.0, 0.0], &s, x, 1.0, None, false, None, 0.0).is_err());
    assert!(mad::mad(&r, x, None, None, 0.0).is_err());
    assert!(cv::cv(&r, x, 0.95, None, None, 0.0).is_err());
    assert!(sk::f5(&r, x, 3.0, 12.0, None, None, 0.0).is_err());
    assert!(it::it(&r, &DVector::from_vec(vec![0.01, -0.01, 0.02]), x, 1e-9, None, 0.0).is_err());
}

#[test]
fn integer_ir_is_solved_faithfully_or_rejected() {
    let mut p = Lp::default();
    p.col(1.0, 0.0, 2.0, true);
    p.row(vec![(0, 1.0)], 0.5, f64::INFINITY);
    let (x, _) = p.solve(None, 0.0).unwrap();
    assert_eq!(x.unwrap(), [1.0]);
    let e = scip::open(&p, Sense::Min, 1e-9, 0.0).err().unwrap();
    assert!(e.contains("binary integer columns only"));
}
