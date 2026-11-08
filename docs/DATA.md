# Data

| Tier | Source | In the repository | Universe | Use |
| --- | --- | --- | --- | --- |
| D1 | OR-Library `port1`–`port5`, `portef1`–`portef5` | `data/orlib`, MIT notice, SHA-256 pins | index constituents (survivors) | G1, G2, the Chang comparison, RQ5 |
| D2 | OR-Library `indtrack1`–`indtrack8` | `data/orlib` | survivors | F2–F5 checks, F4 |
| D3 | CRSP monthly stock file with delisting fields, via WRDS | never; licensed to the owner | full, with delistings | RQ1, RQ2, RQ4 |
| D4 | French data library, `F-F_Research_Data_Factors` RF | fetched to `data/fetched/`, ignored by Git | n/a | B5, excess returns |
| D5 | synthetic CRSP-format panel and RF series | `tests/data` | by construction | the pinned out-of-sample test only |

## D1 and D2 formats

- `portk`: $N$; $N$ lines of mean and standard deviation; every pair $i \le j$ once with its
  correlation. $\Sigma_{ij} = s_i s_j \rho_{ij}$. The loader refuses a missing, repeated or
  extra pair and any extra token.
- `portefk`: (mean, variance) per line, 2,000 lines, ten decimals: the unconstrained
  frontier, computed by Chang et al. with NAG E04NAF at 2,000 return targets.
- `indtrackk`: $N$, $T$; index levels $t = 0..T$; each stock's levels $t = 0..T$.
  indtrack7 declares $N = 1318$ and holds 1,319 stock series; indtrack8 declares 2,151 and holds
  2,152. The loader refuses the trailing series unless called with `tail`.
- Returns from D2 are log returns, as in Canakgoz and Beasley (2009); 290 weekly returns.

## D3: the expected CRSP extract

One CSV, legacy SIZ monthly layout, with exactly these fields in any order and case:
`PERMNO, PERMCO, DATE (or MTHCALDT), SHRCD, EXCHCD, PRC, SHROUT, RET, DLSTCD, DLRET`. Any other
field is refused. Dates must be valid YYYYMMDD; identifiers are integers and observed returns
are finite and at least $-1$. CRSP's missing-value codes (letters, $-66$, $-77$, $-88$,
$-99$, blank) become nan with the code kept. Share codes outside 10–12 are dropped after reading.
Market value is $\lvert\text{PRC}\rvert\cdot\text{SHROUT}$.

Place the file outside the repository or under `data/crsp/` (ignored), and pass its path to
`pf boot --crsp` or `pf oos --crsp`. Bundles record its SHA-256, never its rows.

RQ1 evaluates July 1926–December 2016 and also requires June 1926 price and share observations
for July's beginning values. T-bill coverage is required for the evaluated months only.

## Missing values

| Case | Treatment |
| --- | --- |
| RET missing, DLRET present | $R = $ DLRET |
| both present | $R = (1+\text{RET})(1+\text{DLRET}) - 1$ |
| DLRET missing after a performance delisting, `--fill` | $-0.30$ NYSE/AMEX (codes 500, 520–584), $-0.55$ Nasdaq (500, 505–588) |
| return missing during a holding, with no recorded delisting | refused, including an unexplained end of the series |
| RET and DLRET both missing in the delisting month | total loss of the position; the date stays in the sample |
| return missing after a recorded delisting month | proceeds held at the T-bill rate |
