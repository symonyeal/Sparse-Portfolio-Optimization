//! Embed the digest of the sources the binary was built from, and place SCIP's runtime
//! libraries beside the binaries on Windows (scip-sys links them but leaves them in its build
//! directory, where the loader does not look).

use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

/// Files besides `src/**/*.rs` whose content decides a result; `bd::source` reads the same set.
const CODE: &[&str] = &["Cargo.toml", "Cargo.lock", "build.rs", ".cargo/config.toml", "data/orlib/SHA256"];

fn rs(d: &Path, out: &mut Vec<String>) {
    for e in fs::read_dir(d).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn main() {
    let mut f = Vec::new();
    rs(Path::new("src"), &mut f);
    f.extend(CODE.iter().filter(|p| Path::new(p).exists()).map(|p| p.to_string()));
    f.sort();
    let mut h = Sha256::new();
    for p in &f {
        let b = fs::read(p).unwrap();
        let b = String::from_utf8_lossy(&b).replace("\r\n", "\n");
        h.update(format!("{p}\0{:x}\n", Sha256::digest(b.as_bytes())));
    }
    println!("cargo:rustc-env=PF_SOURCE={:x}", h.finalize());
    println!("cargo:rerun-if-changed=src");
    for p in CODE {
        println!("cargo:rerun-if-changed={p}");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && let Ok(lib) = env::var("DEP_SCIP_LIBDIR")
    {
        let out = env::var("OUT_DIR").unwrap();
        let prof = Path::new(&out).ancestors().nth(3).unwrap();
        let bin = Path::new(&lib).parent().unwrap().join("bin");
        for e in fs::read_dir(bin).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "dll") {
                for d in [prof.to_path_buf(), prof.join("deps")] {
                    let q = d.join(p.file_name().unwrap());
                    if !q.exists() {
                        fs::create_dir_all(&d).unwrap();
                        fs::copy(&p, q).unwrap();
                    }
                }
            }
        }
    }
}
