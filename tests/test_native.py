"""Exact holding parity, with delisting, total loss, zero and tiny negative weights."""

import os

import numpy as np
import pytest

import run_oos
from pf import bd, bt, native

# Legend: P = synthetic panel; w = initial weights; a,b = oracle and native wealth paths.


@pytest.mark.parametrize("N", [1, 7, 8, 9, 31, 65, 100, 129, 257])
@pytest.mark.parametrize("seed", [0, 26, 71])
def test_exact_hold(N, seed):
    if not os.environ.get("PF_NATIVE"):
        pytest.skip("PF_NATIVE not configured")
    g = np.random.default_rng(seed)
    R = g.normal(0.005, 0.06, (144, N))
    ls = g.integers(10, 150, N)
    for k in range(N):
        R[ls[k] + 1:, k] = np.nan
    R[3, 0] = -1.0
    rf = g.uniform(-0.002, 0.01, 144)
    rf[27] = -1.0
    mo = 200001 + np.arange(144) // 12 * 100 + np.arange(144) % 12
    P = bt.P(mo, np.arange(N), np.arange(N), R, np.ones_like(R), rf, ls=ls)
    w = g.dirichlet(np.ones(N))
    if N > 2:
        w[1], w[2] = 0, -1e-13
        w /= w.sum()
    # Reverse columns to expose accidental reassociation or changes in cash summation order.
    cols = np.arange(N)[::-1]
    for H in (1, 12, 120):
        a = bt.hold(P, 0, cols, w, H)
        b = bt.hold(P, 0, cols, w, H, backend="rust")
        assert a.tobytes() == b.tobytes()
        bad = b.copy()
        bad[-1] = np.nextafter(bad[-1], np.inf)
        assert a.tobytes() != bad.tobytes()


def test_missing_and_source_mismatch():
    if not os.environ.get("PF_NATIVE"):
        pytest.skip("PF_NATIVE not configured")
    with pytest.raises(ValueError, match="still listed"):
        native.hold([[np.nan]], [0.01], [1], [1.0])
    with pytest.raises(RuntimeError, match="source mismatch"):
        native._load(os.environ["PF_NATIVE"], b"known-bad-source")


def test_backend_requires_explicit_library(monkeypatch):
    monkeypatch.delenv("PF_NATIVE", raising=False)
    with pytest.raises(RuntimeError, match="PF_NATIVE"):
        native.library()


def test_native_shape_checked_before_ffi():
    with pytest.raises(ValueError, match="shape"):
        native.hold([[0.1, 0.2]], [0.0], [1], [1.0])


def test_unaligned_input_is_copied_before_ffi():
    if not os.environ.get("PF_NATIVE"):
        pytest.skip("PF_NATIVE not configured")
    R = np.ndarray((1, 1), dtype=np.float64, buffer=bytearray(9), offset=1)
    R[0, 0] = 0.1
    assert not R.flags.aligned
    assert native.hold(R, [0.0], [1], [1.0]).tolist() == [1.1]


def test_rust_sources_enter_bundle_digest(tmp_path):
    (tmp_path / "native/src").mkdir(parents=True)
    p = tmp_path / "native/src/lib.rs"
    p.write_text("first\n", encoding="utf-8")
    a = bd.source(tmp_path)
    p.write_text("second\n", encoding="utf-8")
    assert bd.source(tmp_path) != a


def test_engine_routes_backend_and_preserves_selection():
    if not os.environ.get("PF_NATIVE"):
        pytest.skip("PF_NATIVE not configured")
    R = np.array([[0.01, 0.02], [0.03, -0.01], [0.02, 0.04], [-0.02, 0.01]])
    P = bt.P(np.arange(200001, 200005), [1, 2], [1, 2], R,
             np.ones_like(R), np.full(4, 0.002))
    a = bt.run(P, bt.b2, [1], 2, 2, 2, 2)[0]
    b = bt.run(P, bt.b2, [1], 2, 2, 2, 2, backend="rust")[0]
    assert a["status"] == b["status"] == "ok"
    for k in ("cols", "pc", "w", "W"):
        assert a[k].tobytes() == b[k].tobytes()


def test_oos_cli_records_native_identity(tmp_path, monkeypatch):
    if not os.environ.get("PF_NATIVE"):
        pytest.skip("PF_NATIVE not configured")
    p, rf, out = tmp_path / "hand.csv", tmp_path / "rf.csv", tmp_path / "bundles"
    p.write_text("PERMNO,PERMCO,date,SHRCD,EXCHCD,PRC,SHROUT,RET,DLSTCD,DLRET\n"
                 "1,1,20000131,10,1,10,100,.1,,\n"
                 "1,1,20000229,10,1,11,100,.1,,\n"
                 "1,1,20000331,10,1,12,100,.1,,\n"
                 "1,1,20000430,10,1,13,100,.1,,\n", encoding="utf-8")
    rf.write_text(",Mkt-RF,SMB,HML,RF\n200001,0,0,0,0\n200002,0,0,0,0\n"
                  "200003,0,0,0,0\n200004,0,0,0,0\n\n", encoding="utf-8")
    monkeypatch.setattr(run_oos, "OUT", out)
    monkeypatch.setattr(bd, "identity", lambda: {"dirty": False, "source": "fixture",
                                                "commit": "0" * 40})
    args = ["--crsp", str(p), "--rf", str(rf), "--only", "b2", "--K", "1",
            "--M", "2", "--m", "2", "--Nc", "2", "--H", "1"]
    run_oos.main(args)
    run_oos.main([*args, "--backend", "rust"])
    key = "oos-b2-K1-M2-m2-Nc2-H1-nofill"
    a, b = bd.read(out / f"{key}.json"), bd.read(out / f"{key}-rust.json")
    assert a["W"] == b["W"] == [1.1, 1.1]
    assert a["selection"] == b["selection"]
    assert b["bundle"]["pa"]["backend"] == "rust"
    assert b["bundle"]["inputs"]["native"] == native.identity()
