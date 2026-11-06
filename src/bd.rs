//! Atomic result bundles with code, parameters, input hashes and host identity.
//!
//! A bundle is admitted (for tables and for resuming) only when it was written from a clean,
//! committed tree whose source digest equals the current one, with the same parameters and
//! input hashes. The binary carries the digest of the sources it was built from; a binary older
//! than the sources counts as a dirty tree.

use crate::data::root;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

// Legend
// key : case key, a file stem; pa : case parameters; r : result record; ip : {input: sha256}
// code: {commit, dirty, source}; out : results directory

/// Files besides `src/**/*.rs` that decide a result; build.rs embeds the digest of the same set.
const CODE: &[&str] = &["Cargo.toml", "Cargo.lock", "build.rs", ".cargo/config.toml", "data/orlib/SHA256"];

fn rs(d: &Path, base: &Path, out: &mut Vec<String>) {
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs(&p, base, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Digest over (path, sha256 with LF line ends) of every source and configuration file.
pub fn source(r: &Path) -> String {
    let mut f = Vec::new();
    rs(&r.join("src"), r, &mut f);
    f.extend(CODE.iter().filter(|p| r.join(p).exists()).map(|p| p.to_string()));
    f.sort();
    let mut h = Sha256::new();
    for p in &f {
        let b = std::fs::read(r.join(p)).unwrap_or_default();
        let b = String::from_utf8_lossy(&b).replace("\r\n", "\n");
        h.update(format!("{p}\0{:x}\n", Sha256::digest(b.as_bytes())));
    }
    format!("{:x}", h.finalize())
}

/// Successful Git output, or None when Git cannot establish the requested fact.
fn git(r: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git").arg("-C").arg(r).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Whether commit names a full, existing commit object in the repository at r.
pub fn committed(commit: &str, r: &Path) -> bool {
    let full = (commit.len() == 40 || commit.len() == 64)
        && commit.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
    let top = git(r, &["rev-parse", "--show-toplevel"]);
    full && top.is_some_and(|t| Path::new(&t).canonicalize().ok() == r.canonicalize().ok())
        && git(r, &["cat-file", "-t", commit]).as_deref() == Some("commit")
}

/// Full commit, dirty flag (results and prose excluded; stale binary included) and digest.
pub fn identity(r: &Path) -> Value {
    let commit = git(r, &["rev-parse", "--verify", "HEAD^{commit}"]).filter(|c| committed(c, r));
    let d = git(r, &["status", "--porcelain", "--untracked-files=all", "--", ".", ":!docs/results", ":!*.md"]);
    let src = source(r);
    let dirty = commit.is_none() || d.is_none_or(|d| !d.is_empty()) || src != env!("PF_SOURCE");
    json!({"commit": commit, "dirty": dirty, "source": src})
}

/// Machine and solver versions a result was produced on.
pub fn host() -> Value {
    let env = |k: &str| std::env::var(k).ok();
    // Safety: SCIPversion reads a compile-time constant.
    let scip = unsafe { russcip::ffi::SCIPversion() };
    json!({
        "node": env("COMPUTERNAME").or(env("HOSTNAME")), "cpu": env("PROCESSOR_IDENTIFIER"),
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "pf": env!("CARGO_PKG_VERSION"), "scip": scip,
    })
}

/// The result r of case key as a bundle.
pub fn bundle(key: &str, pa: &Value, r: Value, ip: &Value, cmd: &[String], code: &Value) -> Value {
    let mut b = r;
    let digest = payload_hash(&b);
    b["bundle"] = json!({"schema": 3, "key": key, "pa": pa, "code": code, "inputs": ip, "command": cmd, "host": host(), "payload": digest});
    b
}

fn payload_hash(b: &Value) -> String {
    let mut p = b.clone();
    p.as_object_mut().expect("bundle payload must be an object").remove("bundle");
    format!("{:x}", Sha256::digest(serde_json::to_vec(&p).unwrap()))
}

/// Exclusive ownership of an output root or file; an interrupted writer leaves its lease for review.
pub struct Lease(PathBuf);

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub fn lease(p: &Path) -> Result<Lease, String> {
    let p = p.with_file_name(format!("{}.lock", p.file_name().ok_or("lease needs a filename")?.to_string_lossy()));
    let mut f = std::fs::File::create_new(&p).map_err(|e| {
        format!("writer lease {}: {e}; inspect an interrupted writer before archiving its lock", p.display())
    })?;
    use std::io::Write;
    writeln!(f, "{}", std::process::id()).map_err(|e| e.to_string())?;
    Ok(Lease(p))
}

/// Flush and rename a complete file under exclusive ownership; preserves the previous file on failure.
pub fn atomic(p: &Path, raw: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(p.parent().ok_or("output needs a parent directory")?).map_err(|e| e.to_string())?;
    let _lease = lease(p)?;
    let tmp = p.with_file_name(format!(".{}.{}.pending", p.file_name().unwrap().to_string_lossy(), std::process::id()));
    use std::io::Write;
    let mut f = std::fs::File::create_new(&tmp).map_err(|e| e.to_string())?;
    f.write_all(raw).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    drop(f);
    std::fs::rename(&tmp, p).map_err(|e| e.to_string())
}

/// Atomically replace out/<key>.json after flushing a complete JSON record.
pub fn write(out: &Path, b: &Value) -> Result<PathBuf, String> {
    let key = b["bundle"]["key"].as_str().unwrap_or("");
    let ok = key.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && key.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c));
    if !ok {
        return Err("bundle key must be a filename stem without path separators".into());
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let p = out.join(format!("{key}.json"));
    let raw = serde_json::to_string_pretty(b).map_err(|e| e.to_string())? + "\n";
    atomic(&p, raw.as_bytes())?;
    Ok(p)
}

/// A JSON object, or None for an absent, interrupted or malformed record.
pub fn read(p: &Path) -> Option<Value> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
    v.is_object().then_some(v)
}

fn finite(v: &Value) -> bool {
    v.as_f64().is_some_and(f64::is_finite)
}

fn nonnegative(v: &Value) -> bool {
    v.as_f64().is_some_and(|x| x.is_finite() && x >= 0.0)
}

fn nullable(v: &Value) -> bool {
    v.is_null() || finite(v)
}

fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => (a - b).abs() <= 1e-5 * a.abs().max(b.abs()).max(1.0),
        (None, None) => a.is_null() && b.is_null(),
        _ => false,
    }
}

