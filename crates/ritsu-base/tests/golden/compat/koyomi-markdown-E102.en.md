<a id="e102"></a>

## E102 — A source is not pinned

**When**: A table's line, or the pin line of a law's article, has no `sha256:`.

**Fix**: Pin it with the first 16 digits of the copy's SHA-256; the fix gives the line with the copy's own.

**Example**:

```cal
calendar t v1

source 休み = file "holidays.csv"
  format csv
  covers 2026-01-01..2026-12-31

closed 休み
```

`holidays.csv`:

```
2026-01-01,元日
2026-05-04,みどりの日
```

See also: [E101](#e101), [E103](#e103)
