PYTHON ?= python
# `nemo` expected on PATH (or set NEMO=/path/to/nemo). Fallback example:
#   make NEMO="cargo run --manifest-path ../../../compiler/Cargo.toml -p nemoir-cli --"
NEMO ?= nemo

.PHONY: check compile prepare test

check:
	$(NEMO) check autoresearch.nemo

compile:
	$(NEMO) compile autoresearch.nemo --target python -o . --dump-ir

prepare:
	$(PYTHON) harness/data.py prepare

test:
	$(PYTHON) -m pytest
