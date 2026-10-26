"""Optional external research checks; no external library enters empirical selection."""

import numpy as np

from pf import rw

# Legend: mu,S = per-period means and covariance; rho = exact target mean;
# w = continuous weights; W = evaluated wealth starting after initial wealth one;
# R = simple returns; p = explicit exported price field; provenance = export metadata.


def frontier(mu, S, rho=None):
    """PyPortfolioOpt long-only continuous minimum variance, optionally at an exact mean."""
    from pypfopt import EfficientFrontier, objective_functions

    mu, S = rw.ms(mu, S)
    if rho is not None and (not np.isfinite(rho) or not mu.min() <= rho <= mu.max()):
        raise ValueError("target return is outside the continuous feasible range")
    ef = EfficientFrontier(mu, S, weight_bounds=(0, 1), solver="CLARABEL",
                           solver_options={"tol_gap_abs": 1e-11, "tol_feas": 1e-11,
                                           "tol_gap_rel": 1e-11})
    if rho is None:
        ef.min_volatility()
    else:
        ef.add_constraint(lambda w: mu @ w == rho)
        ef.convex_objective(objective_functions.portfolio_variance, cov_matrix=S)
    # Keep the library's uncleaned weights; clean_weights clips and rounds further.
    w = np.asarray(ef.weights).copy()
    if (not np.isfinite(w).all() or abs(w.sum() - 1) > 1e-7 or w.min() < -1e-7
            or (rho is not None and abs(mu @ w - rho) > 1e-7)):
        raise RuntimeError("external continuous weights violate the requested constraints")
    return {"w": w, "f": float(w @ S @ w), "mean": float(mu @ w),
            "status": str(ef._opt.status), "solver": "CLARABEL", "cardinality": "continuous"}


def returns(W):
    """Simple returns from a nonnegative evaluated path, including initial wealth one."""
    W = np.asarray(W, float)
    if W.ndim != 1 or W.size == 0 or not np.isfinite(W).all() or np.any(W < 0):
        raise ValueError("wealth must be a nonempty finite nonnegative path")
    a = np.r_[1.0, W[:-1]]
    if np.any((a == 0) & (W != 0)):
        raise ValueError("wealth cannot recover after complete loss without new capital")
    R = np.zeros_like(W)
    np.divide(W, a, out=R, where=a != 0)
    R[a != 0] -= 1
    return R


def vectorbt(W):
    """Independent vectorbt cumulative-return and drawdown check on an evaluated path."""
    import pandas as pd
    from vectorbt.returns.accessors import ReturnsAccessor

    R = returns(W)
    a = ReturnsAccessor(pd.Series(R))
    return {"wealth": np.asarray(a.cumulative(start_value=1.0)),
            "total_return": float(a.total()), "max_drawdown": float(a.max_drawdown())}


def openbb(frame, *, price, provider, retrieved, adjustment):
    """Read an OpenBB historical export with explicit provider and adjustment provenance."""
    import pandas as pd

    if not all(isinstance(x, str) and x.strip() for x in (price, provider, retrieved, adjustment)):
        raise ValueError("price, provider, retrieval date and adjustment convention are required")
    f = frame.copy()
    if "date" not in f.columns and f.index.name == "date":
        f = f.reset_index()
    if not {"date", "symbol", price} <= set(f.columns) or f.empty:
        raise ValueError("OpenBB export needs date, symbol and the explicit price field")
    f["date"] = pd.to_datetime(f["date"], errors="raise")
    if f[["date", "symbol"]].isna().any().any() or f.duplicated(["date", "symbol"]).any():
        raise ValueError("OpenBB dates and symbols must be nonmissing and unique")
    p = f.pivot(index="date", columns="symbol", values=price).sort_index()
    v = p.to_numpy(dtype=float)
    if np.isinf(v).any() or np.any(v[np.isfinite(v)] <= 0):
        raise ValueError("exported prices must be positive or missing")
    # Missing quotes remain missing; neither forward filling nor resampling is implicit.
    R = p.pct_change(fill_method=None).iloc[1:]
    if len(R) == 0 or not np.isfinite(R.to_numpy(dtype=float)).any():
        raise ValueError("OpenBB export needs at least one observed price-to-price return")
    return {"returns": R, "provenance": {"provider": provider, "retrieved": retrieved,
            "price": price, "adjustment": adjustment, "scope": "public exploratory prices",
            "delisting_complete": False}}
