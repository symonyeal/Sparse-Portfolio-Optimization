# Design decisions

The [working contract](../CLAUDE.md) defines implementation and evidence rules. Independent
recomputation avoids errors shared by two solvers using the same row matrix. Synthetic
fixtures name the property tested and carry no empirical interpretation.

## Optional research tooling (26 October 2026)

| Decision | Reason |
| --- | --- |
| Port the holding account loop to a dependency-free Rust library behind explicit `--backend rust`. | Profiling a deterministic synthetic panel put most holding time in the Python loop; the original route remains the numerical oracle. |
| Retain NumPy's account sums after the Rust loop. | Changing reduction order can change delivered floating-point values. |
| Require source-matched builds and record the Rust binary hash in empirical bundles. | Native code must participate in stale-result rejection. |
| Use PyPortfolioOpt/CLARABEL as an optional continuous variance check. | It provides a separate implementation for an exact-mean continuous frontier; no cardinality certificate is claimed. |
| Use vectorbt on evaluated wealth. | It checks cumulative returns and drawdown without replacing the CRSP delisting account rules. |
| Read OpenBB exports offline with explicit adjustment provenance. | Provider price fields have different adjustment semantics and cannot establish a delisting-complete universe. |
| Keep Eiten and Vibe-Trading outside the dependency set; use awesome-quant as a directory. | They add overlapping portfolio heuristics or agent/execution infrastructure rather than an independent sparse certificate. Any new empirical comparison requires a registered extension. |

Interfaces, source links and reproduction commands are in [Integrations](INTEGRATIONS.md).

## Solvers (2 October 2026)

| Decision | Reason |
| --- | --- |
| SciPy `milp` (HiGHS) for every LP and MILP. | It ships with SciPy; F2–F4 and the ECP master are MILPs. |
| Admit PySCIPOpt 6.2.1 (SCIP 10.0) for F1, F1p and F5. | SciPy has no MIQP or MINLP solver. PySCIPOpt is MIT-licensed and SCIP 10 Apache-2.0; both measured working on Python 3.14 before adoption. |
| Scale the SCIP objective by $1/\overline{\operatorname{diag}\Sigma}$ and divide the certificate back; use scale one for deterministic returns. | SCIP's tolerances are absolute below one; unscaled, the recomputed variance exceeded SCIP's bound by up to $9\times10^{-10}$ on port1. |
| Keep the literal aggregate ECP cut $v \ge 2w_k^\top\Sigma w - w_k^\top\Sigma w_k$. | It is Westerlund and Pettersson's method. An eigen-disaggregated variant was measured and was slower: wider bracket after 60 s at $\lambda = 0.9$. |
| Judge G2 where both routes certify, and otherwise require SCIP's value inside ECP's certified bracket. | Kelley-type cuts converge slowly on this MIQP: 2,000 masters left a $10^{-6}$ relative bracket at $\lambda = 1$. |
| Certify continuous frontier points by an active-set KKT solve (`mv.qp`). | KKT conditions are sufficient for a convex QP; numerical residuals are reported. |

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
| Refuse indtrack7/8's trailing series unless `tail=True`. | The files hold one more series than their headers declare; Canakgoz and Beasley use the declared $N$. |
| Fetch the French factors file at run time; record its hash and its "created using the YYYYMM CRSP database" line. | The file carries a copyright notice and is revised between vintages. |
| Combine RET and DLRET; never read a missing value as zero. | Bessembinder includes delisting returns; a zero would be an invented return. |
| G3 replication without the Shumway fill; RQ2–RQ4 with it, and without it as a sensitivity. | Bessembinder used CRSP's delisting returns as supplied; the specification asks for the fill. |
| Refuse a missing return while a stock is listed. | The holding's value is then unknown. |
| Selection receives `P.upto(t)`, a copy cut at $t$. | No-look-ahead holds by construction and is tested by perturbing later returns. |
| Estimate moments on common observed rows in the $M$-month window, requiring at least $m$. | No return is imputed; record scenario counts and unavailable selections. Freeze the estimator before P6. |
| Bessembinder's monthly-redraw bootstrap and the specification's buy-and-hold B4 kept apart. | The source redraws monthly and value-weights; B4 is an extension and is labelled so. |
| HAC Sharpe test with the Parzen kernel and bandwidth $4(T/100)^{2/9}$. | Ledoit and Wolf leave the kernel and bandwidth to Andrews (1991); prewhitening and their studentized bootstrap are open work ([Issues](ISSUES.md)). |
