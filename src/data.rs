//! Strict loaders for the data tiers; a field or record the loader does not read is refused.
//!
//! D1  OR-Library port1-port5 and portef1-portef5 (Beasley, 1990; Chang et al., 2000)
//! D2  OR-Library indtrack1-indtrack8 (Beasley, Meade and Chang, 2003; Canakgoz and Beasley, 2009)
//! D3  CRSP monthly stock file with delisting returns, supplied by the owner, never committed
//! D4  Kenneth French data library, one-month T-bill (RF of F-F_Research_Data_Factors), fetched
//!
//! OR-Library files are committed under data/orlib with their MIT notice and pinned by SHA-256.
//! indtrack7 and indtrack8 carry one stock series beyond the N their headers declare; the loader
//! refuses it unless the caller passes tail, and then reports the dropped values.

use nalgebra::{DMatrix, DVector};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

// Legend
// N, T   : assets; time periods (indtrack: levels at t = 0..T)
// mu, sd : mean and standard deviation of each asset's return (port)
// cr, s  : correlation and covariance matrices, s = sd_i sd_j cr_ij
// i, v   : index levels (T+1) and stock levels (N, T+1) (indtrack)
// ef     : frontier points (mean, variance) (portef)
// mo     : months as YYYYMM integers; rf : one-month T-bill return per month (decimal)
// sha    : SHA-256 of the raw file bytes

pub const FF: &str = "https://mba.tuck.dartmouth.edu/pages/faculty/ken.french/ftp/F-F_Research_Data_Factors_CSV.zip";

/// The repository root: data and results are read and written relative to it.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn orlib_dir() -> PathBuf {
    root().join("data").join("orlib")
}

/// Default location of the fetched French factors file (ignored by Git).
pub fn ff_path() -> PathBuf {
    root().join("data").join("fetched").join("F-F_Research_Data_Factors_CSV.zip")
}

pub fn sha256(p: &Path) -> Result<String, String> {
    let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(format!("{:x}", Sha256::digest(&b)))
}

/// File name -> pinned SHA-256 of the committed OR-Library files.
pub fn pins() -> Result<BTreeMap<String, String>, String> {
    let p = orlib_dir().join("SHA256");
    let t = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    t.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| match l.split_whitespace().collect::<Vec<_>>()[..] {
            [h, n] => Ok((n.trim_start_matches('*').to_string(), h.to_string())),
            _ => Err(format!("{}: malformed pin {l:?}", p.display())),
        })
        .collect()
}

/// SHA-256 of the committed copy of an OR-Library file, checked against its pin.
pub fn pinned(name: &str) -> Result<String, String> {
    let f = format!("{name}.txt");
    let h = sha256(&orlib_dir().join(&f))?;
    if pins()?.get(&f) != Some(&h) {
        return Err(format!("{f}: SHA-256 {h} does not match the pin"));
    }
    Ok(h)
}

/// Tokens of an OR-Library file: the committed, pinned copy unless a path is given.
fn tokens(name: &str, path: Option<&Path>) -> Result<(Vec<String>, String, String), String> {
    let (p, sha) = match path {
        Some(p) => (p.to_path_buf(), sha256(p)?),
        None => (orlib_dir().join(format!("{name}.txt")), pinned(name)?),
    };
    let t = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    if !t.is_ascii() {
        return Err(format!("{}: not ASCII", p.display()));
    }
    let t = String::from_utf8(t).unwrap();
    Ok((t.split_whitespace().map(String::from).collect(), p.display().to_string(), sha))
}

fn num(tok: &str, p: &str, what: &str) -> Result<f64, String> {
    match tok.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        Ok(_) => Err(format!("{p}: {what} is not finite: {tok:?}")),
        Err(_) => Err(format!("{p}: {what} is not a number: {tok:?}")),
    }
}

