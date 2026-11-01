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
    b["bundle"] =
        json!({"schema": 2, "key": key, "pa": pa, "code": code, "inputs": ip, "command": cmd, "host": host()});
    b
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
    let tmp = out.join(format!(".{key}.{}.pending", std::process::id()));
    let raw = serde_json::to_string_pretty(b).map_err(|e| e.to_string())? + "\n";
    {
        use std::io::Write;
        let mut f = std::fs::File::create_new(&tmp).map_err(|e| e.to_string())?;
        f.write_all(raw.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    Ok(p)
}

/// A JSON object, or None for an absent, interrupted or malformed record.
pub fn read(p: &Path) -> Option<Value> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
    v.is_object().then_some(v)
}

/// A clean committed bundle matching the current sources, parameters and expected inputs.
pub fn valid(out: &Path, key: &str, pa: &Value, src: &str, ip: &Value) -> Option<Value> {
    let b = read(&out.join(format!("{key}.json")))?;
    let m = &b["bundle"];
    let c = &m["code"];
    let ok = m["schema"] == 2
        && m["key"] == key
        && c["dirty"] == false
        && c["source"] == src
        && &m["pa"] == pa
        && &m["inputs"] == ip
        && c["commit"].as_str().is_some_and(|x| committed(x, &root()));
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
    fn source_digest_tracks_the_tree() {
        assert_eq!(source(&root()), source(&root()));
        assert_ne!(source(&root()), source(&root().join("src")));
    }
}
