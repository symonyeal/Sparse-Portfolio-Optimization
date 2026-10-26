"""The monthly panel and the out-of-sample engine: eligibility, selection at t, buy-and-hold.

Selection at t receives `P.upto(t)`, a copy holding only months <= t, so no weight can depend on
a later return (the no-look-ahead test perturbs every later return). Evaluation then holds the
weights on the full panel. Protocol: specification sec. 8; DeMiguel, Garlappi and Uppal (2009)
for the rolling estimation window M.

Holding: buy-and-hold for H months. After a recorded delisting month (whose return includes
DLRET), proceeds earn the T-bill rate. Other missing holding returns are refused.
"""

import numpy as np

from pf import dat, wc

# Legend
# mo      : months YYYYMM (Tm,); ids : PERMNO (N,); pc : PERMCO (N,)
# R       : combined returns (Tm, N), nan where the stock has no return that month
# ME      : end-of-month market value |PRC| SHROUT (Tm, N), nan where unknown
# rf      : T-bill return per month (Tm,)
# ls      : explicit delisting-month index per stock; Tm means no observed delisting
# PC, j   : monthly PERMCO labels and original column indices
# t       : month index of a rebalance date; M : estimation window; m : required returns in it
# Nc      : eligible universe size, top Nc by ME at t; U : eligible column indices
# K       : names held; w : weights over U (or over all columns); H : holding months
# Wp      : wealth path of a holding, starting at 1 at the end of month t


class P:
    """A wide monthly panel; `upto` cuts it at a month."""

    def __init__(self, mo, ids, pc, R, ME, rf, ls=None, j=None):
        self.mo, self.ids = np.asarray(mo), np.asarray(ids)
        self.R, self.ME = np.asarray(R, float), np.asarray(ME, float)
        self.rf = np.asarray(rf, float)
        if self.R.ndim != 2:
            raise ValueError("panel returns must be a matrix")
        Tm, N = self.R.shape
        if (Tm == 0 or self.ME.shape != (Tm, N) or self.rf.shape != (Tm,)
                or self.mo.shape != (Tm,) or self.ids.shape != (N,)):
            raise ValueError("panel arrays disagree in shape")
        if (not np.issubdtype(self.mo.dtype, np.integer)
                or np.any((self.mo % 100 < 1) | (self.mo % 100 > 12))
                or not np.array_equal(self.mo[1:], [dat.nx(m) for m in self.mo[:-1]])):
            raise ValueError("panel months must be valid and consecutive")
        if not np.isfinite(self.rf).all() or np.any(self.rf < -1):
            raise ValueError("T-bill return missing in a panel month")
        if np.isinf(self.R).any() or np.any(self.R[~np.isnan(self.R)] < -1):
            raise ValueError("panel return must be finite or missing and at least -1")
        pc = np.asarray(pc)
        self.PC = np.tile(pc, (Tm, 1)) if pc.shape == (N,) else pc.copy()
        if self.PC.shape != (Tm, N):
            raise ValueError("PERMCO labels disagree in shape")
        # An absent monthly label is carried from prior observations only.
        self.pc = np.zeros(N, dtype=self.PC.dtype)
        for row in self.PC:
            k = row != 0
            self.pc[k] = row[k]
        self.ls = np.full(N, Tm, dtype=int) if ls is None else np.asarray(ls)
        self.j = np.arange(N) if j is None else np.asarray(j)
        if self.ls.shape != (N,) or self.j.shape != (N,):
            raise ValueError("panel column metadata disagree in shape")

    def upto(self, t):
        """Observed columns, company labels and delistings through month t only."""
        if not 0 <= t < self.R.shape[0]:
            raise ValueError("cutoff is outside the panel")
        a = slice(None, t + 1)
        k = (np.isfinite(self.R[a]) | np.isfinite(self.ME[a])).any(axis=0)
        ls = np.minimum(self.ls[k], t + 1)
        return P(self.mo[a], self.ids[k], self.PC[a, k], self.R[a, k].copy(),
                 self.ME[a, k].copy(), self.rf[a].copy(), ls=ls, j=self.j[k])


def panel(rows, rfd, fill=False):
    """A panel from pf.dat.crsp rows and pf.dat.rf; returns (P, number of Shumway fills)."""
    mo = np.unique(rows["mo"])
    ids, j = np.unique(rows["PERMNO"], return_inverse=True)
    i = np.searchsorted(mo, rows["mo"])
    R0, n = wc.comb(rows["RET"], rows["DLRET"], rows["dl"], rows["EXCHCD"], fill)
    R = np.full((mo.size, ids.size), np.nan)
    ME = np.full((mo.size, ids.size), np.nan)
    R[i, j] = R0
    ME[i, j] = np.abs(rows["PRC"]) * rows["SHROUT"]
    pc = np.zeros(R.shape, dtype=np.int64)
    pc[i, j] = rows["PERMCO"]
    ls = np.full(ids.size, mo.size, dtype=int)
    for a, b in zip(i[rows["dl"] != 0], j[rows["dl"] != 0]):
        ls[b] = a
    if rfd["mo"].size == 0:
        raise ValueError("T-bill series does not cover every panel month")
    k = np.searchsorted(rfd["mo"], mo)
    if np.any(k >= rfd["mo"].size) or np.any(rfd["mo"][np.minimum(k, rfd["mo"].size - 1)] != mo):
        raise ValueError("T-bill series does not cover every panel month")
    return P(mo, ids, pc, R, ME, rfd["rf"][k], ls=ls), n


