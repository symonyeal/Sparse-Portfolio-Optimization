# Industry allocation experiment

Protocol recorded 5 November 2026 before its first run. This separate descriptive experiment
uses twelve aggregate industry portfolios. It does not address the individual-stock questions
in PLAN.md, identify wealth creators, or replace the missing delisting-inclusive CRSP extract.

Input: the [French 12 Industry Portfolios](https://mba.tuck.dartmouth.edu/pages/faculty/ken.french/Data_Library/det_12_ind_port.html),
value-weighted monthly return block. Fetch the official CSV ZIP to an ignored local cache;
pass the ZIP or extracted CSV with `pf learn --input PATH --out PATH`. Raw source files retain
their Fama/French copyright and are not redistributed under this repository's MIT licence.
The output records their SHA-256, source vintage and code identity.

The experiment fixes K=3, full investment, long-only weights, no return floor, monthly
rebalancing and the existing F2 mean-absolute-deviation objective. Two candidates estimate
on the preceding 60 or 120 complete months. Each solve has a 0.5-second time limit and zero
requested relative gap. Optimal and independently feasible limit incumbents are retained
with their achieved certificate. Unavailable or invalid incumbents remain explicit failures;
there is no substitute portfolio and no selective removal of dates.
Time-limited incumbents can vary between runs and machines; record their actual status and
gap rather than infer deterministic allocation or empirical superiority from a fixed budget.

At each test month, the adaptive strategy picks the candidate with the smaller sum of squared
realized monthly returns over the preceding twelve candidate holdings. These holdings were
themselves estimated before their respective holding months. Ties select the 60-month
candidate. Squared return is an uncentered risk score, not an expected-return forecast. A
132-month warm-up provides both estimation and validation histories. The full remaining
sample is evaluated; no dates or settings are selected after looking at performance.

Both fixed candidates, the adaptive candidate and a monthly rebalanced equal-weighted
12-industry baseline share the same reported test months. Output includes every date,
candidate weight vector, realized return, solver certificate and any failure; adaptive
scores and window choice; and descriptive gross terminal wealth, annualized volatility,
maximum drawdown and one-way turnover. Turnover compares new weights with the previous
portfolio after return drift; the first allocation is excluded. Aggregate path statistics
are unavailable when that strategy has any failed month. Successful months are never
stitched across failures. Numerical feasibility is checked at 1e-7 without changing weights,
so the certificate and the evaluated point refer to the same solver allocation.

These are changing research portfolios, not twelve individual stocks or directly tradable
funds. The analysis excludes trading costs, fees and taxes. Their membership is reassigned
annually. French reconstructs the history at each release, and changed from CRSP FIZ to CIZ
inputs in 2025. This walk-forward calculation uses one revised data vintage; it does not
reconstruct the data that an investor could download on each historical date. Results are
descriptive, with no investment recommendation, statistical superiority or causal claim.
