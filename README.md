# Sparse Portfolio Optimization

Results: [benchmark tables](docs/results/README.md), [industry experiment results](docs/results/INDUSTRY.md). Open issues and unavailable empirical evidence: [ISSUES](docs/ISSUES.md).

## Problem

- The question: does optimizing a limited set of holdings change the probability of underperforming the value-weighted market or Treasury bills, and how does the answer depend on the risk measure and the eligible universe?
- The individual-stock evaluation requires a licensed CRSP extract that includes delisted firms. Public benchmark results test the implementation; individual-stock comparisons await the extract. Nothing here is investment advice.
- A concentrated portfolio holds a small part of the equity market. Its performance depends on the names selected, the weights assigned and the rebalancing rule.
- Index sampling seeks to track a benchmark with fewer holdings; tracking error measures variation in returns relative to that benchmark. Active Share measures the difference in portfolio weights from the index. [Canakgoz and Beasley (2009)](https://doi.org/10.1016/j.ejor.2008.03.015), [Cremers and Petajisto (2009)](https://doi.org/10.1093/rfs/hhp057).
- Long-horizon stock wealth is positively skewed: a small group of firms accounts for the market's net wealth creation above Treasury bills. Omitting delisted firms introduces survivorship bias into an evaluation of stock selection. Rebalancing also changes the distribution of compound returns, so a small rebalanced portfolio and a small buy-and-hold portfolio answer different questions. [Bessembinder (2018)](https://doi.org/10.1016/j.jfineco.2018.06.004), [Farago and Hjalmarsson (2023)](https://doi.org/10.1093/rof/rfac021).
- The project compares risk measures and eligible universes. It makes no claim of priority over the [existing literature](docs/SOURCES.md).

## Methods

Cardinality-constrained models:

| Model | Criterion | Reference route | Independent check |
| --- | --- | --- | --- |
| F1 | mean–variance | SCIP | continuous KKT frontier; pinned cutting-plane bounds |
| F1p | perspective reformulation of F1 | SCIP | objective agreement with F1 |
| F2 | mean absolute deviation | HiGHS | deviation recomputed from returns |
| F3 | CVaR | HiGHS | sorted losses with a fractional tail atom |
| F4 | lexicographic regression tracking, zero costs | HiGHS | regression and tracking error from returns |
| F5 | mean, variance and third central moment | SCIP | pinned small-support lattice value |

- The [model](docs/MODEL.md) states the equations and source restrictions. F4 has no identical target in the paper's transaction-cost table. F5's default central-moment coefficients do not give a literal CRRA Taylor expansion about unit wealth.
- Restricted index tracking is implemented alongside the formulations above.
- [Validation](docs/VALIDATION.md) records the gates and checks. Benchmark tables are generated from result bundles carrying the code revision, input hashes, parameters, solver certificate and host. A limit is reported with its achieved gap.
- One separate [industry-allocation experiment](docs/INDUSTRY.md) compares fixed estimation windows with a rule that learns from prior realized losses, using French's public research portfolios. It records choices, allocation checks, solver bounds and unavailable dates, over 1,071 shared test months.
- `pf eval` prints the primary estimands: the share of start dates below B1 and below B5, with unavailable dates counted.
- `pf boot` and `pf oos` require the licensed extract and follow the [pre-registration](docs/PLAN.md).

## Data

- OR-Library data are included with their MIT notice and SHA-256 pins. They are survivor universes and support computational validation only.
- `pf fetch` downloads the French risk-free file. CRSP data must remain outside the repository.
- [Data](docs/DATA.md) defines the accepted schema and delisting treatment.
- Synthetic fixtures (`tests/data/panel.csv`, `tests/data/rf.csv`) test arithmetic, selection timing and numerical properties only.

## Execution

One Rust crate, `pf`, with HiGHS and SCIP as its solvers. No commercial solver is required.

- Building HiGHS needs CMake and a C++ compiler; SCIP's bindings need libclang. The build downloads SCIP's prebuilt library.
- On Windows, CMake and libclang must be on the path (or `CMAKE` and `LIBCLANG_PATH` set), and `CARGO_TARGET_DIR` must be a short path such as `C:\t\pf`, because highs-sys fails at 260 characters.

```text
cargo test --locked
cargo run --release -- bench --run --only g1-port1 --time 10
cargo run --release -- bench --run --only rq5-port1-K5 --time 60 --missing
cargo run --release -- bench
cargo run --release -- learn --input PATH_TO_12_Industry_Portfolios_CSV.zip --out industry.json
```

- `bench` without `--run` regenerates the benchmark table from admitted bundles.
- Bundles require a committed, clean tree and a binary built from it; Markdown-only changes do not dirty code identity.
- `--missing` reuses only admitted bundles with matching code, parameters and inputs.
- `--jobs` sets parallel cases; each completed case is saved. The manual GitHub Actions workflow runs one case per job and uploads its bundles.
- Commands resolve resources from the working directory; use `--root PATH` for another checkout. Unknown flags fail before running.
- A writer lease protects each output, and schema-3 bundle admission checks the payload as well as its provenance.
- `pf boot --crsp PATH` runs the Bessembinder replication; `pf oos --crsp PATH` runs the empirical comparisons; `pf eval --crsp PATH` runs on the synthetic fixture.

Code: [MIT](LICENSE). OR-Library data: [separate MIT notice](data/orlib/LICENSE).
