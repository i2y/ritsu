"""Build the two sets of rounding cases from the tool's own generated output.

`rulec gen` writes a unit test of the rounding helpers beside every language (`_round_test.py`
in the Python directory). Its CASES are the vectors the tool holds every backend to, so they
are the right thing to hold a tenth one to. The stress set is generated from the same
reference implementations, over values the standard set does not reach: int64 at both ends,
2^53 either side, and grids that are neither a power of ten nor a factor of anything.
"""

import json
import pathlib
import random
import sys

gen = pathlib.Path(sys.argv[1])
out = pathlib.Path(sys.argv[2])
src = (gen / "python" / "_round_test.py").read_text()

ns: dict = {}
exec(src[: src.index("CASES = [")].replace("import sys", ""), ns)  # noqa: S102
FN = {
    "down": ns["_round_down"],
    "up": ns["_round_up"],
    "half": ns["_round_half"],
    "half_down": ns["_round_half_down"],
    "bankers": ns["_round_bankers"],
}

# --- the tool's own vectors, read back out of the file it wrote
start = src.index("CASES = [")
cases = eval(src[start + len("CASES = ") : src.index("]", start) + 1])  # noqa: S307
json.dump({"cases": [list(c) for c in cases]}, (out / "cases.json").open("w"))

# --- the stress set: same reference implementations, harder inputs
MAX = 2**63 - 1
random.seed(7)
xs = set()
for base in (0, 2**31, 2**52, 2**53, 2**62, MAX):
    for d in range(-4, 5):
        xs.add(base + d)
        xs.add(-(base + d))
for _ in range(400):
    xs.add(random.randint(-MAX, MAX))
xs = sorted(x for x in xs if -MAX <= x <= MAX)

stress = [
    [m, x, g, FN[m](x, g)]
    for g in (1, 2, 3, 5, 7, 10, 20, 100, 1000, 999983, 1000000007)
    for x in xs
    for m in FN
]
json.dump({"cases": stress}, (out / "stress.json").open("w"))

vs = [json.loads(line) for line in (gen / "vectors" / "ec261.jsonl").read_text().splitlines()]
json.dump({"vectors": vs}, (out / "ec261_vectors.json").open("w"))

print(f"{len(cases)} {len(stress)} {max(abs(c[1]) for c in stress)} {len(vs)}")
