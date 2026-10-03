#!/bin/sh
# Stand τ²-bench up beside this directory with the airline_rulec domain registered.
#   sh setup.sh            clones sierra-research/tau2-bench into ./tau2-bench, installs it
#                          into ./tau2-bench/.venv, generates the three rules' Python into
#                          ./gen, puts tau2_rulec on the path, and registers the domain.
set -e
cd "$(dirname "$0")"
HERE=$(pwd)
RULEC=${RULEC:-../../target/release/rulec}
CORPUS=${CORPUS:-../../tests/corpus}
[ -d tau2-bench ] || git clone --depth 1 https://github.com/sierra-research/tau2-bench.git tau2-bench
rm -rf gen
$RULEC gen "$CORPUS/預け荷物料金.rule" "$CORPUS/予約取消可否.rule" "$CORPUS/補償証明書.rule" --out gen > /dev/null
cd tau2-bench
[ -d .venv ] || uv venv -q -p 3.12 .venv
uv pip install -q --python .venv/bin/python -e . websockets
SP=$(.venv/bin/python -c "import site; print(site.getsitepackages()[0])")
printf '%s\n' "$HERE" > "$SP/tau2_rulec.pth"
grep -q tau2_rulec src/tau2/registry.py || cat >> src/tau2/registry.py <<'PY'

# --- the airline domain with the rulec-generated policy tools (tau2_rulec, on the path via a .pth)
try:
    from tau2_rulec.environment import register as _rulec_register

    _rulec_register(registry)
    logger.debug("Registered airline_rulec")
except Exception as _e:  # noqa: BLE001
    logger.warning(f"tau2_rulec not registered: {_e}")
PY
.venv/bin/python "$HERE/smoke.py" 2>&1 | grep -v "DEBUG\|INFO\|WARNING"