fn int(tok: &str, p: &str, what: &str) -> Result<usize, String> {
    let v = num(tok, p, what)?;
    if v.fract() != 0.0 || v < 1.0 {
        return Err(format!("{p}: {what} must be a positive integer, got {tok:?}"));
    }
    Ok(v as usize)
}

pub struct Port {
    pub n: usize,
    pub mu: Vec<f64>,
    pub s: DMatrix<f64>,
    pub name: String,
    pub sha: String,
}

/// D1 instance portk: N; N lines (mean, std); every pair i <= j once with its correlation.
pub fn port(k: usize, path: Option<&Path>) -> Result<Port, String> {
    let (t, p, sha) = tokens(&format!("port{k}"), path)?;
    if t.is_empty() {
        return Err(format!("{p}: empty"));
    }
    let n = int(&t[0], &p, "N")?;
    if n.to_string() != t[0] {
        return Err(format!("{p}: N must be a positive integer, got {:?}", t[0]));
    }
    let np = n * (n + 1) / 2;
    if t.len() != 1 + 2 * n + 3 * np {
        return Err(format!("{p}: {} tokens, expected {} for N={n}", t.len(), 1 + 2 * n + 3 * np));
    }
    let a: Vec<f64> = t[1..1 + 2 * n].iter().map(|x| num(x, &p, "value")).collect::<Result<_, _>>()?;
    let (mu, sd): (Vec<f64>, Vec<f64>) = a.chunks(2).map(|c| (c[0], c[1])).unzip();
    if sd.iter().any(|&s| s <= 0.0) {
        return Err(format!("{p}: nonpositive standard deviation"));
    }
    let mut cr = DMatrix::from_element(n, n, f64::NAN);
    for q in t[1 + 2 * n..].chunks(3) {
        let (i, j) = (int(&q[0], &p, "index")? - 1, int(&q[1], &p, "index")? - 1);
        let v = num(&q[2], &p, "correlation")?;
        if !(i <= j && j < n) {
            return Err(format!("{p}: pair ({}, {}) outside 1 <= i <= j <= {n}", i + 1, j + 1));
        }
        if !cr[(i, j)].is_nan() {
            return Err(format!("{p}: pair ({}, {}) repeated", i + 1, j + 1));
        }
        if v.abs() > 1.0 {
            return Err(format!("{p}: correlation {v} outside [-1, 1]"));
        }
        cr[(i, j)] = v;
        cr[(j, i)] = v;
    }
    if cr.iter().any(|v| v.is_nan()) {
        return Err(format!("{p}: missing correlation pairs"));
    }
    if (0..n).any(|i| (cr[(i, i)] - 1.0).abs() > 1e-8 + 1e-5) {
        return Err(format!("{p}: diagonal correlation not 1"));
    }
    let s = DMatrix::from_fn(n, n, |i, j| sd[i] * sd[j] * cr[(i, j)]);
    Ok(Port { n, mu, s, name: format!("port{k}"), sha })
}

pub struct Portef {
    pub ef: Vec<(f64, f64)>,
    pub name: String,
    pub sha: String,
}

/// D1 frontier portefk: (mean, variance) per line; the unconstrained frontier, not a CCEF.
pub fn portef(k: usize, path: Option<&Path>) -> Result<Portef, String> {
    let (t, p, sha) = tokens(&format!("portef{k}"), path)?;
    if t.is_empty() {
        return Err(format!("{p}: empty frontier"));
    }
    if t.len() % 2 == 1 {
        return Err(format!("{p}: odd number of tokens"));
    }
    let v: Vec<f64> = t.iter().map(|x| num(x, &p, "value")).collect::<Result<_, _>>()?;
    let ef: Vec<(f64, f64)> = v.chunks(2).map(|c| (c[0], c[1])).collect();
    if ef.iter().any(|e| e.1 < 0.0) {
        return Err(format!("{p}: negative variance"));
    }
    Ok(Portef { ef, name: format!("portef{k}"), sha })
}