/// Certificate acceptance concerns read-back consistency, not proof of the underlying formulation.
fn certificate(c: &Value) -> bool {
    let Some(o) = c.as_object() else { return false };
    if !["st", "sn", "f", "bd", "g", "t"].iter().all(|k| o.contains_key(*k))
        || !nonnegative(&c["t"])
        || !nullable(&c["f"])
        || !nullable(&c["bd"])
        || !(c["g"].is_null() || nonnegative(&c["g"]))
    {
        return false;
    }
    let sn = match c["sn"].as_str() {
        Some("min") => crate::crt::Sense::Min,
        Some("max") => crate::crt::Sense::Max,
        _ => return false,
    };
    match c["st"].as_str() {
        Some("opt") if !finite(&c["f"]) || !finite(&c["bd"]) => return false,
        Some("infeasible" | "unbounded") if !c["f"].is_null() => return false,
        Some("opt" | "limit" | "infeasible" | "unbounded" | "numerical") => {}
        _ => return false,
    }
    match (c["f"].as_f64(), c["bd"].as_f64()) {
        (Some(f), Some(bd)) => {
            !crate::crt::crossed(sn, f, bd, crate::crt::TOL) && same(&c["g"], &json!(crate::crt::gap(f, bd)))
        }
        _ => c["g"].is_null(),
    }
}

