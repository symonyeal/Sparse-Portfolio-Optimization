"""Offline public-data checks against optional PyPortfolioOpt and vectorbt installations."""

import argparse
from importlib.metadata import version
import json

import numpy as np

from pf import bd, dat, ext, mv

# Legend: d = public OR-Library data; rho = exact target mean; W = equal-weight
# buy-and-hold wealth on survivor-only indtrack1; a,b = external and independent results.


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--only", choices=("frontier", "vectorbt"), action="append", default=[])
    x = p.parse_args(argv)
    out = {"scope": "public survivor-only data; computational checks only", "versions": {}}
    for check in x.only or ("frontier", "vectorbt"):
        if check == "frontier":
            out["versions"].update({p: version(p) for p in ("PyPortfolioOpt", "cvxpy", "clarabel")})
            d = dat.port(1)
            rho = float(np.median(d["mu"]))
            a = ext.frontier(d["mu"], d["S"], rho)
            b = mv.qp(d["mu"], d["S"], rho, np.ones(d["N"]))
            if not np.isclose(a["f"], b["f"], rtol=1e-7, atol=1e-10):
                raise RuntimeError("PyPortfolioOpt variance disagrees with the KKT frontier")
            out[check] = {"external": a, "kkt_variance": b["f"], "kkt_residuals": b["kt"],
                          "target_mean": rho, "input_sha256": d["sha"]}
        else:
            out["versions"]["vectorbt"] = version("vectorbt")
            d = dat.indtrack(1)
            W = np.mean(d["V"][:, 1:] / d["V"][:, :1], axis=0)
            a = ext.vectorbt(W)
            if not np.allclose(a["wealth"], W, rtol=1e-12, atol=1e-12):
                raise RuntimeError("vectorbt wealth disagrees with buy-and-hold accounts")
            out[check] = {"total_return": a["total_return"],
                          "max_drawdown": a["max_drawdown"], "periods": int(W.size),
                          "max_wealth_error": float(np.max(np.abs(a["wealth"] - W))),
                          "input_sha256": d["sha"]}
    print(json.dumps(bd.plain(out), indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
