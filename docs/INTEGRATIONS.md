# Optional research tools

The certified F1–F5 solvers and registered empirical comparison retain their current
definitions. External tools provide continuous optimization checks, arithmetic checks on
evaluated wealth, and exploratory public-price inputs. They introduce no selection strategy
into `run_oos.py` or amendment to `PLAN.md`.

## Choice of tools

| Resource | Use | Boundary |
| --- | --- | --- |
| Rust | Optional holding account recursion in `native/src/lib.rs`, selected by `--backend rust` | Python remains the default and numerical oracle; no solver or random-number generator is ported. |
| [PyPortfolioOpt](https://github.com/PyPortfolio/PyPortfolioOpt) | Continuous long-only minimum variance, with an optional exact mean row | No cardinality constraint or sparse optimality certificate. Input moments retain their own period and units. |
| [vectorbt](https://github.com/polakowo/vectorbt) | Cumulative returns and drawdown from already evaluated portfolio wealth | No price-based order reconstruction, fees, annualization, or substitute delisting treatment. |
| [awesome-quant](https://github.com/wilsonfreitas/awesome-quant) | Resource directory used to assess overlap with the current NumPy/SciPy stack | A directory is not an installable dependency or evidence for a formulation. |
| [Vibe-Trading](https://github.com/HKUDS/Vibe-Trading) | Reviewed for applicability | Its agent, data-provider and execution stack adds no independent certificate for F1–F5; no runtime dependency added. |
| [OpenBB](https://github.com/openbq-org/OpenBB) | Offline `to_dataframe()` export reader with explicit provider, price field and adjustment convention | Public price exports lack the licensed CRSP universe and delisting coverage. |
| [Eiten](https://github.com/tradytics/eiten) | Reviewed its minimum-variance, eigenportfolio, Sharpe and genetic-search comparison set | Existing continuous minimum variance and B2 supply the relevant starting checks; additional strategies would require a separately registered empirical extension. |

PyPortfolioOpt is MIT-licensed. The reviewed vectorbt license is Apache 2.0 with the Commons
Clause; it is an optional installed package, and no vectorbt code is vendored here. OpenBB's
full provider stack is not required to read an export. Provider credentials and network
requests remain in the user's OpenBB environment.

## Rust holding backend

Rust uses no crate dependencies. Build with a Rust toolchain supporting edition 2024:

```text
cargo build --release --locked --manifest-path native/Cargo.toml --target-dir BUILD_DIR
cargo test --locked --manifest-path native/Cargo.toml --target-dir BUILD_DIR
cargo clippy --locked --manifest-path native/Cargo.toml --target-dir BUILD_DIR -- -D warnings
```

On the owner's machine, `BUILD_DIR` must be under `Claude Func Folder`; use
`sparse-portfolio/cargo`. Set `PF_NATIVE` to the absolute library path:
`BUILD_DIR/release/pf_hold.dll` on Windows, `libpf_hold.so` on Linux, or
`libpf_hold.dylib` on macOS. For example, in PowerShell:

```powershell
$env:PF_NATIVE = 'ABSOLUTE_BUILD_DIR\release\pf_hold.dll'
python -m pytest tests/test_native.py -q --basetemp 'ABSOLUTE_WORK_DIR\pytest-native'
python run_oos.py --crsp LICENSED_PATH --rf RF_PATH --backend rust
```

The loader requires explicit configuration and compares the library's embedded Rust source
and Cargo manifest with the current files. A stale build is refused. Rust bundles use a
separate `-rust` key, record the binary SHA-256, and include the Rust sources in code identity.
An explicitly requested backend cannot silently fall back to Python.

The Rust kernel updates individual asset accounts and the cash account in the original
order. NumPy computes the final row sums on both routes, retaining its reduction order.
The account matrix uses O(HK) additional memory, where H is the horizon and K is the number
of supplied holdings. Missing listed returns raise an error; confirmed delisting proceeds
earn T-bills as in the Python oracle. Random draws remain in their original Python modules.

## Independent library checks

Install the optional requirements and run the public-data comparison:

```text
python -m pip install -r requirements-research.txt
python run_compare.py
python run_compare.py --only frontier
python run_compare.py --only vectorbt
python -m pytest tests/test_external.py -q
```

The direct integration pins exercised on Python 3.14 were PyPortfolioOpt 1.6.0,
vectorbt 1.2.0, CVXPY 1.9.3 and CLARABEL 0.11.1, with NumPy 2.4.6, Numba 0.68.0
and llvmlite 0.50.0. They are optional and recorded in `requirements-research.txt`.

`run_compare.py` uses the pinned OR-Library files without a network request. The continuous
frontier check compares PyPortfolioOpt/CLARABEL against the existing active-set KKT solve.
The exact mean row is explicit: PyPortfolioOpt's default `efficient_return` uses a minimum
mean, which would answer a different question. Library weights are retained without calling
`clean_weights`, which would clip small values and round further.
External solver status is returned; no dual bound or sparse certificate is inferred from it.

The vectorbt check compares cumulative returns against equal-weight buy-and-hold accounts
on indtrack1. `pf.ext.vectorbt(W)` can also inspect any evaluated wealth path, including a
`bt.hold` result. The initial value is one. A complete loss stays at zero; recovery without
new capital is refused. No return or volatility is annualized. The public survivor-only
universe supports computational checks only.

## OpenBB exports

The reader accepts a DataFrame from `obb.equity.price.historical(...).to_dataframe()` or a
saved export. A single-symbol export may need an explicit `symbol` column added by the
caller. The OpenBB standard model names `close`; adjustment fields vary by provider.
There is no automatic choice of an adjusted field or assumption of total returns.

```python
from pf import ext

# f: an OpenBB-exported DataFrame with date, symbol and the chosen price field.
q = ext.openbb(f, price="adj_close", provider="PROVIDER_NAME",
               retrieved="2026-10-26", adjustment="PROVIDER_DOCUMENTED_CONVENTION")
R = q["returns"]
metadata = q["provenance"]
```

The reader rejects duplicate date-symbol rows and nonpositive observed prices. Dates and
symbols are sorted, and missing prices remain missing through `pct_change(fill_method=None)`.
No monthly aggregation, history imputation, market-cap construction, return-adjustment
claim, or delisting inference is made. The output is marked exploratory and is not accepted
as a replacement for the CRSP panel. Save the export, its hash, provider and retrieval date
before any subsequent reproducible study.