/// Required fields and cross-field relationships for each retained wire result family.
fn payload(key: &str, b: &Value) -> bool {
    if key.starts_with("oos-") {
        let Some(mo) = b["mo"].as_array() else { return false };
        let n = mo.len();
        if n == 0
            || !["W", "pc", "selection", "B1", "B1_errors", "B5"]
                .iter()
                .all(|k| b[*k].as_array().is_some_and(|v| v.len() == n))
            || b["fills"].as_u64().is_none()
            || mo.iter().any(|v| v.as_i64().is_none_or(|m| !(1..=12).contains(&(m % 100))))
            || mo.windows(2).any(|w| w[0].as_i64() >= w[1].as_i64())
        {
            return false;
        }
        return (0..n).all(|i| {
            let s = &b["selection"][i];
            let w = &b["W"][i];
            let Some(pc) = b["pc"][i].as_array() else { return false };
            let status = s["status"].as_str();
            pc.iter().all(|p| p.as_i64().is_some_and(|id| id > 0))
                && s["eligible"].as_u64().is_some_and(|n| pc.len() as u64 <= n)
                && matches!(
                    status,
                    Some("ok" | "unavailable" | "opt" | "limit" | "infeasible" | "unbounded" | "numerical")
                )
                && s.get("c").is_none_or(certificate)
                && match status {
                    Some("ok") => s.get("c").is_none(),
                    Some("unavailable") => true,
                    Some(st) => s.get("c").is_some_and(|c| c["st"] == st),
                    None => false,
                }
                && (w.is_null() || nonnegative(w))
                && nonnegative(&b["B5"][i])
                && (b["B1"][i].is_null() || nonnegative(&b["B1"][i]))
                && (if b["B1"][i].is_null() { b["B1_errors"][i].is_string() } else { b["B1_errors"][i].is_null() })
                && (w.is_null() == pc.is_empty())
                && (status != Some("ok") || nonnegative(w))
                && (status != Some("unavailable") || (w.is_null() && s["reason"].is_string()))
                && (w.is_null() || s.get("c").is_none_or(|c| finite(&c["f"])))
        });
    }
    if key.starts_with("boot-") {
        return ["life", "1y", "10y"].iter().all(|h| {
            let s = &b[*h];
            finite(&s["mean"])
                && finite(&s["med"])
                && s.get("skew").is_some_and(nullable)
                && ["gt0", "gtf", "gtvw"].iter().all(|k| s[*k].as_f64().is_some_and(|x| (0.0..=1.0).contains(&x)))
        });
    }
    if !nonnegative(&b["wall"]) {
        return false;
    }
    if key.starts_with("g1-") {
        return b["n"].as_u64().is_some_and(|n| n > 0)
            && ["max_abs", "max_rel", "kkt", "max_separation"].iter().all(|k| nonnegative(&b[*k]))
            && ["max_pos", "min_neg"].iter().all(|k| finite(&b[*k]))
            && b["half_unit"].as_f64().is_some_and(|x| (0.0..=1.0).contains(&x))
            && b["rectangles"].as_u64().is_some_and(|n| Some(n) <= b["n"].as_u64())
            && b["pass"].as_bool().is_some();
    }
    if key.starts_with("chang-") {
        let Some(p) = b["pts"].as_array() else { return false };
        return p.len() == 50
            && p.iter().all(|q| {
                finite(&q["lam"])
                    && matches!(q["st"].as_str(), Some("opt" | "limit" | "infeasible" | "unbounded" | "numerical"))
                    && ["g", "y", "x", "pd"].iter().all(|k| q.get(*k).is_some_and(nullable))
            })
            && b["n"].as_u64() == Some(p.iter().filter(|q| finite(&q["pd"])).count() as u64)
            && b["all_opt"].as_bool() == Some(p.iter().all(|q| q["st"] == "opt"))
            && b.get("median").is_some_and(nullable)
            && b.get("mean").is_some_and(nullable);
    }
    if key.starts_with("f4-") {
        if !certificate(&b["c1"])
            || !b["N"].as_u64().is_some_and(|n| n > 0)
            || b["drop"].as_u64().is_none()
            || b.get("c2").is_none()
        {
            return false;
        }
        let stages =
            if b["c2"].is_null() { b["c1"]["st"] != "opt" } else { b["c1"]["st"] == "opt" && certificate(&b["c2"]) };
        let metrics = ["a_in", "b_in", "a_out", "b_out", "te_in", "te_out", "names"];
        return stages
            && if finite(&b["c1"]["f"]) {
                metrics[..6].iter().all(|k| finite(&b[*k]))
                    && b["names"].as_u64().is_some_and(|n| n > 0 && Some(n) <= b["N"].as_u64())
            } else {
                metrics.iter().all(|k| b.get(*k).is_none()) && b["c2"].is_null()
            };
    }
    if key.starts_with("rq5-") || key.starts_with("f5-") {
        return certificate(&b["c"])
            && b["N"].as_u64().is_some_and(|n| n > 0)
            && b.get("f").is_some_and(nullable)
            && same(&b["f"], &b["c"]["f"]);
    }
    if key.starts_with("f2-") || key.starts_with("f3-") {
        return certificate(&b["c"])
            && b.get("recomputed").is_some_and(nullable)
            && same(&b["recomputed"], &b["c"]["f"])
            && b.get("diff").is_some_and(nullable)
            && b.get("ok").is_some()
            && (if finite(&b["c"]["f"]) { b["ok"] == true && nonnegative(&b["diff"]) } else { b["ok"].is_null() });
    }
    false
}

