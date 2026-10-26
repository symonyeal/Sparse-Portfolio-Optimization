# Validation

## Optional tooling checks, 26 October 2026

The full `python -m pytest -q` run with the Rust library and optional Python packages
enabled returned **182 passed, 1 xfailed**, with no skips, in 43.70 seconds. The expected
failure is the existing strict G1 printed-frontier gate described below. Python was 3.14;
NumPy was 2.4.6. `python -m ruff check .` and `python -m pip check` passed. The system
Python environment's dependency check also passed after the optional packages were
installed in a separate working directory with `pip --target`.

`cargo test --locked --manifest-path native/Cargo.toml` returned one passing Rust test.
`cargo fmt --manifest-path native/Cargo.toml -- --check` and
`cargo clippy --locked --manifest-path native/Cargo.toml -- -D warnings` passed.
The new GitHub Actions jobs have been configured; their hosted execution is not checked.

The Rust holding tests compare output bytes against Python for seeded synthetic panels,
including delistings, missing listed returns, total loss, zero weights, tolerated tiny
negative weights and reordered columns. They also reject an intentionally altered path
and a mismatched embedded source. A synthetic `run_oos.py` test checks identical terminal
wealth and selections, separate Rust bundle keys, and the recorded native binary hash.
The Python route also matches the baseline captured before the port on the saved panel.
Parity on a licensed CRSP panel is not measured because the extract is unavailable.

The final same-input timing used 100 holding calls per batch, five batches per backend,
H = 120 months and K = 100 supplied holdings. Both routes matched the saved baseline exactly.
The seed was 26; this was a synthetic arithmetic check on the same Windows laptop.

| Holding backend | Median seconds per 100 calls | Observed range, seconds |
| --- | ---: | ---: |
| Python | 2.121632 | 1.367047–4.427479 |
| Rust with NumPy account sums | 0.108604 | 0.103602–0.127810 |

The ratio of medians is 19.54. Runtime varied across batches; the measurement covers the
holding kernel and its wrapper only. Solver and end-to-end speedups are not measured.

`python run_compare.py` passed both public-data checks: PyPortfolioOpt/CLARABEL continuous
variance against the KKT frontier on port1, and vectorbt cumulative wealth against
buy-and-hold accounts on indtrack1. The maximum wealth difference was
1.2434497875801753e-14. The optional library versions and reproduction commands are in
[Integrations](INTEGRATIONS.md). OpenBB export parsing was tested on offline fixtures;
live provider access was not checked. No new empirical comparison or investment-performance
claim was produced.

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
