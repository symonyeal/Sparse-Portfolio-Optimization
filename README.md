# Sparse Portfolio Optimization

## In plain terms

Over their lifetimes, most individual US stocks have done worse than safe Treasury bills. A
few percent of companies account for all of the stock market's net gains (Bessembinder,
2018). A portfolio of only a few stocks is therefore a bet on catching one of those winners.

This project asks two things. Does choosing those few stocks by mathematical optimization,
rather than at random, lower the chance of doing worse than the whole market or than
Treasury bills? And how much does the answer depend on how "risk" is measured?

**Status: no answer yet.** The optimization models are built and checked on public test
data. The real test needs a licensed historical database of US stocks (CRSP). It must
include companies that later went bankrupt or were delisted, because leaving them out makes
any strategy look better than it was. That data is not yet available, so no investment
result has been produced. Nothing here is investment advice.

The rest of this page is written for specialists.

## Overview

Cardinality-constrained portfolio models with solver bounds, independent checks and a
framework for evaluating long-horizon outcomes on a universe that includes delisted firms.

The repository implements mean–variance, perspective mean–variance, absolute-deviation,
CVaR, restricted index-tracking and third-central-moment formulations. Public benchmark
results test the implementation. No CRSP empirical result has been produced; replication
and empirical comparison await the owner's licensed extract.

## Background

A concentrated portfolio holds a small part of the equity market. Its performance depends on
the names selected, the weights assigned and the rebalancing rule. Index sampling seeks to
track a benchmark with fewer holdings; tracking error measures variation in returns relative
to that benchmark. Active Share measures the difference in portfolio weights from the index.
[Canakgoz and Beasley (2009)](https://doi.org/10.1016/j.ejor.2008.03.015),
[Cremers and Petajisto (2009)](https://doi.org/10.1093/rfs/hhp057).

Long-horizon stock wealth is positively skewed: a small group of firms accounts for the
market's net wealth creation above Treasury bills. Omitting delisted firms introduces
survivorship bias into an evaluation of stock selection. Rebalancing also changes the
distribution of compound returns, so a small rebalanced portfolio and a small buy-and-hold
portfolio answer different questions.
[Bessembinder (2018)](https://doi.org/10.1016/j.jfineco.2018.06.004),
[Farago and Hjalmarsson (2023)](https://doi.org/10.1093/rof/rfac021).

This project asks whether optimizing a limited set of holdings changes the probability of
underperforming the value-weighted market or Treasury bills, and how much the answer depends
on the risk measure and the eligible universe. It makes no claim of priority over the
[existing literature](docs/SOURCES.md).

## Models and evidence

| Model | Criterion | Reference route | Independent check |
| --- | --- | --- | --- |
| F1 | mean–variance | SCIP | HiGHS cutting-plane bounds; continuous KKT frontier |
| F1p | perspective reformulation of F1 | SCIP | objective agreement with F1 |
| F2 | mean absolute deviation | HiGHS | deviation recomputed from returns |
| F3 | CVaR | HiGHS | sorted losses with a fractional tail atom |
| F4 | lexicographic regression tracking, zero costs | HiGHS | regression and tracking error from returns |
| F5 | mean, variance and third central moment | SCIP | exhaustive small-support simplex grid |

The [model](docs/MODEL.md) states the equations and source restrictions. F4 has no identical
target in the paper's transaction-cost table. F5's default central-moment coefficients do
not give a literal CRRA Taylor expansion about unit wealth.

[Validation](docs/VALIDATION.md) records the gates and checks;
[benchmark tables](docs/results/README.md) are generated from result bundles carrying the
code revision, input hashes, parameters, solver certificate and host. A limit is reported with
its achieved gap. [Open issues](docs/ISSUES.md) identifies unavailable empirical evidence.

## Reproduction

Use Python with the dependencies in `requirements.txt`. No commercial solver is required.

```text
python -m pip install -r requirements.txt
python -m pytest -q
python -m ruff check .
python -m pip check
python run_bench.py --run --only g1-port1 --time 10
python run_bench.py --run --only g2-port1-lam0.25 --time 60 --missing
python run_bench.py
```

On the project owner's Windows machine, pass pytest an explicit `--basetemp` under
`Claude Func Folder`. Benchmark bundles require a committed, clean code tree. Markdown-only
changes do not dirty code identity. `--missing` reuses only admitted bundles with matching
code, parameters and inputs. `--jobs` controls parallel cases; each completed case is saved.
The manual GitHub Actions workflow runs one benchmark case per job and uploads its bundles.

OR-Library data are included with their MIT notice and SHA-256 pins. They are survivor
universes and support computational validation only. The French risk-free file is fetched
at runtime; CRSP data must remain outside the repository. [Data](docs/DATA.md) defines the
accepted schema and delisting treatment.

`run_boot.py` runs the Bessembinder replication; `run_oos.py` runs empirical comparisons.
Both require the licensed extract and follow the [pre-registration](docs/PLAN.md).
Synthetic fixtures test arithmetic, selection timing and numerical properties only.

Code: [MIT](LICENSE). OR-Library data: [separate MIT notice](data/orlib/LICENSE).
