# Validation

## Rust port, 1 November 2026

The crate replaces the Python package of revision 1ca7e76. Parity was measured against
result bundles that the Python code wrote at that revision on the same inputs, with the same
solvers and limits. The harness and its output are kept outside the repository; the pins it
produced are in `tests/models.rs` and `tests/data/oos.json`.

| Case | Status | Objective, relative | Weights, max abs |
| --- | --- | ---: | ---: |
| F1, F1p on port1, K = 5, 10 | opt/opt | ≤ 4.1e-15 | ≤ 3.1e-14 |
| G2 on port1, five λ | opt/opt | ≤ 5.2e-16 | 0 |
| F2, F3 on indtrack1, K = 5, 10, with and without ρ | opt/opt | ≤ 2.3e-14 | ≤ 6.7e-14 |
| F4 on indtrack1 | opt/opt | both stages ≈ 0 | 5.0e-16 |
| F5, 36 × 6 lattice case | opt/opt, 35/35 nodes | 2.8e-16 | 2.4e-18 |
| F5 on indtrack1, K = 3, 60 s | limit/limit | 6.0e-16 | 1.5e-10 |

F4 on indtrack2 reaches the same zero objectives with different weights (max 0.46): the
optimum is not unique there. F5 at its time limit has the same incumbent and a dual bound
2.5% apart: the two SCIP builds take different search paths. The F2 node counts
differ (103 against 184 at K = 5) for the same reason.
The independent routes (`mv::qp`, `est::shrink`, `est::ols`) agree to 3.5e-15 or better.

Out of sample, on the synthetic 40-firm panel in `tests/data` (K = 3, M = 12, Nc = 8,
H = 6, m = 6 and 12, with and without the Shumway fill), B2, B3, F2 and F3 match at every
one of the 43 dates: the same selection status, the same holdings, and terminal wealth within
4.3e-16 relative. F1 has the same status and holdings, with wealth within 7.7e-6 relative;
the difference comes from SCIP's 1e-9 feasibility tolerance at each solve. F5 differs at one
or two dates, all of them solves stopped by the 20 s limit, and its wealth then differs by
up to 6.2%. F5 is therefore not pinned out of sample.

Gates on this revision: `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D
warnings` pass; `cargo test --locked` runs 52 tests and all pass (43 unit, 8 model, 1
out-of-sample).

## Formulation

All optimization routes reach `src/x.rs` for the budget, weight bounds and cardinality row.
The indicator count is distinguished from the number of positive weights when the floor is
zero. F4 retains the source's regression approximation with the owner-approved zero-cost
budget; the original transaction-cost results are not targets for this restriction.

## Algorithms

The [model](MODEL.md#6-algorithms-and-independent-checks) lists the independent routes.
Each check recomputes from data and returned weights rather than rebuilding the optimization
rows.

## Gates

The owner confirmed that no licensed CRSP extract is available. Public numerical checks
therefore remain the available evidence; G3 and the CRSP empirical studies remain blocked,
with no investment-performance claim and no synthetic replacement for the missing data.

| Gate | Required evidence |
| --- | --- |
| G1 | OR-Library unconstrained frontier at its printed coordinate precision; also report raw differences and KKT residuals |
| G2 | F1 objective within 1e-6 relative of the pinned cutting-plane value wherever both certify, and inside the pinned bracket to 1e-8 relative everywhere |
| F1p | equality with F1's integer optimum; root-bound comparison reported independently |
| F2/F3 | returned primal agrees with independently recomputed absolute deviation or fractional-tail CVaR |
| F4 restricted | both lexicographic stages certified, source regression identity checked, tracking error recomputed |
| F5 | SCIP dual bound does not lie below the pinned best small-support lattice value |
| G3 | licensed CRSP replication against Bessembinder Table 4 under PLAN.md; currently blocked |

The source frontier prints both coordinates with finite precision. A variance difference
at the printed mean is reported separately from agreement with the rectangle defined by
rounding both coordinates. A test that accepts a larger unexplained tolerance cannot
establish the printed-precision gate.

G1 is not met on portef1: 142 of 2,000 printed rectangles do not intersect the KKT-certified
frontier. The maximum separation is $8.96\times10^{-11}$; the KKT residuals stay below
$10^{-12}$. `tests/models.rs` asserts this count and separation, so the discrepancy
stays recorded rather than hidden by a looser tolerance.

## Computation

The authoritative measured tables are [generated from admitted bundles](results/README.md).
Each bundle records code, input hashes, parameters and host. Tables reject stale bundles; a
clean source digest alone cannot establish that the inputs still match.

No licensed CRSP individual-stock empirical run has been performed. The separate
[industry experiment](results/INDUSTRY.md) reports a walk-forward aggregate-portfolio study.

## Stored results and runtime resources

Bundle schema 3 adds a digest of the result payload and admission rules for each retained
result family. Admission checks required fields, paired dates and outcomes, allowed statuses,
certificate bounds and gaps, and recomputed objective agreement. A digest detects changed
bytes; these structural checks do not re-prove an optimization result. Schema 2 bundles are
not resumed or rendered and must be recomputed with the current sources.

The CLI owns a writer lease for the complete benchmark, bootstrap or out-of-sample command;
individual publications also acquire a file lease. Tables and bundles are flushed to a
pending file and renamed. A failed French factors download or monthly-block validation
preserves the previously accepted file. An interrupted writer can leave a `.lock` or
`.pending` file: inspect the process and archive the abandoned files before retrying.
Crash durability of directory renames and network filesystems has not been measured.

Resource paths resolve from the working directory, or the immutable root supplied by
`--root PATH`. A relocated executable does not read from its build checkout. Set this root
before workers start; a second configuration attempt is rejected.
