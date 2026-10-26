"""Explicit ctypes boundary for the optional Rust holding kernel."""

import ctypes as ct
from functools import lru_cache
import hashlib
import os
from pathlib import Path

import numpy as np

# Legend: lib = source-matched shared library; R = holding returns (H,N);
# rf = T-bill returns; ls = relative delisting indices; w = initial weights;
# v = asset account values (H,N); c = cash account values (H,).
ROOT = Path(__file__).resolve().parent.parent / "native"
F = np.ctypeslib.ndpointer(dtype=np.float64, flags=("C_CONTIGUOUS", "ALIGNED"))
I = np.ctypeslib.ndpointer(dtype=np.int64, flags=("C_CONTIGUOUS", "ALIGNED"))


@lru_cache(maxsize=4)
def _load(path, source):
    lib = ct.CDLL(path)
    lib.pf_source.argtypes = []
    lib.pf_source.restype = ct.c_char_p
    if lib.pf_source().replace(b"\r\n", b"\n") != source:
        raise RuntimeError("Rust library source mismatch; rebuild native/Cargo.toml")
    lib.pf_hold.argtypes = [F, F, I, F, ct.c_size_t, ct.c_size_t, F, F]
    lib.pf_hold.restype = ct.c_int
    return lib


def library():
    path = os.environ.get("PF_NATIVE")
    if not path:
        raise RuntimeError("Rust backend requires PF_NATIVE pointing to the built shared library")
    source = b"".join((ROOT / p).read_bytes().replace(b"\r\n", b"\n")
                      for p in ("src/lib.rs", "Cargo.toml"))
    return _load(str(Path(path).resolve()), source)


def identity():
    """Verify the loaded source and record the binary used for a result bundle."""
    library()
    return hashlib.sha256(Path(os.environ["PF_NATIVE"]).read_bytes()).hexdigest()


def hold(R, rf, ls, w):
    R, rf, w = (np.require(x, dtype=np.float64, requirements=["C", "A"]) for x in (R, rf, w))
    ls = np.require(ls, dtype=np.int64, requirements=["C", "A"])
    if (R.ndim != 2 or 0 in R.shape or rf.shape != (R.shape[0],)
            or w.shape != (R.shape[1],) or ls.shape != w.shape):
        raise ValueError("native holding arrays disagree in shape")
    v, c = np.empty_like(R), np.empty(R.shape[0])
    st = library().pf_hold(R, rf, ls, w, *R.shape, v, c)
    if st == 1:
        raise ValueError("missing return while still listed or without a confirmed delisting")
    if st != 0:
        raise RuntimeError(f"Rust holding kernel failed: {st}")
    # NumPy retains its own reduction order, including pairwise sums for large K.
    return v.sum(axis=1) + c
