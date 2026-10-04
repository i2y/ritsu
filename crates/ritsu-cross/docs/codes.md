# Diagnostic codes

Written by `ritsu explain --all --format markdown`; do not edit.

<a id="e101"></a>

## E101 — A file that does not read as a .proto

**When**: A `.proto` of the project that ritsu's one reader of `.proto` files (ritsu-proto) cannot read: a `{` that is never closed, a statement without its `;`, a `syntax` it does not know, a proto2 `group`, a file that is not UTF-8. Every language reads a `.proto` with that reader, so none of them can read the file; a language that reads it says so where it does, in its own code too (rulec's E013, dandori's E016, sakai's E106, yuen's E205).

**Fix**: Correct it where it points, as a proto3 `.proto`; a file `buf build` builds, ritsu's reader reads.

**Reproduction**: put the files below in one directory, and run `ritsu check .` there.

`shop.proto`:

```proto
syntax = "proto3";

package shop.v1;

message Order {
  string id = 1;
```
