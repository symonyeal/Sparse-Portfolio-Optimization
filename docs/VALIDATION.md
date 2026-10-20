# Validation

## Formulation

All optimization routes reach `pf/rw.py` for the budget, weight bounds and cardinality row.
The indicator count is distinguished from the number of positive weights when the floor is
zero. F4 retains the source's regression approximation with the owner-approved zero-cost
budget; the original transaction-cost results are not targets for this restriction.

## Algorithms

The [model](MODEL.md#6-algorithms-and-independent-checks) lists reference and independent
routes. Each check recomputes from data and returned weights rather than rebuilding the
optimization rows.

## Gates

Local reproduction on 20 October 2026: the existing suite returned 142 passed and
one expected failure at the declared printed-precision frontier gate. Ruff and
dependency checks passed. The owner confirmed that no licensed CRSP extract is
available. Public numerical checks therefore remain the available evidence;
G3 and the CRSP empirical studies remain blocked, with no investment-performance
claim added and no synthetic replacement for the missing empirical data.

| Gate | Required evidence |
| --- | --- |
| G1 | OR-Library unconstrained frontier at its printed coordinate precision; also report raw differences and KKT residuals |
| G2 | F1 objective agreement within 1e-6 relative wherever SCIP and ECP both certify; otherwise report the valid ECP bracket |
| F1p | equality with F1's integer optimum; root-bound comparison reported independently |
| F2/F3 | returned primal agrees with independently recomputed absolute deviation or fractional-tail CVaR |
| F4 restricted | both lexicographic stages certified, source regression identity checked, tracking error recomputed |
| F5 | SCIP upper bound does not lie below the best small-support grid value |
| G3 | licensed CRSP replication against Bessembinder Table 4 under PLAN.md; currently blocked |

The source frontier prints both coordinates with finite precision. A variance difference
at the printed mean is reported separately from agreement with the rectangle defined by
rounding both coordinates. A test that accepts a larger unexplained tolerance cannot
establish the printed-precision gate.

G1 is not met on portef1: 142 of 2,000 printed rectangles do not intersect the KKT-certified
frontier. The maximum separation is $8.960743692193462\times10^{-11}$; the maximum KKT
residual is $1.3322676295501878\times10^{-15}$. Raw coordinate differences remain reported.
The strict gate is an expected test failure while the discrepancy remains unresolved.

## Computation

The authoritative measured tables are [generated from admitted bundles](results/README.md).
Each bundle records code, input hashes, parameters and host. Tables must reject stale
bundles; a clean source digest alone cannot establish that the inputs still match.

No CRSP or out-of-sample empirical run has been performed.

## Verification (5 October 2026)

The full local suite reports 142 passed and one expected failure for G1. Ruff, dependency
consistency and active documentation links pass. Regression checks cover data validation,
selection timing, delisting, solver deadlines and bounds, bundle admission and runner inputs.

Ordinary deterministic solver fixtures retain their weights and recomputed objectives after
cleanup; timing and certificate metadata are excluded from that comparison. Corrections
cover singular and deterministic inputs, tiny CVaR tails, unavailable observations and
numerical cancellation in Sharpe inference.

Eight admitted public bundles cover port1 G1/G2, F1/F1p at $K=5$, F2/F3 at $K=5$, restricted
F4 on indtrack1 and the small F5 grid. The G2 case meets relative objective agreement;
its stricter bracket-inclusion diagnostic fails. Both diagnostics remain in the table.