/// A clean committed bundle matching the current sources, parameters and expected inputs.
pub fn valid(out: &Path, key: &str, pa: &Value, src: &str, ip: &Value) -> Option<Value> {
    let b = read(&out.join(format!("{key}.json")))?;
    let m = &b["bundle"];
    let c = &m["code"];
    let ok = m["schema"] == 3
        && m["key"] == key
        && c["dirty"] == false
        && c["source"] == src
        && &m["pa"] == pa
        && &m["inputs"] == ip
        && c["commit"].as_str().is_some_and(|x| committed(x, &root()))
        && m["payload"].as_str() == Some(payload_hash(&b).as_str())
        && payload(key, &b);
    ok.then_some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(n: &str) -> PathBuf {
        let d = root().join("target").join("test-bundles").join(n);
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn nonfinite_fields_are_null_and_bad_keys_refused() {
        let d = dir("a");
        let b = bundle("k1", &json!({}), json!({"f": f64::NAN}), &json!({}), &[], &json!({}));
        let p = write(&d, &b).unwrap();
        assert!(read(&p).unwrap()["f"].is_null());
        let mut b2 = b.clone();
        b2["bundle"]["key"] = json!("../x");
        assert!(write(&d, &b2).is_err());
    }

    #[test]
    fn damaged_or_mismatched_bundles_are_not_admitted() {
        let d = dir("b");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("k.json"), "{\"a\": 1").unwrap();
        assert!(valid(&d, "k", &json!({}), "s", &json!({})).is_none());
        let code = json!({"commit": "0".repeat(40), "dirty": false, "source": "s"});
        let b = bundle("k", &json!({"tl": 1.0}), json!({}), &json!({"x": "h"}), &[], &code);
        write(&d, &b).unwrap();
        // a nonexistent revision is never admitted, whatever else matches
        assert!(valid(&d, "k", &json!({"tl": 1.0}), "s", &json!({"x": "h"})).is_none());
        assert!(!committed("HEAD", &root()));
        assert!(!committed(&"0".repeat(40), &root()));
    }

    #[test]
    fn payload_damage_and_inconsistent_certificates_are_rejected() {
        let d = dir("payload");
        let commit = git(&root(), &["rev-parse", "HEAD"]).unwrap();
        let code = json!({"commit": commit, "dirty": false, "source": "s"});
        let r = json!({"c": {"st": "opt", "sn": "min", "f": 1.0, "bd": 1.0, "g": 0.0, "t": 0.0},
            "recomputed": 1.0, "diff": 0.0, "ok": true, "wall": 0.0});
        let b = bundle("f2-test", &json!({}), r.clone(), &json!({}), &[], &code);
        write(&d, &b).unwrap();
        assert!(valid(&d, "f2-test", &json!({}), "s", &json!({})).is_some());
        // Changing bytes without updating metadata fails the digest check.
        let mut damaged = b.clone();
        damaged["recomputed"] = json!(2.0);
        write(&d, &damaged).unwrap();
        assert!(valid(&d, "f2-test", &json!({}), "s", &json!({})).is_none());
        // Recomputing the digest cannot admit a semantically malformed record.
        for field in ["recomputed", "c", "ok", "diff"] {
            let mut damaged = r.clone();
            damaged.as_object_mut().unwrap().remove(field);
            write(&d, &bundle("f2-test", &json!({}), damaged, &json!({}), &[], &code)).unwrap();
            assert!(valid(&d, "f2-test", &json!({}), "s", &json!({})).is_none(), "{field}");
        }
        let mut damaged = r;
        damaged["c"]["bd"] = json!(2.0);
        assert!(!payload("f2-test", &damaged));
        let oos = json!({"mo": [200001], "W": [1.0], "pc": [[1]], "selection": [{"status": "ok", "eligible": 1}],
            "B1": [1.0], "B1_errors": [null], "B5": [1.0], "fills": 0});
        assert!(payload("oos-test", &oos));
        let mut damaged = oos;
        damaged["B5"] = json!([]);
        assert!(!payload("oos-test", &damaged));
    }

    #[test]
    fn writer_conflict_preserves_the_previous_file() {
        let d = dir("lease");
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("report.md");
        atomic(&p, b"accepted").unwrap();
        let owner = lease(&p).unwrap();
        assert!(atomic(&p, b"replacement").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"accepted");
        drop(owner);
        atomic(&p, b"replacement").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"replacement");
    }

    #[test]
    fn admitted_shapes_cover_limited_producer_results() {
        for kd in ["rq5", "f2", "f3", "f4", "f5"] {
            let (key, _, pa) = crate::bench::cases().into_iter().find(|c| c.1 == kd).unwrap();
            let mut r = crate::bench::run(kd, &pa, 0.0).unwrap();
            r["wall"] = json!(0.0);
            assert!(payload(&key, &r), "{key}: {r}");
        }
        // Stage 1 can hit its limit with an incumbent; stage 2 is then absent.
        let c = crate::crt::C::new(
            crate::crt::St::Limit,
            crate::crt::Sense::Min,
            Some(1.0),
            Some(0.0),
            0.0,
            crate::crt::TOL,
        )
        .unwrap();
        let f4 = json!({"c1": c.json(), "c2": null, "N": 2, "drop": 0, "wall": 0.0,
            "a_in": 1.0, "b_in": 1.0, "a_out": 1.0, "b_out": 1.0,
            "te_in": 1.0, "te_out": 1.0, "names": 1});
        assert!(payload("f4-test", &f4));
        // PERMCO labels can repeat when separate share classes are selected.
        let oos = json!({"mo": [200001], "W": [1.0], "pc": [[1, 1]],
            "selection": [{"status": "ok", "eligible": 2}],
            "B1": [1.0], "B1_errors": [null], "B5": [1.0], "fills": 0});
        assert!(payload("oos-test", &oos));
        let mut bad = oos;
        bad["selection"][0]["status"] = json!("opt");
        assert!(!payload("oos-test", &bad));
    }

    #[test]
    fn pinned_panel_baseline_records_pass_result_admission() {
        let at = root().join("tests/data");
        let rows = crate::data::crsp(&at.join("panel.csv"), &[10, 11, 12]).unwrap();
        let rf = crate::data::rf(&at.join("rf.csv")).unwrap();
        for fill in [false, true] {
            let (p, nf) = crate::panel::panel(&rows, &rf, fill).unwrap();
            let c = crate::oos::Run { k: 3, mw: 12, m: 6, nc: 8, h: 6, fill, tl: 20.0, seed: 0 };
            for s in ["b2", "b3", "b4"] {
                let r = crate::oos::record(s, &p, &c, nf).unwrap();
                assert!(payload("oos-test", &r), "{s} fill={fill}");
            }
        }
    }

    #[test]
    fn source_digest_tracks_the_tree() {
        assert_eq!(source(&root()), source(&root()));
        assert_ne!(source(&root()), source(&root().join("src")));
    }
}
