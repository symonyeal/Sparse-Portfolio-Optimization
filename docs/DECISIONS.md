# Design decisions

The [working contract](../CLAUDE.md) defines implementation and evidence rules. Independent
recomputation avoids errors shared by two solvers using the same row matrix. Synthetic
fixtures name the property tested and carry no empirical interpretation.

## One Rust crate (1 November 2026)

| Decision | Reason |
| --- | --- |
| Replace the Python package, its runners and the optional Rust holding library with one crate, `pf`, and its command `pf bench, oos, boot, fetch`. | One implementation to maintain. Every retained route matched the Python results before removal (VALIDATION.md). |
| HiGHS 1.15.0 through the `highs` crate, built from source; SCIP 10.0.2 through `russcip`, from the prebuilt library that `scip-sys` downloads. | The same solvers as before, without a Python runtime. HiGHS is compiled with `/O2` (`.cargo/config.toml`), otherwise MSVC builds it unoptimised. |
| Read HiGHS's node count through `Highs_getInt64InfoValue`. | The counter is 64-bit; the `highs` crate reads it as a 32-bit value and fails. |
| Write SCIP's cubic and square terms as products ($y\cdot y\cdot y$, $w_i\cdot w_i$), as PySCIPOpt did. | With `pow(y, 3)` SCIP needed 1,228 nodes for the 35-node lattice case, and its 60 s bound on indtrack1 at $K = 3$ was eleven times weaker. |
| Drop the extended cutting-plane route and the F5 lattice; keep their values as pinned tests. | Both were independent checks run once. Their values now gate SCIP in `tests/models.rs`, with the revision that produced them. |
| Drop the optional PyPortfolioOpt, vectorbt and OpenBB checks and the Rust holding backend. | They checked arithmetic that the Rust tests now cover directly; none entered a registered result. |
| Bundles are schema 2. The source digest covers `src/`, the manifest, the lock file, the build script and the OR-Library pins, and is compiled into the binary. | A binary built from other sources counts as a dirty tree and cannot write bundles. |

## Solvers (2 October 2026; revised 1 November 2026)

| Decision | Reason |
| --- | --- |
| HiGHS for every LP and MILP. | F2–F4 are MILPs; HiGHS is open source and certifies a dual bound. |
| SCIP for F1, F1p and F5. | HiGHS has no MIQP or MINLP solver. SCIP 10 is Apache-2.0. |
| Scale the SCIP objective by $1/\overline{\operatorname{diag}\Sigma}$ and divide the certificate back; use scale one for deterministic returns. | SCIP's tolerances are absolute below one; unscaled, the recomputed variance exceeded SCIP's bound by up to $9\times10^{-10}$ on port1. |
| Judge G2 where both routes certify, and otherwise require SCIP's value inside the cutting-plane bracket. | Kelley-type cuts converge slowly on this MIQP: 2,000 masters left a $10^{-6}$ relative bracket at $\lambda = 1$. The bracket is pinned; SCIP's $10^{-9}$ feasibility tolerance can put its value $2\times10^{-11}$ below a certified bound, so the pin allows $10^{-8}$ relative. |
| Certify continuous frontier points by an active-set KKT solve (`mv::qp`). | KKT conditions are sufficient for a convex QP; numerical residuals are reported. |

## Formulations

| Decision | Reason |
| --- | --- |
| Carry the cardinality sense as a parameter; reproduce Chang et al. with $=$. | The source imposes equality; the specification uses $\le$. |
| F1p: $\pi = 0.999\,\lambda_{\min}(\Sigma)$. | Günlük and Linderoth's absolute $0.001$ makes $\pi$ negative at return scale. |
| F2 in Konno–Yamazaki's epigraph form. | It is the source's form; the $d^\pm$ split is equivalent. |
| F4 with the shared fully invested budget and no cost rows (owner, 2 October 2026). | Matching X(K) across formulations; with $c=0$ and positive rates the paper's model would forbid any trade. The paper's Table 2 rows all carry costs, so they are not targets for this variant. |
| F4 stage-2 tolerance $\epsilon_a = 10^{-9}$. | The source requires only a small $\epsilon$. |
| F5 objective of Harvey et al. (2010) eq. (6) with $\kappa = \gamma(\gamma+1)$ by default. | Their linear utility in moments is the stated model; $\kappa$ is CRRA's derivative ratio at unit wealth, a stated convention. |
| Use indtrack1 returns for F5. | port1 carries means, deviations and correlations only; the third moment needs scenarios. indtrack1 is the same Hang Seng universe, $N = 31$. |
| $\Sigma$ by Ledoit–Wolf constant-correlation shrinkage, divisor $T$. | The specification; the appendix estimators $\hat\pi_{ij}$ and $\hat\vartheta$ centre on the divisor-$T$ covariance. |

## Data and evaluation

| Decision | Reason |
| --- | --- |
| Commit OR-Library files with their notice, pinned by SHA-256. | OR-Library states the MIT licence; the Brunel server is slow (indtrack8 took over 30 minutes), so CI must not fetch it. |
| Refuse indtrack7/8's trailing series unless asked (`tail`). | The files hold one more series than their headers declare; Canakgoz and Beasley use the declared $N$. |
| Fetch the French factors file at run time; record its hash and its "created using the YYYYMM CRSP database" line. | The file carries a copyright notice and is revised between vintages. |
| Combine RET and DLRET; never read a missing value as zero. | Bessembinder includes delisting returns; a zero would be an invented return. |
| G3 replication without the Shumway fill; RQ2–RQ4 with it, and without it as a sensitivity. | Bessembinder used CRSP's delisting returns as supplied; the specification asks for the fill. |
| Refuse a missing return while a stock is listed; read one in the delisting month itself as a total loss. | A listed holding's value is unknown. Dropping the date on an unknown terminal return would remove it selectively from larger portfolios (docs/PLAN.md: no selective disappearance). |
| Selection receives `P::upto(t)`, a copy cut at $t$. | No-look-ahead holds by construction and is tested by perturbing later returns. |
| Estimate moments on common observed rows in the $M$-month window, requiring at least $m$. | No return is imputed; record scenario counts and unavailable selections. Freeze the estimator before P6. |
| Bessembinder's monthly-redraw bootstrap and the specification's buy-and-hold B4 kept apart. | The source redraws monthly and value-weights; B4 is an extension and is labelled so. |
| HAC Sharpe test with the Parzen kernel and bandwidth $4(T/100)^{2/9}$. | Ledoit and Wolf leave the kernel and bandwidth to Andrews (1991); prewhitening and their studentized bootstrap are open work ([Issues](ISSUES.md)). |
