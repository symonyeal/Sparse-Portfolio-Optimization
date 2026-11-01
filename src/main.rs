//! pf: the command-line shell over the library.
//!
//! ```text
//! pf bench [--run] [--missing] [--only PREFIX]... [--time S] [--jobs N] [--allow-long]
//! pf oos --crsp PATH [--rf PATH] [--only S]... [--K 10] [--M 120] [--m 120] [--Nc 200]
//!        [--H 120] [--fill] [--time 60] [--seed 0] [--missing]
//! pf boot --crsp PATH [--rf PATH] [--n 20000] [--sizes 1 5 25 50 100] [--seed 0] [--missing]
//! pf fetch
//! ```
//!
//! `bench` without --run regenerates docs/results/README.md from admitted bundles. Every run
//! refuses to write bundles from a dirty tree or from a binary older than its sources. `oos` and
//! `boot` need the licensed CRSP extract (docs/DATA.md); without one, P5 and P6 stay blocked.

use pf::{bd, bench, boot, data, oos};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Parsed options: flag -> values (empty for a switch).
struct Args(BTreeMap<String, Vec<String>>);

impl Args {
    fn parse(a: &[String], switches: &[&str], lists: &[&str]) -> Result<Args, String> {
        let mut o: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut i = 0;
        while i < a.len() {
            let f = a[i].strip_prefix("--").ok_or(format!("unexpected argument {:?}", a[i]))?.to_string();
            i += 1;
            let e = o.entry(f.clone()).or_default();
            if switches.contains(&f.as_str()) {
                continue;
            }
            let n0 = e.len();
            while i < a.len() && !a[i].starts_with("--") {
                e.push(a[i].clone());
                i += 1;
                if !lists.contains(&f.as_str()) {
                    break;
                }
            }
            if e.len() == n0 {
                return Err(format!("--{f} needs a value"));
            }
        }
        Ok(Args(o))
    }

    fn has(&self, f: &str) -> bool {
        self.0.contains_key(f)
    }

    fn all(&self, f: &str) -> Vec<String> {
        self.0.get(f).cloned().unwrap_or_default()
    }

    fn get<T: std::str::FromStr>(&self, f: &str, d: T) -> Result<T, String> {
        match self.0.get(f).and_then(|v| v.last()) {
            None => Ok(d),
            Some(v) => v.parse().map_err(|_| format!("--{f}: invalid value {v:?}")),
        }
    }

    fn path(&self, f: &str) -> Option<PathBuf> {
        self.0.get(f).and_then(|v| v.last()).map(PathBuf::from)
    }
}

/// Code identity of a run that writes bundles; refuses a dirty tree.
fn clean() -> Result<serde_json::Value, String> {
    let c = bd::identity(&data::root());
    if c["dirty"] != false {
        return Err("refusing to write bundles from a dirty tree or a stale build; commit and rebuild first".into());
    }
    Ok(c)
}

fn bench(a: &Args, cmd: &[String]) -> Result<(), String> {
    let tl: f64 = a.get("time", 60.0)?;
    let jobs: usize = a.get("jobs", 1)?;
    if !tl.is_finite() || tl <= 0.0 || jobs < 1 {
        return Err("--time must be finite and positive; --jobs must be positive".into());
    }
    if a.has("run") {
        let code = clean()?;
        let only = a.all("only");
        let cases = bench::cases();
        if let Some(o) = only.iter().find(|o| !cases.iter().any(|c| c.0.starts_with(o.as_str()))) {
            return Err(format!("--only {o:?} does not match a public case"));
        }
        let src = code["source"].as_str().unwrap().to_string();
        let todo: Vec<_> = cases
            .into_iter()
            .filter(|c| only.is_empty() || only.iter().any(|o| c.0.starts_with(o.as_str())))
            .filter(|c| !a.has("missing") || bench::admit(&c.0, c.1, &c.2, tl, &src).is_none())
            .collect();
        let budget: f64 = todo.iter().map(|c| bench::cost(c.1) * tl).sum();
        if budget > 1800.0 && !a.has("allow-long") {
            return Err(format!(
                "solve budget {:.1} min exceeds 30 minutes; obtain approval and use --allow-long or dispatch the bench workflow",
                budget / 60.0
            ));
        }
        let next = AtomicUsize::new(0);
        let err = Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..jobs.min(todo.len().max(1)) {
                s.spawn(|| {
                    while let Some((k, kd, pa)) = todo.get(next.fetch_add(1, Ordering::SeqCst)) {
                        match bench::one(k, kd, pa, tl, &code, cmd) {
                            Ok(w) => println!("{k}: {w:.1}s"),
                            Err(e) => err.lock().unwrap().push(format!("{k}: {e}")),
                        }
                    }
                });
            }
        });
        let err = err.into_inner().unwrap();
        if !err.is_empty() {
            return Err(err.join("\n"));
        }
    }
    bench::table()
}

fn rf(a: &Args) -> PathBuf {
    a.path("rf").unwrap_or_else(data::ff_path)
}

fn run(argv: &[String]) -> Result<(), String> {
    let (c, rest) = argv.split_first().ok_or("usage: pf bench|oos|boot|fetch [options]; see src/main.rs")?;
    let cmd: Vec<String> = std::iter::once("pf".to_string()).chain(argv.iter().cloned()).collect();
    match c.as_str() {
        "bench" => bench(&Args::parse(rest, &["run", "missing", "allow-long"], &[])?, &cmd),
        "oos" => {
            let a = Args::parse(rest, &["fill", "missing"], &[])?;
            let crsp = a.path("crsp").ok_or("no CRSP extract supplied (--crsp): P6 is blocked; see docs/PLAN.md")?;
            let r = oos::Run {
                k: a.get("K", 10)?,
                mw: a.get("M", 120)?,
                m: a.get("m", 120)?,
                nc: a.get("Nc", 200)?,
                h: a.get("H", 120)?,
                fill: a.has("fill"),
                tl: a.get("time", 60.0)?,
                seed: a.get("seed", 0)?,
            };
            oos::main(&crsp, &rf(&a), &a.all("only"), &r, a.has("missing"), &oos::out(), &clean()?, &cmd)
        }
        "boot" => {
            let a = Args::parse(rest, &["missing"], &["sizes"])?;
            let crsp = a.path("crsp").ok_or("no CRSP extract supplied (--crsp): P5 is blocked; see docs/PLAN.md")?;
            let sizes: Vec<usize> = if a.has("sizes") {
                a.all("sizes")
                    .iter()
                    .map(|s| s.parse().map_err(|_| format!("--sizes: invalid value {s:?}")))
                    .collect::<Result<_, _>>()?
            } else {
                vec![1, 5, 25, 50, 100]
            };
            boot::main(
                &crsp,
                &rf(&a),
                a.get("n", 20000)?,
                &sizes,
                a.get("seed", 0)?,
                a.has("missing"),
                &boot::out(),
                &clean()?,
                &cmd,
            )
        }
        "fetch" => {
            let p = data::ff_path();
            let h = data::fetch(&p)?;
            println!("{} {h}", p.display());
            Ok(())
        }
        _ => Err(format!("unknown command {c:?}; use bench, oos, boot or fetch")),
    }
}

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match run(&a) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pf: {e}");
            ExitCode::FAILURE
        }
    }
}
