# A command whose output changes from one run to the next, for a `run` that waits
# with `within` (DESIGN §18): each run adds one to the count in `tick.count` in the
# directory it runs in, and prints "running" before the third run and "done" from
# the third on. Run as `python3 tick.py`.
import os

path = "tick.count"
n = int(open(path).read()) + 1 if os.path.exists(path) else 1
with open(path, "w") as f:
    f.write(str(n))
print("done" if n >= 3 else "running")