def elig(Pt, M, m, Nc):
    """Columns with a market value at the last month of Pt and >= m returns in its last M months,
    the Nc largest by that market value."""
    if min(M, m, Nc) < 1 or m > M:
        raise ValueError("eligibility needs positive sizes and m <= M")
    t = Pt.R.shape[0] - 1
    if t + 1 < M:
        return np.array([], dtype=int)
    v = Pt.ME[t]
    n = np.isfinite(Pt.R[t + 1 - M:t + 1]).sum(axis=0)
    U = np.flatnonzero(np.isfinite(v) & (v > 0) & (n >= m) & (Pt.ls > t))
    return U[np.argsort(-v[U], kind="stable")][:Nc]


def hold(P0, t, cols, w, H, backend="python"):
    """Wealth path over months t+1..t+H of weights w on columns cols, bought at the end of t."""
    cols, w = np.asarray(cols), np.asarray(w, float)
    horizon(P0, t, H)
    if cols.ndim != 1 or cols.shape != w.shape or not np.isfinite(w).all():
        raise ValueError("holding columns and weights disagree")
    if abs(w.sum() - 1) > 1e-9 or np.any(w < -1e-12):
        raise ValueError("weights must be nonnegative and sum to one")
    if backend == "rust":
        from pf import native
        return native.hold(P0.R[t + 1:t + H + 1][:, cols], P0.rf[t + 1:t + H + 1],
                           P0.ls[cols] - t - 1, w)
    if backend != "python":
        raise ValueError("holding backend must be python or rust")
    v = w.copy()
    cash = 0.0
    Wp = []
    for s in range(t + 1, t + H + 1):
        cash *= 1 + P0.rf[s]
        r = P0.R[s, cols]
        for k in np.flatnonzero(v > 0):
            if np.isfinite(r[k]):
                v[k] *= 1 + r[k]
            elif s > P0.ls[cols[k]]:          # left the panel after its delisting month
                cash += v[k] * (1 + P0.rf[s])
                v[k] = 0.0
            else:
                raise ValueError("missing return while still listed or without a "
                                 "confirmed delisting")
        Wp.append(v.sum() + cash)
    return np.array(Wp)


def b1(P0, t, H):
    """B1: the value-weighted market of every stock with a return and a market value."""
    from pf import bs
    horizon(P0, t, H)
    r = bs.vw(P0.R[t + 1:t + H + 1], P0.ME[t:t + H])
    return np.cumprod(1 + r)


def b5(P0, t, H):
    """B5: the one-month T-bill rolled."""
    horizon(P0, t, H)
    return np.cumprod(1 + P0.rf[t + 1:t + H + 1])


def horizon(P0, t, H):
    """Refuse an incomplete holding horizon."""
    if (not isinstance(H, (int, np.integer)) or H < 1 or t < 0
            or t + H >= P0.R.shape[0]):
        raise ValueError("holding horizon is outside the panel")


def b2(Pt, U, K=None):
    """B2: 1/N over the eligible universe (DeMiguel et al., 2009)."""
    return U, np.full(U.size, 1.0 / U.size)


def b3(Pt, U, K):
    """B3: the K largest eligible names, cap-weighted."""
    c = U[:K]
    v = Pt.ME[-1, c]
    return c, v / v.sum()


def b4(Pt, U, K, g):
    """B4 (extension of Bessembinder's bootstrap): K eligible names at random, equal-weighted."""
    c = g.choice(U, size=K, replace=False)
    return c, np.full(K, 1.0 / K)


def run(P0, sel, dates, M, m, Nc, H, backend="python"):
    """Select on P0.upto(t), hold on P0, and retain unavailable or failed dates."""
    out = []
    for t in dates:
        horizon(P0, t, H)
        Pt = P0.upto(t)
        U = elig(Pt, M, m, Nc)
        q = {"t": int(t), "mo": int(P0.mo[t]), "cols": np.array([], dtype=int),
             "pc": np.array([], dtype=int), "w": np.array([]), "W": np.array([]),
             "eligible": int(U.size), "status": "unavailable"}
        if U.size == 0:
            q["reason"] = "empty eligible universe"
            out.append(q)
            continue
        try:
            chosen = sel(Pt, U)
            if isinstance(chosen, dict):
                cols, w = chosen["cols"], chosen["w"]
                q.update({k: v for k, v in chosen.items() if k not in ("cols", "w")})
                if w is None:
                    out.append(q)
                    continue
            else:
                cols, w = chosen
            cols = np.asarray(cols)
            original = Pt.j[cols]
            q.update(cols=original, pc=Pt.pc[cols], w=np.asarray(w),
                     W=hold(P0, t, original, w, H, backend=backend))
            q["status"] = "ok" if "c" not in q else q["c"]["st"]
        except (ValueError, RuntimeError) as e:
            q.update(status="unavailable", reason=str(e))
        out.append(q)
    return out
