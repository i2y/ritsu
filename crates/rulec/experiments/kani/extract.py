"""Pull the row conditions out of the Rust rulec generated, as a prototype of the emitter.

Each table lowers to one if / else-if chain whose branch conditions are the full row
conditions (not residuals, in `first` too), so the row predicates read straight back out.
The `let`s ahead of a table are its derived values and definitions; the ones after it
belong to later tables and are left alone.
"""
import re, sys

src = open(sys.argv[1]).read()
fn = re.search(r"pub fn (\w+)_traced\(([^)]*)\)", src)
name, params = fn.group(1), fn.group(2)
lines = src[fn.end():].splitlines()

lets, tables, cur = [], [], None
for i, line in enumerate(lines):
    m = re.match(r"\s*(let \w+ = .+?;) // (?:derived value|definition)$", line)
    if m:
        lets.append((i, m.group(1)))
        continue
    m = re.match(r"\s*// table (\S+) \(policy (\w+)\)", line)
    if m:
        cur = {"name": m.group(1), "policy": m.group(2), "rows": [], "at": i}
        tables.append(cur)
        continue
    m = re.match(r"\s*\}? ?else if (.+?) \{ // row (\d+)(?: \([^)]*\))?:", line) \
        or re.match(r"\s*if (.+?) \{ // row (\d+)(?: \([^)]*\))?:", line)
    if m and cur is not None:
        cur["rows"].append((int(m.group(2)), m.group(1)))

for t in tables:
    print(f"// table {t['name']} (policy {t['policy']}), {len(t['rows'])} rows")
    print(f"pub fn rows_{t['name']}_{name}({params}) -> u32 {{")
    for i, l in lets:
        if i < t["at"]:
            print(f"    {l}")
    print("    0")
    for n, cond in t["rows"]:
        print(f"      + (({cond}) as u32)  // row {n}")
    print("}")
