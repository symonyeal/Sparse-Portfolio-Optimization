# Industry allocation results

The [fixed protocol](../INDUSTRY.md) was run on the official French 12-industry
value-weighted monthly block, July 1926–September 2026 (1,203 months). The 202609 CRSP
vintage ZIP has SHA-256
`78ff32de9b3b308c96beb48a5f6f8ae06209495ef63ee6efa274d6bdbb491b86`.
[Complete results](industry.json) retain source/code/protocol identity, all candidate
allocations and certificates, validation histories and monthly paths.

All four methods have the same 1,071 test months, July 1937–September 2026. Of the 2,142
candidate solves stored with those months, 2,090 were reported optimal by HiGHS and 52
retained feasible time-limit incumbents (relative gaps up to 4.5%, median 0.75%, 22 above 1%).
The optimal solves stop at a gap of at most 2.2e-9 under the stricter tolerance in `src/lp.rs`;
the earlier run, with the default absolute gap of 1e-6, reported 3 solves at the limit. The
0.5-second limit makes the count machine-dependent: a run on the same laptop while other
builds were running had 188 at the limit and terminal wealth within 2% of the figures below.
This run was on an otherwise idle laptop. No dates were dropped.
Adaptive selection chose 60 months on 606 dates and 120 months on 465 dates.

| Method | Annualized volatility | Maximum drawdown | Mean monthly one-way turnover | Terminal gross wealth, initial 1 |
| --- | ---: | ---: | ---: | ---: |
| Fixed 60 months | 12.27% | 41.92% | 8.48% | 3,634.14 |
| Fixed 120 months | 12.26% | 39.76% | 4.98% | 4,587.52 |
| Adaptive | 12.11% | 37.56% | 7.84% | 4,325.81 |
| Equal 12 industries | 15.23% | 49.55% | 1.08% | 14,561.83 |

Adaptive selection reduced sample volatility and drawdown relative to both fixed
windows, while trading more than the 120-month method. The diversified equal-weight
baseline accumulated more gross wealth with greater volatility and drawdown. The
objective and selection score measure risk; neither estimates expected return.
This comparison does not establish statistical superiority or investment suitability.

An independent computation from the original ZIP reconciled all dates, unchanged
weights, budget/cardinality constraints, training MAD objectives and objective bounds,
realized returns, prior-only adaptive scores, drifted turnover and summary statistics
for the earlier run; it has not been repeated on this one. The calculation checks the
retained feasible points; it does not independently prove the solver's global bounds.

Returns use changing-membership research portfolios from one revised vintage, with no
trading costs, fees or taxes. They are not point-in-time tradable funds. Time limits can
change incumbent choices on another run or machine. The licensed individual-stock
CRSP study remains pending; this experiment does not replace it.
