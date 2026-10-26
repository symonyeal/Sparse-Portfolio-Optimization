"""Atomic result bundles with code, parameters, input hashes and host identity."""

import hashlib
import json
import math
import os
import platform
import re
import subprocess
import sys
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CODE = ("pf",)
CONFIG = ("run_bench.py", "run_boot.py", "run_oos.py", "pyproject.toml", "requirements.txt",
          "data/orlib/SHA256", "native/Cargo.toml", "native/Cargo.lock", "native/src/lib.rs")

# Legend
# key : case key; pa : case parameters; r : result record; ip : {input file: sha256}
# b   : bundle; out : results directory


def _sha(p):
    return hashlib.sha256(Path(p).read_bytes().replace(b"\r\n", b"\n")).hexdigest()


def digest(x):
    """SHA-256 of a JSON-serialisable value, keys sorted."""
    raw = json.dumps(plain(x), sort_keys=True, allow_nan=False).encode()
    return hashlib.sha256(raw).hexdigest()


def source(root=ROOT):
    """SHA-256 over (path, sha256) of every executed source and configuration file."""
    root = Path(root)
    f = sorted(p for d in CODE for p in (root / d).rglob("*.py") if "__pycache__" not in p.parts)
    f += [root / c for c in CONFIG if (root / c).exists()]
    return digest([[p.relative_to(root).as_posix(), _sha(p)] for p in f])


def _git(root, *args):
    """Successful Git output, or None when Git cannot establish the requested fact."""
    try:
        r = subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True,
                           check=False)
    except OSError:
        return None
    return r.stdout.strip() if r.returncode == 0 else None


def committed(commit, root=ROOT):
    """Whether commit names a full, existing commit object in this repository."""
    if not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", commit):
        return False
    top = _git(root, "rev-parse", "--show-toplevel")
    return (top is not None and Path(top).resolve() == Path(root).resolve()
            and _git(root, "cat-file", "-t", commit) == "commit")


def identity(root=ROOT):
    """Full commit, dirty flag (results and prose excluded), and source digest."""
    commit = _git(root, "rev-parse", "--verify", "HEAD^{commit}")
    d = _git(root, "status", "--porcelain", "--untracked-files=all", "--",
             ".", ":!docs/results", ":!*.md")
    good = committed(commit, root)
    return {"commit": commit if good else None,
            "dirty": not good or d is None or bool(d), "source": source(root)}


def host():
    """Machine and library versions a result was produced on."""
    import numpy
    import scipy
    v = {"python": sys.version.split()[0], "numpy": numpy.__version__, "scipy": scipy.__version__}
    try:
        import pyscipopt
        v["pyscipopt"] = pyscipopt.__version__
        v["scip"] = pyscipopt.Model().version()
    except ImportError:
        pass
    return {"node": platform.node(), "cpu": platform.processor(), "os": platform.platform(), **v}


def plain(x):
    """JSON values with numpy converted and unavailable nonfinite numbers represented by null."""
    if hasattr(x, "tolist"):
        return plain(x.tolist())
    if isinstance(x, dict):
        return {k: plain(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [plain(v) for v in x]
    if isinstance(x, float) and not math.isfinite(x):
        return None
    if x is None or isinstance(x, (str, bool, int, float)):
        return x
    raise TypeError(f"unsupported bundle value {type(x).__name__}")


def bundle(key, pa, r, ip, cmd, code=None):
    """The result r of case key as a bundle."""
    return plain({**r, "bundle": {"schema": 1, "key": key, "pa": pa, "code": code or identity(),
                                  "inputs": ip, "command": list(cmd), "host": host()}})


def write(out, b):
    """Atomically replace out/<key>.json after flushing a complete strict JSON record."""
    key = b["bundle"]["key"]
    if not isinstance(key, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", key):
        raise ValueError("bundle key must be a filename stem without path separators")
    raw = (json.dumps(plain(b), indent=1, sort_keys=True, allow_nan=False) + "\n").encode()
    p = Path(out) / f"{key}.json"
    p.parent.mkdir(parents=True, exist_ok=True)
    pending = p.with_name(f".{key}.{uuid.uuid4().hex}.pending")
    with pending.open("xb") as fh:
        fh.write(raw)
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(pending, p)
    return p


def read(p):
    """A strict JSON object, or None for an absent, interrupted or malformed record."""
    def bad(value):
        raise ValueError(f"nonstandard JSON constant {value}")

    try:
        b = json.loads(Path(p).read_text(encoding="utf-8"), parse_constant=bad)
    except (OSError, UnicodeError, ValueError):
        return None
    return b if isinstance(b, dict) else None


def valid(out, key, pa, src=None, ip=None, root=ROOT):
    """A clean committed bundle matching current sources, parameters and expected inputs."""
    if ip is None:
        return None
    p = Path(out) / f"{key}.json"
    b = read(p)
    if b is None:
        return None
    m = b.get("bundle", {})
    if not isinstance(m, dict) or not isinstance(m.get("code"), dict):
        return None
    c = m["code"]
    if (m.get("schema") != 1 or m.get("key") != key or c.get("dirty") is not False
            or c.get("source") != (src or source(root)) or m.get("pa") != plain(pa)
            or m.get("inputs") != plain(ip) or not committed(c.get("commit"), root)):
        return None
    return b
