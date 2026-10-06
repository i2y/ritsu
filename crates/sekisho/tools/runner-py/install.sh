#!/bin/sh
# The Python tests/python.rs runs the generated Python with: a virtual environment at
# tools/runner-py/.venv holding the packages of requirements.txt, each to its hash (cedarpy, boto3
# and its stubs, mypy), made by uv with Python 3.13. Run again, it brings the environment to the
# lock. Nothing of it is in git.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
uv venv --allow-existing --python 3.13 "$here/.venv"
uv pip sync --python "$here/.venv/bin/python" --require-hashes "$here/requirements.txt"
"$here/.venv/bin/python" -c 'import cedarpy, boto3; print("cedarpy and boto3", boto3.__version__)'
"$here/.venv/bin/mypy" --version
