"""Shared fixtures and path discovery for the xgboost-autoresearch test suite.

Discovers repo paths robustly and injects the harness + runtime source onto
sys.path so tests can import from ``harness.*`` and ``nemoir_runtime``
regardless of the working directory or installed packages.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

# ---- Repo-root and path setup ------------------------------------------------

DEMO_ROOT = Path(__file__).resolve().parents[1]
HARNESS_DIR = DEMO_ROOT / "harness"
REPO_ROOT = DEMO_ROOT.parents[1]  # nemo-ir/
RUNTIME_SRC = REPO_ROOT / "python" / "nemoir-runtime" / "src"
NEMO_BIN = REPO_ROOT / "compiler" / "target" / "release" / "nemo"
NEMO_BIN_DEBUG = REPO_ROOT / "compiler" / "target" / "debug" / "nemo"
NEMO_BIN_LINK = DEMO_ROOT.parent / "nemo"  # symlink at demos/nemo


def _nemo_binary() -> Path | None:
    """Return the first available nemo binary, or None."""
    for candidate in (NEMO_BIN, NEMO_BIN_DEBUG, NEMO_BIN_LINK):
        if candidate.exists() and os.access(candidate, os.X_OK):
            return candidate
    return None


NEMO_PATH = _nemo_binary()


def _setup_sys_path() -> None:
    for p in (str(HARNESS_DIR), str(RUNTIME_SRC)):
        if p not in sys.path:
            sys.path.insert(0, p)


_setup_sys_path()


# ---- Pytest hooks -----------------------------------------------------------


def pytest_configure(config: pytest.Config) -> None:  # type: ignore[reportUnknownParameterType]
    config.addinivalue_line(  # type: ignore[reportUnknownMemberType]
        "markers",
        "needs_nemo: test requires the compiled nemo binary",
    )
    config.addinivalue_line(  # type: ignore[reportUnknownMemberType]
        "markers",
        "needs_numpy: test requires numpy",
    )
    config.addinivalue_line(  # type: ignore[reportUnknownMemberType]
        "markers",
        "needs_sklearn: test requires scikit-learn",
    )
