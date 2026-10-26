"""Independent optional-library checks and strict offline public-price ingestion."""

import numpy as np
import pytest

from pf import ext, mv
import run_compare

# Legend: mu,S = synthetic means and covariance; r = target return; W = wealth path.


def test_pypfopt_exact_return_frontier():
    pytest.importorskip("pypfopt")
    mu = np.array([0.005, 0.01, 0.02])
    S = np.diag([0.002, 0.003, 0.005])
    r = 0.012
    a = ext.frontier(mu, S, r)
    b = mv.qp(mu, S, r, np.ones(3))
    assert a["status"] == "optimal"
    assert a["f"] == pytest.approx(b["f"], abs=1e-10)
    assert mu @ a["w"] == pytest.approx(r, abs=1e-10)
    a = ext.frontier(mu, S)
    w = 1 / np.diag(S)
    w /= w.sum()
    assert a["w"] == pytest.approx(w, abs=1e-6)
    with pytest.raises(ValueError, match="feasible"):
        ext.frontier(mu, S, 0.03)


def test_public_comparison_cli(capsys):
    pytest.importorskip("pypfopt")
    pytest.importorskip("vectorbt")
    import json

    run_compare.main([])
    q = json.loads(capsys.readouterr().out)
    assert q["frontier"]["external"]["status"] == "optimal"
    assert q["vectorbt"]["periods"] > 0
    assert q["vectorbt"]["max_wealth_error"] < 1e-12


@pytest.mark.parametrize("W", [[1.1, 0.8, 1.2, 1.0], [0.5, 0.0, 0.0]])
def test_vectorbt_evaluated_wealth(W):
    pytest.importorskip("vectorbt")
    a = ext.vectorbt(W)
    assert a["wealth"] == pytest.approx(W, abs=1e-14)
    assert a["total_return"] == pytest.approx(W[-1] - 1, abs=1e-14)
    peaks = np.maximum.accumulate(np.r_[1.0, W])[1:]
    assert a["max_drawdown"] == pytest.approx(np.min(np.asarray(W) / peaks - 1))


def test_wealth_rejects_unfunded_recovery():
    with pytest.raises(ValueError, match="recover"):
        ext.returns([0.0, 1.0])


def test_openbb_export_preserves_missing_prices():
    pd = pytest.importorskip("pandas")
    f = pd.DataFrame({"date": ["2020-01-01", "2020-01-02", "2020-01-03"] * 2,
                      "symbol": ["A"] * 3 + ["B"] * 3,
                      "adj_close": [10, np.nan, 12, 20, 22, 24]})
    kw = {"price": "adj_close", "provider": "fixture", "retrieved": "2026-10-26",
          "adjustment": "fixture dividend and split adjustment"}
    a = ext.openbb(f, **kw)
    assert a["returns"]["A"].isna().all()
    assert a["returns"]["B"].to_numpy() == pytest.approx([0.1, 24 / 22 - 1])
    assert not a["provenance"]["delisting_complete"]
    with pytest.raises(ValueError, match="unique"):
        ext.openbb(pd.concat([f, f.iloc[:1]]), **kw)
    with pytest.raises(ValueError, match="price field"):
        ext.openbb(f.drop(columns="adj_close"), **kw)
    with pytest.raises(ValueError, match="required"):
        ext.openbb(f, **(kw | {"adjustment": ""}))
    assert ext.openbb(f.set_index("date"), **kw)["returns"].equals(a["returns"])
