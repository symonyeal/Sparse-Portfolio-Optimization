# Industry allocation results

The [fixed protocol](../INDUSTRY.md) was run on the official French 12-industry
value-weighted monthly block, July 1926–September 2026 (1,203 months). The 202609 CRSP
vintage ZIP has SHA-256
`78ff32de9b3b308c96beb48a5f6f8ae06209495ef63ee6efa274d6bdbb491b86`.
[Complete results](industry.json) retain source/code/protocol identity, all candidate
allocations and certificates, validation histories and monthly paths.

All four methods have the same 1,071 test months, July 1937–September 2026. Of 2,166
candidate solves, 2,163 were reported optimal by HiGHS and three retained feasible time-limit incumbents
(relative gaps 0.081%, 2.132%, 2.517%). The solver stopped at its default absolute gap of 1e-6
on 161 of the 2,163, with gaps up to 5.4e-5 (every primal-bound difference below 1e-6); this
file predates the stricter tolerance now set in `src/lp.rs`. No dates were dropped. Adaptive selection chose
60 months on 604 dates and 120 months on 467 dates.

| Method | Annualized volatility | Maximum drawdown | Mean monthly one-way turnover | Terminal gross wealth, initial 1 |
| --- | ---: | ---: | ---: | ---: |
| Fixed 60 months | 12.27% | 41.92% | 8.48% | 3,645.68 |
| Fixed 120 months | 12.25% | 39.76% | 4.97% | 4,670.29 |
| Adaptive | 12.08% | 37.56% | 7.97% | 4,317.56 |
| Equal 12 industries | 15.23% | 49.55% | 1.08% | 14,561.83 |

Adaptive selection reduced sample volatility and drawdown relative to both fixed
windows, while trading more than the 120-month method. The diversified equal-weight
baseline accumulated more gross wealth with greater volatility and drawdown. The
objective and selection score measure risk; neither estimates expected return.
This comparison does not establish statistical superiority or investment suitability.

An independent computation from the original ZIP reconciled all dates, unchanged
weights, budget/cardinality constraints, training MAD objectives and objective bounds,
realized returns, prior-only adaptive scores, drifted turnover and summary statistics.
The calculation checks the retained feasible points; it does not independently prove
the solver's global bounds. The 68 repository tests, formatting and warning-denied
Clippy checks also passed on Windows.

Returns use changing-membership research portfolios from one revised vintage, with no
trading costs, fees or taxes. They are not point-in-time tradable funds. Time limits can
change incumbent choices on another run or machine. The licensed individual-stock
CRSP study remains pending; this experiment does not replace it.