pub struct Indtrack {
    pub n: usize,
    pub t: usize,
    pub i: DVector<f64>,
    pub v: DMatrix<f64>,
    pub drop: usize,
    pub name: String,
    pub sha: String,
}

impl Indtrack {
    /// Stock log returns (T, N) and index log returns (T), r_t = log(L_t / L_{t-1}).
    pub fn lr(&self) -> (DMatrix<f64>, DVector<f64>) {
        let r = DMatrix::from_fn(self.t, self.n, |t, i| (self.v[(i, t + 1)] / self.v[(i, t)]).ln());
        let ri = DVector::from_fn(self.t, |t, _| (self.i[t + 1] / self.i[t]).ln());
        (r, ri)
    }
}

/// D2 instance indtrackk: (N, T); index levels t=0..T; each stock's levels t=0..T.
pub fn indtrack(k: usize, path: Option<&Path>, tail: bool) -> Result<Indtrack, String> {
    let (t, p, sha) = tokens(&format!("indtrack{k}"), path)?;
    if t.len() < 2 {
        return Err(format!("{p}: missing header"));
    }
    let (n, tt) = (int(&t[0], &p, "N")?, int(&t[1], &p, "T")?);
    let m = (n + 1) * (tt + 1);
    let x = &t[2..];
    if x.len() < m {
        return Err(format!("{p}: {} values, expected {m}", x.len()));
    }
    let dr = x.len() - m;
    if dr > 0 && !tail {
        return Err(format!("{p}: {dr} values after the {n} declared series; pass tail to drop them"));
    }
    if dr > 0 && (!(k == 7 || k == 8) || dr != tt + 1) {
        return Err(format!("{p}: only the documented extra series in indtrack7/8 may be dropped"));
    }
    for y in &x[m..] {
        if num(y, &p, "trailing level")? <= 0.0 {
            return Err(format!("{p}: nonpositive trailing price level"));
        }
    }
    let v: Vec<f64> = x[..m].iter().map(|y| num(y, &p, "level")).collect::<Result<_, _>>()?;
    if v.iter().any(|&y| y <= 0.0) {
        return Err(format!("{p}: nonpositive price level"));
    }
    let l = DMatrix::from_row_slice(n + 1, tt + 1, &v);
    Ok(Indtrack {
        n,
        t: tt,
        i: l.row(0).transpose(),
        v: l.rows(1, n).into_owned(),
        drop: dr,
        name: format!("indtrack{k}"),
        sha,
    })
}

// D4 ------------------------------------------------------------------------------------------

/// Fetch the French factors zip with the system curl (Schannel on Windows); returns its SHA.
pub fn fetch(dst: &Path) -> Result<String, String> {
    std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut c = std::process::Command::new("curl");
    c.arg("-sSLf");
    if cfg!(windows) {
        c.arg("--ssl-no-revoke");
    }
    let st = c.args(["--max-time", "120", "-o"]).arg(dst).arg(FF).status().map_err(|e| format!("curl: {e}"))?;
    if !st.success() {
        return Err(format!("curl failed: {st}"));
    }
    sha256(dst)
}

pub struct Rf {
    pub mo: Vec<i64>,
    pub rf: Vec<f64>,
    pub sha: String,
    pub vintage: String,
}

