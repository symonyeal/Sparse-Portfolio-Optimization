//! Cardinality-constrained portfolio selection evaluated by long-horizon wealth creation.
//!
//! Core: X(K) (`x`), programs and certificates (`lp`, `scip`, `crt`), estimators (`est`) and
//! the formulations F1-F5 (`mv`, `mad`, `cv`, `it`, `sk`). Data and evaluation: `data`,
//! `panel`, `wc`, `bs`, `ev`. Shell: bundles (`bd`) and the runners `bench`, `oos`, `boot`.

pub mod bd;
pub mod bench;
pub mod boot;
pub mod bs;
pub mod crt;
pub mod cv;
pub mod data;
pub mod est;
pub mod ev;
pub mod eval;
pub mod it;
pub mod learn;
pub mod lp;
pub mod mad;
pub mod mv;
pub mod oos;
pub mod panel;
pub mod scip;
pub mod sk;
pub mod wc;
pub mod x;
