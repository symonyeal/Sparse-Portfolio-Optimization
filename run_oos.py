"""RQ2-RQ4: out-of-sample selection on CRSP under the pre-registered plan (docs/PLAN.md).

    python run_oos.py --crsp PATH [--rf PATH] [--only STRATEGY] [--K 10] [--M 120] [--H 120]
                      [--Nc 200] [--m 120] [--fill] [--time 60] [--missing]

Needs a licensed CRSP extract; without one it stops and P6 stays blocked. For each rebalance
date the selection sees P.upto(t) only; the holding is evaluated on the full panel. One bundle
per (strategy, K, M, m, Nc, H, fill) holds terminal wealths and held PERMCOs per date, never
row-level data. The set T of top wealth creators is computed afterwards by pf/wc.py and used
only in pf/ev.py.
"""

import argparse
import sys
from pathlib import Path

import numpy as np

from pf import bd, bt, cv, dat, mad, mv, sk

# Legend
# P0 : panel; Pt : panel cut at t; U : eligible columns; sel : selection rule
# K, M, m, Nc, H : names, window, required returns, universe size, holding months
# tl : per-solve time limit; lam : Chang's weight for F1 (docs/PLAN.md)

OUT = Path("docs/results/oos")
LAM = 0.5
GAM, KAP = 3.0, 12.0
ST = ("b2", "b3", "b4", "f1", "f2", "f3", "f5")


def window(Pt, U, M, m=None):
    """Common observed scenarios within M months, with at least m rows."""
    m = M if m is None else m
    if not 1 <= m <= M:
        raise ValueError("history threshold must be between 1 and M")
    y = Pt.R[-M:][:, U]
    y = y[np.isfinite(y).all(axis=1)]
    if U.size == 0 or y.shape[0] < m:
        raise ValueError("not enough common observed scenarios for the history threshold")
    return U, y


def rule(s, K, M, tl, g, m=None):
    if s not in ST:
        raise ValueError(f"unknown strategy {s!r}")
    def sel(Pt, U):
        if s == "b2":
            return bt.b2(Pt, U)
        if U.size < K:
            raise ValueError("eligible universe has fewer than K names")
        if s == "b3":
            return bt.b3(Pt, U, K)
        if s == "b4":
            return bt.b4(Pt, U, K, g)
        c, y = window(Pt, U, M, m)
        rho = float(np.median(y.mean(axis=0)))
        if s in ("f1", "f5"):
            S, shrink = mv.shrink(y)
            x = (mv.f1(y.mean(axis=0), S, K, LAM, tl=tl) if s == "f1"
                 else sk.f5(y, K, GAM, k=KAP, Sg=S, tl=tl))
        elif s == "f2":
            x = mad.mad(y, K, rho=rho, tl=tl)
        else:
            x = cv.cv(y, K, b=0.95, rho=rho, tl=tl)
        q = {"cols": c, "w": None, "c": x["c"], "status": x["c"]["st"],
             "scenarios": int(y.shape[0]), "rho": rho if s in ("f2", "f3") else None}
        if s in ("f1", "f5"):
            q["shrink"] = float(shrink)
        if x["w"] is None:
            return q
        w = np.clip(x["w"], 0, None)
        q["w"] = w / w.sum()
        return q
    return sel


def main(argv=None):
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument("--crsp")
    a.add_argument("--rf")
    a.add_argument("--only", choices=ST, action="append", default=[])
    a.add_argument("--K", type=int, default=10)
    a.add_argument("--M", type=int, default=120)
    a.add_argument("--m", type=int, default=120)
    a.add_argument("--Nc", type=int, default=200)
    a.add_argument("--H", type=int, default=120)
    a.add_argument("--fill", action="store_true")
    a.add_argument("--time", type=float, default=60.0)
    a.add_argument("--seed", type=int, default=0)
    a.add_argument("--missing", action="store_true")
    a.add_argument("--backend", choices=("python", "rust"), default="python")
    x = a.parse_args(argv)
    if not x.crsp:
        raise SystemExit("no CRSP extract supplied (--crsp): P6 is blocked; see docs/PLAN.md")
    if (min(x.K, x.M, x.m, x.Nc, x.H) < 1 or x.m > x.M or x.K > x.Nc
            or not np.isfinite(x.time) or x.time <= 0 or x.seed < 0):
        a.error("positive sizes/time, m <= M, K <= Nc and a nonnegative seed are required")
    code = bd.identity()
    if code["dirty"]:
        raise SystemExit("refusing to write bundles from a dirty tree; commit first")
    rows = dat.crsp(x.crsp)
    rfd = dat.rf(x.rf)
    P0, nf = bt.panel(rows, rfd, fill=x.fill)
    ip = {"crsp": rows["sha"], "rf": rfd["sha"], "rf_vintage": rfd["vintage"],
          "plan": dat.sha256(Path(__file__).parent / "docs/PLAN.md")}
    if x.backend == "rust":
        from pf import native
        ip["native"] = native.identity()
    dates = range(x.M - 1, P0.R.shape[0] - x.H)
    for s in x.only or ST:
        key = f"oos-{s}-K{x.K}-M{x.M}-m{x.m}-Nc{x.Nc}-H{x.H}-{'fill' if x.fill else 'nofill'}"
        pa = {"s": s, "K": x.K, "M": x.M, "m": x.m, "Nc": x.Nc, "H": x.H, "fill": x.fill,
              "tl": x.time, "seed": x.seed}
        if x.backend != "python":
            pa["backend"] = x.backend
            key += "-rust"
        if s == "f5":
            pa.update(g=GAM, k=KAP)
        if x.missing and bd.valid(OUT, key, pa, code["source"], ip):
            continue
        g = np.random.default_rng(x.seed)
        o = bt.run(P0, rule(s, x.K, x.M, x.time, g, x.m), dates, x.M, x.m, x.Nc, x.H,
                   backend=x.backend)
        market, errors = [], []
        for q in o:
            try:
                market.append(float(bt.b1(P0, q["t"], x.H)[-1]))
                errors.append(None)
            except ValueError as e:
                market.append(None)
                errors.append(str(e))
        r = {"mo": [q["mo"] for q in o],
             "W": [float(q["W"][-1]) if q["W"].size else None for q in o],
             "pc": [q["pc"][q["w"] > 0].tolist() for q in o],
             "selection": [{k: q[k] for k in ("status", "eligible", "scenarios", "rho", "shrink",
                                              "c", "reason")
                            if k in q} for q in o],
             "B1": market, "B1_errors": errors,
             "B5": [float(bt.b5(P0, q["t"], x.H)[-1]) for q in o], "fills": nf}
        cmd = sys.argv if argv is None else [str(Path(__file__).name), *argv]
        bd.write(OUT, bd.bundle(key, pa, r, ip, cmd, code))
        print(key, len(o), "dates", flush=True)


if __name__ == "__main__":
    main()