/// D4 monthly one-month T-bill returns (decimal) from the French factors file (zip or CSV).
///
/// Reads the monthly block between its header ',Mkt-RF,SMB,HML,RF' and the first blank line;
/// months must be consecutive. Values are printed in percent.
pub fn rf(p: &Path) -> Result<Rf, String> {
    let d = p.display();
    let mut b = std::fs::read(p).map_err(|e| format!("{d}: {e}"))?;
    if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")) {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(b)).map_err(|e| format!("{d}: {e}"))?;
        if z.len() != 1 {
            return Err(format!("{d}: expected one member, got {}", z.len()));
        }
        let mut m = Vec::new();
        z.by_index(0).map_err(|e| format!("{d}: {e}"))?.read_to_end(&mut m).map_err(|e| format!("{d}: {e}"))?;
        b = m;
    }
    let txt: String = b.iter().map(|&c| c as char).collect(); // latin-1
    let l: Vec<&str> = txt.lines().collect();
    let h = l
        .iter()
        .position(|x| x.replace(' ', "") == ",Mkt-RF,SMB,HML,RF")
        .ok_or(format!("{d}: monthly header not found"))?;
    let (mut mo, mut v) = (Vec::new(), Vec::new());
    for x in &l[h + 1..] {
        if x.trim().is_empty() {
            break;
        }
        let c: Vec<&str> = x.split(',').map(str::trim).collect();
        if c.len() != 5 || c[0].len() != 6 || !c[0].bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("{d}: malformed monthly row {x:?}"));
        }
        let m: i64 = c[0].parse().unwrap();
        if !(1..=12).contains(&(m % 100)) {
            return Err(format!("{d}: invalid calendar month"));
        }
        mo.push(m);
        v.push(num(c[4], &d.to_string(), "RF")? / 100.0);
    }
    if mo.is_empty() {
        return Err(format!("{d}: empty monthly block"));
    }
    if mo.windows(2).any(|w| w[1] != nx(w[0])) {
        return Err(format!("{d}: months are not consecutive"));
    }
    let vintage = l[..h].iter().find(|x| x.contains("CRSP database")).map_or("", |x| x.trim()).to_string();
    Ok(Rf { mo, rf: v, sha: sha256(p)?, vintage })
}

/// The month after YYYYMM.
pub fn nx(m: i64) -> i64 {
    let (y, k) = (m / 100, m % 100);
    if k == 12 { (y + 1) * 100 + 1 } else { y * 100 + k + 1 }
}

/// Months a..b inclusive as YYYYMM integers.
pub fn mseq(a: i64, b: i64) -> Vec<i64> {
    let mut o = vec![a];
    while *o.last().unwrap() < b {
        o.push(nx(*o.last().unwrap()));
    }
    o
}

// D3 ------------------------------------------------------------------------------------------

pub const COLS: [&str; 10] = ["PERMNO", "PERMCO", "DATE", "SHRCD", "EXCHCD", "PRC", "SHROUT", "RET", "DLSTCD", "DLRET"];
const MISS: [&str; 18] =
    ["", ".", "A", "B", "C", "D", "E", "P", "S", "T", "-66", "-77", "-88", "-99", "-66.0", "-77.0", "-88.0", "-99.0"];

/// D3 rows of a CRSP monthly extract, sorted by (month, PERMNO); missing values are NaN with
/// their CRSP code kept in `retc` and `dlretc`.
#[derive(Clone, Debug, Default)]
pub struct Crsp {
    pub mo: Vec<i64>,
    pub permno: Vec<i64>,
    pub permco: Vec<i64>,
    pub shrcd: Vec<i64>,
    pub exchcd: Vec<i64>,
    pub dl: Vec<i64>,
    pub prc: Vec<f64>,
    pub shrout: Vec<f64>,
    pub ret: Vec<f64>,
    pub dlret: Vec<f64>,
    pub retc: Vec<String>,
    pub dlretc: Vec<String>,
    pub sha: String,
    pub dropped: usize,
}

/// A CRSP numeric cell: (value, missing code). CRSP letter codes mark missing.
fn cell(x: &str) -> Result<(f64, String), ()> {
    let x = x.trim();
    let u = x.to_uppercase();
    if MISS.contains(&u.as_str()) {
        return Ok((f64::NAN, if u.is_empty() { "blank".into() } else { u }));
    }
    match x.parse::<f64>() {
        Ok(v) if v.is_finite() => Ok((v, String::new())),
        _ => Err(()),
    }
}

