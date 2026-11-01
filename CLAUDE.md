# Working contract

Read before changing anything. [README](README.md) states what the models are;
[Decisions](docs/DECISIONS.md) states why they are that way; [Validation](docs/VALIDATION.md)
states what has been checked. This file states the rules a change must not break.

## Always on

| Rule | Reason |
| --- | --- |
| Report no solve outcome that was not read back from the result: status, primal value, dual bound, achieved gap. | A requested tolerance is not a result. HiGHS stops at `mip_rel_gap = 1e-4` by default. |
| Take a bound only from the solver's dual bound (HiGHS `mip_dual_bound`; SCIP `SCIPgetDualbound`), never from an incumbent. | An incumbent can understate the optimum. |
| Name a failed solve by the solver's status: infeasible, limit, unbounded, numerical. | Bad data and an empty feasible set both end in failure. |
| Re-solve with presolve off whenever infeasibility is reported. | HiGHS presolve declared feasible programs infeasible in Electricity-Clearing-and-Pricing. |
| Reject a certificate whose bounds cross by more than the tolerance. | A dual bound past the primal value is a defect. |
| Report a solve that hit its time limit as `limit`, with its gap. | It is not optimal. |
| Cite only what [Sources](docs/SOURCES.md) records as opened, and add the entry in the change that uses it. | A formulation is validated by the paper it implements, not by the solver that ran it. |
| No CRSP or other WRDS row-level data in the repository, its history or CI logs. | The data are licensed to the owner. |
| No future data in selection; the set T enters `src/ev.rs` only. | A test asserts it. |
| No empirical claim from survivor-only (OR-Library) or synthetic data. | Both universes exclude the firms the question is about. |
| Add a dependency only after measurement; no commercial solver as a requirement. | HiGHS solves every LP and MILP; SCIP is admitted for MIQP and MINLP only. |
| State what was measured and what was not. | "Not measured" is a finding. |

## The feasible set is written once

`src/x.rs` holds X(K) (`Xk::set`) and the return row (`ret`). Every formulation writes its
linear part into an `Lp` (`src/lp.rs`) and solves it with HiGHS, or opens it as a SCIP model
(`src/scip.rs`) and adds its nonlinear constraints:

| Module | Model | Solver |
| --- | --- | --- |
| `src/mv.rs` | F1, F1p | SCIP |
| `src/mad.rs`, `src/cv.rs`, `src/it.rs` | F2, F3, F4 | HiGHS |
| `src/sk.rs` | F5 | SCIP |

`Xk::ok` checks membership from the definition, not from the rows; the tests use it. The
independent checks (`mv::qp`, `mad::dev`, `cv::cvar`, `est::ols`) recompute from data and the
returned weights and never build rows. G2 and the F5 lattice are pinned values in
`tests/models.rs`; do not change a pin without rerunning its route from the recorded revision.

The rest of the crate: `crt` certificates, `est` estimators, `data` loaders, `panel` and `oos`
the out-of-sample engine, `wc`, `bs` and `boot` wealth creation and the bootstrap, `ev`
evaluation and inference, `bd` bundles, `bench` the public cases, `main` the `pf` command.

## Before a change is finished

```text
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Result bundles under `docs/results/` carry the revision that produced them; `pf` refuses to
write them from a dirty tree or from a binary built from other sources. Before any batch
longer than 30 minutes in total, give a time estimate and ask the owner; offload the tail to
the `Public benchmark cases` workflow.

On Windows, building HiGHS and SCIP's bindings needs CMake and libclang on the path (or
`CMAKE` and `LIBCLANG_PATH` set).