/// An integer cell, without silently truncating a decimal.
fn integer(x: &str) -> Result<i64, ()> {
    match x.trim().parse::<f64>() {
        Ok(v) if v.is_finite() && v.fract() == 0.0 && v.abs() < 9.2e18 => Ok(v as i64),
        _ => Err(()),
    }
}

fn date(d: &str) -> Option<i64> {
    let d: String = d.trim().chars().filter(|&c| c != '-' && c != '/').collect();
    if d.len() != 8 || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (y, m, k): (i64, i64, i64) = (d[..4].parse().ok()?, d[4..6].parse().ok()?, d[6..].parse().ok()?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    ((1..=12).contains(&m) && y >= 1 && k >= 1 && k <= days[m as usize - 1]).then_some(y * 100 + m)
}

/// D3 rows of a CRSP monthly stock extract with delisting fields (legacy SIZ layout).
///
/// The header must hold exactly COLS (any case, any order; MTHCALDT for DATE); any other field
/// is refused. Nothing missing is read as zero. Share codes outside shr are dropped after
/// reading (Bessembinder, 2018: 10, 11 and 12). Errors name the row, never its contents.
pub fn crsp(p: &Path, shr: &[i64]) -> Result<Crsp, String> {
    let d = p.display();
    let mut rd =
        csv::ReaderBuilder::new().has_headers(false).flexible(true).from_path(p).map_err(|e| format!("{d}: {e}"))?;
    let mut it = rd.records();
    let hd = it.next().ok_or(format!("{d}: empty extract"))?.map_err(|e| format!("{d}: {e}"))?;
    let hd: Vec<String> = hd
        .iter()
        .map(|c| c.trim().trim_start_matches('\u{feff}').to_uppercase())
        .map(|c| if c == "MTHCALDT" { "DATE".into() } else { c })
        .collect();
    let mut ix = [usize::MAX; 10];
    for (k, c) in COLS.iter().enumerate() {
        let f: Vec<usize> = (0..hd.len()).filter(|&j| hd[j] == *c).collect();
        if f.len() == 1 {
            ix[k] = f[0];
        }
    }
    if hd.len() != COLS.len() || ix.contains(&usize::MAX) {
        let ex: Vec<_> = hd.iter().filter(|c| !COLS.contains(&c.as_str())).collect();
        let ms: Vec<_> = COLS.iter().filter(|c| !hd.contains(&c.to_string())).collect();
        return Err(format!("{d}: fields must be exactly {COLS:?}; unread {ex:?}, missing {ms:?}"));
    }
    let mut o = Crsp::default();
    for (k, r) in it.enumerate() {
        let r = r.map_err(|_| format!("{d}: row {} unreadable", k + 2))?;
        let bad = |c: &str| format!("{d}: row {} invalid {c}", k + 2);
        if r.len() != COLS.len() {
            return Err(format!("{d}: row {} has {} fields", k + 2, r.len()));
        }
        let g = |c: usize| &r[ix[c]];
        for (c, v) in [(0, &mut o.permno), (1, &mut o.permco), (3, &mut o.shrcd), (4, &mut o.exchcd)] {
            let x = integer(g(c)).map_err(|_| bad(COLS[c]))?;
            if c < 2 && x <= 0 {
                return Err(bad(COLS[c]));
            }
            v.push(x);
        }
        o.mo.push(date(g(2)).ok_or_else(|| bad("DATE"))?);
        for (c, v) in [(5, &mut o.prc), (6, &mut o.shrout), (7, &mut o.ret), (9, &mut o.dlret)] {
            let (x, code) = cell(g(c)).map_err(|_| bad(COLS[c]))?;
            if ((c == 7 || c == 9) && x < -1.0) || (c == 6 && x < 0.0) {
                return Err(bad(COLS[c]));
            }
            v.push(x);
            if c == 7 {
                o.retc.push(code);
            } else if c == 9 {
                o.dlretc.push(code);
            }
        }
        let x = g(8).trim();
        let dl = if x.is_empty() || x == "." { 0 } else { integer(x).map_err(|_| bad("DLSTCD"))? };
        if dl < 0 {
            return Err(bad("DLSTCD"));
        }
        o.dl.push(dl);
    }
    let mut k: Vec<usize> = (0..o.mo.len()).filter(|&j| shr.contains(&o.shrcd[j])).collect();
    o.dropped = o.mo.len() - k.len();
    if k.is_empty() {
        return Err(format!("{d}: no rows with the requested share codes"));
    }
    k.sort_by_key(|&j| (o.mo[j], o.permno[j]));
    let dup = k.windows(2).filter(|w| o.mo[w[0]] == o.mo[w[1]] && o.permno[w[0]] == o.permno[w[1]]).count();
    if dup > 0 {
        return Err(format!("{d}: {dup} repeated (PERMNO, month) rows"));
    }
    let mut c = o.pick(&k);
    c.sha = sha256(p)?;
    c.dropped = o.dropped;
    Ok(c)
}

impl Crsp {
    /// The rows k, in that order.
    pub fn pick(&self, k: &[usize]) -> Crsp {
        fn g<T: Clone>(v: &[T], k: &[usize]) -> Vec<T> {
            k.iter().map(|&j| v[j].clone()).collect()
        }
        Crsp {
            mo: g(&self.mo, k),
            permno: g(&self.permno, k),
            permco: g(&self.permco, k),
            shrcd: g(&self.shrcd, k),
            exchcd: g(&self.exchcd, k),
            dl: g(&self.dl, k),
            prc: g(&self.prc, k),
            shrout: g(&self.shrout, k),
            ret: g(&self.ret, k),
            dlret: g(&self.dlret, k),
            retc: g(&self.retc, k),
            dlretc: g(&self.dlretc, k),
            sha: self.sha.clone(),
            dropped: self.dropped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, body: &str) -> PathBuf {
        let d = root().join("target").join("test-data");
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(name);
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn orlib_pins_and_shapes() {
        let p = port(1, None).unwrap();
        assert_eq!((p.n, p.s.shape()), (31, (31, 31)));
        assert_eq!(portef(1, None).unwrap().ef.len(), 2000);
        let d = indtrack(1, None, false).unwrap();
        assert_eq!((d.n, d.t, d.drop), (31, 290, 0));
        let (r, ri) = d.lr();
        assert_eq!((r.shape(), ri.len()), ((290, 31), 290));
        assert!(pinned("port1").is_ok());
    }

    #[test]
    fn trailing_indtrack_series_refused_unless_asked() {
        assert!(indtrack(7, None, false).is_err());
        let d = indtrack(7, None, true).unwrap();
        assert_eq!((d.n, d.drop), (1318, d.t + 1));
        let p = tmp("indtrack3.txt", "1 1\n1 1\n2 2\n3 3\n");
        assert!(indtrack(3, Some(&p), true).is_err());
    }

    #[test]
    fn malformed_files_refused() {
        for (n, b) in [
            ("p1.txt", "2\n0.1 0.2\n0.1 0.3\n1 1 1\n1 2 0.5\n"),
            ("p2.txt", "2\n0.1 0.2\n0.1 0.3\n1 1 1\n1 2 0.5\n1 2 0.5\n2 2 1\n"),
            ("p3.txt", "2\n0.1 0.2\n0.1 0.3\n1 1 1\n1 2 1.5\n2 2 1\n"),
            ("p4.txt", "2.5\n"),
            ("p5.txt", ""),
        ] {
            assert!(port(1, Some(&tmp(n, b))).is_err(), "{n}");
        }
        assert!(port(1, Some(&tmp("p6.txt", "2\n0.1 0.2\n0.1 0.3\n1 1 1\n1 2 0.5\n2 2 1\n"))).is_ok());
        assert!(portef(1, Some(&tmp("e1.txt", ""))).is_err());
        assert!(portef(1, Some(&tmp("e2.txt", "0.1 0.2 0.3"))).is_err());
        assert!(portef(1, Some(&tmp("e3.txt", "0.1 -0.2"))).is_err());
        assert!(indtrack(1, Some(&tmp("i1.txt", "1.5 1\n1 1 1 1\n")), false).is_err());
        assert!(indtrack(1, Some(&tmp("i2.txt", "1 1\n1 1\n2 0\n")), false).is_err());
        assert!(indtrack(1, Some(&tmp("i3.txt", "1 1\n1 1\n2\n")), false).is_err());
    }

    #[test]
    fn crsp_missing_stays_missing_and_extra_field_refused() {
        let h = "permno,PERMCO,mthcaldt,SHRCD,EXCHCD,PRC,SHROUT,RET,DLSTCD,DLRET\n";
        let p = tmp(
            "c1.csv",
            &format!(
                "{h}10001,1,1990-01-31,11,1,-10.5,100,C,,\n10001,1,19900228,11,1,11,100,0.05,550,-0.3\n10002,2,19900131,31,1,5,10,0.1,,\n"
            ),
        );
        let c = crsp(&p, &[10, 11, 12]).unwrap();
        assert_eq!((c.mo.clone(), c.dropped), (vec![199001, 199002], 1));
        assert!(c.ret[0].is_nan() && c.retc[0] == "C" && c.dlret[0].is_nan() && c.dlretc[0] == "blank");
        assert_eq!((c.dl[1], c.dlret[1], c.prc[0]), (550, -0.3, -10.5));
        let p = tmp("c2.csv", &format!("{},X\n10001,1,19900131,11,1,1,1,0.1,,,1\n", h.trim_end()));
        assert!(crsp(&p, &[11]).unwrap_err().contains("unread"));
        let p = tmp("c3.csv", &format!("{h}10001,1,19900230,11,1,1,1,0.1,,\n"));
        assert!(crsp(&p, &[11]).unwrap_err().contains("row 2 invalid DATE"));
        let p = tmp("c4.csv", &format!("{h}10001,1,19900131,11,1,1,1,-1.5,,\n"));
        let e = crsp(&p, &[11]).unwrap_err();
        assert!(e.contains("invalid RET") && !e.contains("-1.5"));
        let p = tmp("c5.csv", &format!("{h}10001.5,1,19900131,11,1,1,1,0.1,,\n"));
        assert!(crsp(&p, &[11]).is_err());
        let p = tmp("c6.csv", &format!("{h}10001,1,19900131,11,1,1,1,0.1,,\n10001,1,19900115,11,1,1,1,0.1,,\n"));
        assert!(crsp(&p, &[11]).unwrap_err().contains("repeated"));
    }

    #[test]
    fn french_rf_parser() {
        let b = "This file was created using the 202608 CRSP database.\n\n,Mkt-RF,SMB,HML,RF\n199001, 1, 2, 3, 0.57\n199002, 1, 2, 3, 0.57\n\n Annual\n";
        let r = rf(&tmp("rf1.csv", b)).unwrap();
        assert!(r.mo == [199001, 199002] && (r.rf[0] - 0.0057).abs() < 1e-15);
        assert!(r.vintage.contains("202608"));
        assert!(rf(&tmp("rf2.csv", ",Mkt-RF,SMB,HML,RF\n199001, 1, 2, 3, 0.5\n199003, 1, 2, 3, 0.5\n")).is_err());
        assert!(rf(&tmp("rf3.csv", ",Mkt-RF,SMB,HML,RF\n\n")).is_err());
        assert!(rf(&tmp("rf4.csv", "no header\n")).is_err());
        assert!(rf(&tmp("rf5.csv", ",Mkt-RF,SMB,HML,RF\n199013, 1, 2, 3, 0.5\n")).is_err());
        assert_eq!(mseq(199011, 199102), [199011, 199012, 199101, 199102]);
    }
}
