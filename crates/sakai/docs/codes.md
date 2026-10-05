# Diagnostic codes

Written by `sakai explain --all --format markdown`; do not edit.

<a id="e001"></a>

## E001 — Something cannot be read as a word of the language

**When**: A string not closed, an escape a string does not take, a full-width space, or a character a name cannot have. A name is letters, digits and `_`, and does not start with a digit.

**Fix**: Correct it where it points: close the string with `"` on the same line, and use no escape but `\"` and `\\`.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
description "not closed
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E002](#e002)

<a id="e002"></a>

## E002 — A word is written where it does not belong

**When**: The syntax does not take the word there: an `upstream` with no role, a keyword as a name, a heading without its version, a word left over at the end of a line, and the like.

**Fix**: Use one of the forms the note gives.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E004](#e004)

<a id="e003"></a>

## E003 — The file does not start with a `map` or a `context` line

**When**: The first line that is not a comment or blank is neither `map …` nor `context …`.

**Fix**: Start a context map like `map Shop(shop) v1`, and one context like `context Inventory(inventory) v1`.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E004](#e004)

<a id="e004"></a>

## E004 — A section is out of order, repeated, missing, or in the wrong kind of file

**When**: Sections are out of their order, a section that comes once comes twice, one that cannot be left out is missing (a map's `use context` and `covers`, a context's `owns`, the `through` under `upstream`), or a section belongs to the other kind of file (`owns` in a map); also a term with both a definition and `as`, or neither.

**Fix**: A map goes: heading, `description`, `use context`, `covers`, `except`, `proto root`, `code`. A context goes: heading, `description`, `owner`, `also`, `owns`, `published language`, `terms`, then the relationships.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
owner "Alpha's team"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E002](#e002), [E003](#e003)

<a id="e005"></a>

## E005 — The indentation does not line up

**When**: The indentation has a tab, the lines of one block are not indented alike, or an indented line has no line above to take it.

**Fix**: Indent with spaces, every line of a block by the same amount.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
	dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e006"></a>

## E006 — A name is declared twice

**When**: A context's name or alias, a term's name or `also` in one context, a published package, the same relationship toward the same context, an entry of `owns`, or the left side of a mapping comes twice.

**Fix**: Rename one of them, or delete the second.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Alpha(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E007](#e007)

<a id="e007"></a>

## E007 — A name is not declared

**When**: A relationship or an `as` names a context the map does not read, or `means` or a mapping names an element the proto does not have; also a short name that two packages have.

**Fix**: Correct the spelling, or add the `use context`. Write a name two packages have with its package (`message warehouse.v1.ReserveResponse`).

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Gamma conformist
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E006](#e006), [E410](#e410)

<a id="e008"></a>

## E008 — A map or a context has no ASCII alias

**When**: The heading's name has no `(alias)` right after it, or the alias is not of the form `[A-Za-z_][A-Za-z0-9_]*`. The alias names it in CML and in the settings of the import linters.

**Fix**: Write it in parentheses right after the name, like `Ordering(ordering)`.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e009"></a>

## E009 — A path written is not there

**When**: A path of `use context`, `covers`, `except`, `proto root`, `code`, `owns`, a published language's `proto`, `crate` and `generated dir`, `layer`, a shared kernel or `means` is not on the disk. A path counts from the directory of the file it is in.

**Fix**: Correct the path.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a", "nowhere"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E012](#e012)

<a id="e010"></a>

## E010 — What `use context` names cannot be used

**When**: What `use context` names is a map file, or the same file is named twice.

**Fix**: Name each context file once.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
use context "alpha.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e011"></a>

## E011 — The name of an artifact is not of the right form

**When**: The tool and the extension do not agree, a `dir` or a published language's `crate` is a file or a tool's file a directory, a tool or a kind is not one the tool has, a child kind (`value`, `field`, `method`) does not come right after its parent, or a `file` is given a kind; also a mapping's target that is not an enum.

**Fix**: Write it in the form `<tool> "<path>" [<kind> <name>]...`.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  rulec "a/a.proto"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E012](#e012)

<a id="e012"></a>

## E012 — A path is absolute, goes outside the root, or is empty

**When**: A path starts with `/`, goes outside the root with `..`, or is written empty, `""`. The root is the nearest directory above the path given to sakai that holds .git (else the directory given; `--root` changes it).

**Fix**: Write a path inside the root, from the directory of the file; that directory itself is written `"."`.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a", "../outside"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E009](#e009)

<a id="e101"></a>

## E101 — An artifact belongs to no context

**When**: An artifact of the scope is held by no entry of any context's `owns`. A directory of such files only is told once, at its top.

**Fix**: Write the directory or the file under the `owns` of a context.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`c/c.proto`:

```proto
syntax = "proto3";
package c;
message C {}
```

See also: [E102](#e102)

<a id="e102"></a>

## E102 — Two contexts own an artifact at the same depth

**When**: The `owns` of two contexts write the same directory or file; an artifact belongs to the context of the deepest entry that holds it, and that cannot be told.

**Fix**: Delete one, or let one of them write a deeper directory.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b", "a"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E101](#e101)

<a id="e103"></a>

## E103 — Something outside the scope is owned or referred to

**When**: An entry of `owns`, what a proto imports, or the proto of a `means` is outside the map's scope: the `covers`, without `except` and without any path with a name that starts with ., node_modules, site-packages, __pycache__ or target in it.

**Fix**: Widen the scope, or delete the entry or the reference.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
except "c"
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a", "c"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`c/c.proto`:

```proto
syntax = "proto3";
package c;
message C {}
```

<a id="e104"></a>

## E104 — Another language's artifacts cannot be read: the language is not joined

**When**: The map holds rulec, koyomi or dandori artifacts, and the ports of ritsu that read that language are not joined (the binary of sakai's own crate). Told once a language, at the line of `owns` that holds its first artifact. What is not checked is not passed in silence. The exit code is 2: it is how the command is run, not what the map says.

**Fix**: Run it as `ritsu sakai`, which joins every language.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/x.rule`:

```rule
rule x v1

import proto "../b/v1/b.proto" Kind -> kind
enum kind = one | two

inputs
  k : kind

outputs
  n : kind

table t
policy unique
| k   | -> n |
| one | one  |
| two | two  |
```

See also: [E105](#e105)

<a id="e105"></a>

## E105 — An artifact does not pass its language's check, or cannot be read

**When**: rulec does not answer for a rule (it does not pass rulec's check), or koyomi or dandori cannot read the file. The notes give what the language says; the references of the artifact cannot be checked.

**Fix**: Fix the artifact until its language's check passes.

**Reproduction**: put the files below in one directory, and run `ritsu sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/fee.rule`:

```rule
rule fee v1

inputs
  amount : money[JPY]  range >=0JPY <=10000JPY

outputs
  fee : money[JPY]  round down(1JPY)

table fees
| amount       | -> fee  |
| <5000JPY     | 500JPY  |
| >5000JPY     | 0JPY    |
```

See also: [E104](#e104)

<a id="e106"></a>

## E106 — A proto cannot be read

**When**: sakai's reader cannot read a `.proto` of the scope: a syntax error, or a proto2 `group`.

**Fix**: Correct it where it points.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = ; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e107"></a>

## E107 — Cargo cannot say the crates of the Rust code

**When**: The map writes `code rust`, and there is no `Cargo.toml` at its place, or `cargo metadata` fails (cargo is not there, a manifest does not read, and the like). sakai reads Rust's crates and their dependencies from Cargo; when it cannot, the dependencies of the crates are not checked.

**Fix**: Write under `code rust` the directory of the workspace's `Cargo.toml` (for one crate, of its own), and see that `cargo metadata --no-deps --offline` passes there.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`shop.ctx`:

```ctx
map Shop(shop) v1
use context "orders.ctx"
covers "."
code rust "."
```

`orders.ctx`:

```ctx
context Orders(orders) v1
owns
  dir "orders"
```

`orders/Cargo.toml`:

```
[package]
name = "orders"
version = "0.1.0"
edition = "2021"
```

`orders/src/lib.rs`:

```
pub fn reserve() {}
```

<a id="e108"></a>

## E108 — An OpenAPI or AsyncAPI document cannot be read

**When**: An OpenAPI or AsyncAPI document of the scope (a `.yaml`, `.yml` or `.json` whose top holds `openapi`, `asyncapi` or `swagger`), or a file a document points at by `$ref`, cannot be read: it does not read as YAML or JSON, it writes YAML that does not go to JSON and back (a tag, a key with `?`, a second document), it is of a version sakai does not read (OpenAPI 2.0, AsyncAPI 2.x), or a `$ref` points at no file or at nothing in one.

**Fix**: Correct it where it points. Convert OpenAPI 2.0 to OpenAPI 3, and AsyncAPI 2.x to AsyncAPI 3 with `asyncapi convert`.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`b/api.yaml`:

```
openapi: 3.1.0
info:
	title: B
  version: 1.0.0
```

See also: [W104](#w104)

<a id="w101"></a>

## W101 — An entry of `owns` holds no artifact

**When**: A directory or file under `owns` holds no artifact; the path is often mistyped.

**Fix**: Correct the path, or delete the entry.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a", "empty"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`empty/README.txt`:

```
nothing here is an artifact
```

<a id="w102"></a>

## W102 — A proto's import is not found

**When**: What a proto imports is in none of the map's `proto root`s, the directory its package places it in, or its own directory. Google's well-known types, `buf/validate` and dandori's `options.proto` are known without their files.

**Fix**: Write a `proto root` in the map, or correct the import; a contract outside the scope may stay so (its types are left out of the check of the references).

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "elsewhere/v1/x.proto";
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="w103"></a>

## W103 — A context file no map reads

**When**: `check` was given a directory, and no map under it names a context file under it with `use context`.

**Fix**: Add it to a map's `use context`, or delete the file.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`gamma.ctx`:

```ctx
context Gamma(c) v1
owns
  dir "a"
```

<a id="w104"></a>

## W104 — A `$ref` points at a URL

**When**: A `$ref` of an OpenAPI or AsyncAPI document points at a URL, like `https://…`. sakai does not go to the network: what it points at is not read, and is taken as outside the scope (its types are left out of the checks of the boundaries).

**Fix**: For another context's document, point at its file in the repository by a relative path; a contract of a system outside may stay so.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`b/api.yaml`:

```
openapi: 3.1.0
info:
  title: B
  version: 1.0.0
components:
  schemas:
    Price:
      $ref: 'https://example.com/schemas/money.yaml#/Money'
```

See also: [E108](#e108)

<a id="n101"></a>

## N101 — The references of dandori are not checked

**When**: Never: it was for a map that holds `.flow` files, when sakai checked only who owns them, dandori printing no references as JSON.

**Fix**: Nothing to fix.

**Reproduction**: Retired in ritsu 0.23.0: dandori's references are read through ritsu's ports (`References`), and those that cross a boundary are checked (E202, E207 to E209).

See also: [E202](#e202), [E207](#e207), [E208](#e208), [E209](#e209)

<a id="e201"></a>

## E201 — A reference to a context there is no relationship with

**When**: A reference crosses to a context with no relationship that allows it: upstream and downstream (written by the downstream), a partnership, or a shared kernel; also when the relationship goes the other way.

**Fix**: Write `upstream <context> <role>` and `through <package>` in the referring context, or delete the reference.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.B b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E202](#e202), [E206](#e206)

<a id="e202"></a>

## E202 — A reference to the inside of another context

**When**: What a reference crosses to is in no published language of the other context and in no shared kernel of the two: a proto or an OpenAPI or AsyncAPI document that is not published, a rule or a calendar itself.

**Fix**: Refer through the other context's published language, or list it in a shared kernel of the two.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta conformist
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/internal/v1/x.proto";
message A { b.internal.v1.X x = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`b/internal/v1/x.proto`:

```proto
syntax = "proto3";
package b.internal.v1;
message X {}
```

See also: [E201](#e201)

<a id="e203"></a>

## E203 — A reference goes through a package `through` does not list

**When**: A reference reaches a published package of the upstream that the relationship's `through` does not list.

**Fix**: Add the package to `through`, or change the reference.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta conformist
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

published language b.v2
  proto "b/v2/b2.proto"

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v2/b2.proto";
message A { b.v2.B2 b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`b/v2/b2.proto`:

```proto
syntax = "proto3";
package b.v2;
message B2 {}
```

See also: [E312](#e312)

<a id="e204"></a>

## E204 — A reference to the upstream's published language from outside the anticorruption layer

**When**: An anticorruption layer has a `layer`, and an artifact outside it refers to the upstream's published language.

**Fix**: Move the reference into the layer, or add it to `layer`.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta anticorruption layer
  through b.v1
  layer dir "a/acl"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.Plain p = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/acl/v1/acl.proto`:

```proto
syntax = "proto3";
package a.acl.v1;
message Seen {}
```

See also: [E205](#e205)

<a id="e205"></a>

## E205 — The published language of a layer's downstream shows the upstream's types

**When**: Downstream of an anticorruption layer, a proto of the downstream's own published language imports the upstream's published language (or a document of it points at the upstream's documents by `$ref`): the upstream's model goes out past the layer.

**Fix**: Map it to the downstream's own types in the layer, and publish those only.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

published language a.v1
  proto "a/v1/a1.proto"

upstream Beta anticorruption layer
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/v1/a1.proto`:

```proto
syntax = "proto3";
package a.v1;
import "b/v1/b.proto";
message A1 { b.v1.Plain p = 1; }
```

See also: [E204](#e204)

<a id="e206"></a>

## E206 — A reference to a context it goes separate ways from

**When**: There is a reference across the boundary between two contexts that go separate ways.

**Fix**: Delete the reference, or replace separate ways with a relationship.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

separate ways from Beta
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.Plain p = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E310](#e310)

<a id="e207"></a>

## E207 — A workflow calls a service that is no open host service of the other side

**When**: A workflow calls a service across a boundary with `connect`, or a rule with `use rule … connect`, and the service is not under `open host service` of the other side's published language.

**Fix**: The other side lists the service under `open host service`, or the workflow calls one it lists.

**Reproduction**: put the files below in one directory, and run `ritsu sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta conformist
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/w.flow`:

```flow
workflow w v1

use proto b from "../b/v1/b.proto"
  url "https://b.example.com"

inputs
  kind : b.Kind

task get(kind: b.Kind) -> b.B
  connect b "BService/Get"

flow
  let r = get(kind: kind)
  succeed
```

See also: [E202](#e202), [E301](#e301)

<a id="e208"></a>

## E208 — The service a workflow implements is not in its own published language

**When**: The service a workflow `implements` is not under `open host service` of a published language of the workflow's context (nor when the .proto is in no published language).

**Fix**: List the .proto in the context's published language, and the service under `open host service`.

**Reproduction**: put the files below in one directory, and run `ritsu sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

published language a.v1
  proto "a/v1/a.proto"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/v1/a.proto`:

```proto
syntax = "proto3";
package a.v1;
import "dandori/v1/options.proto";
service AService {
  option (dandori.v1.workflow) = {name: "w", version: 1};
  rpc Start(StartRequest) returns (StartResponse) {
    option (dandori.v1.start) = {};
  }
}
message StartRequest { string id = 1; }
message StartResponse { string id = 1; }
```

`a/w.flow`:

```flow
workflow w v1 implements a1.AService

use proto a1 from "v1/a.proto"

inputs
  id : string

outputs
  id : string

flow
  succeed id = id
```

See also: [E301](#e301)

<a id="e209"></a>

## E209 — A workflow runs another context's workflow as its child

**When**: A child workflow a workflow runs with `flow` belongs to another context, the two are not partners, the child is in no shared kernel of theirs, and it implements no open host service of the other side.

**Fix**: Call a service the other side publishes with `connect`; or make the two partners; or list the child in their shared kernel; or have the child implement an open host service of the other side.

**Reproduction**: put the files below in one directory, and run `ritsu sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta conformist
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`b/c.flow`:

```flow
workflow c v1

inputs
  id : string

outputs
  id : string

flow
  succeed id = id
```

`a/p.flow`:

```flow
workflow p v1

record Answer
  id : string

inputs
  id : string

task run_child(id: string) -> Answer
  flow "../b/c.flow"

flow
  let r = run_child(id: id)
  succeed
```

See also: [E202](#e202), [E207](#e207)

<a id="e210"></a>

## E210 — A document uses a channel or an HTTP operation that is no open host service of the other side

**When**: An AsyncAPI document sends to or receives from a channel across a boundary (an entry of its `channels` is a `$ref` to a channel of another context's document), and the channel is not under `open host service` of the other side's published language; and the same for the HTTP operations of an entry of `paths` an OpenAPI document points at across a boundary.

**Fix**: The other side lists the channel or the operation under `open host service`, or the document uses one it lists.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta conformist
  through b.events.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

published language b.events.v1
  asyncapi "b/events.yaml"

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`b/events.yaml`:

```
asyncapi: 3.0.0
info:
  title: B events
  version: 1.0.0
channels:
  done:
    address: b.done
    messages:
      done:
        payload:
          type: string
operations:
  sendDone:
    action: send
    channel:
      $ref: '#/channels/done'
```

`a/events.yaml`:

```
asyncapi: 3.0.0
info:
  title: A, from B
  version: 1.0.0
channels:
  done:
    $ref: '../b/events.yaml#/channels/done'
operations:
  receiveDone:
    action: receive
    channel:
      $ref: '#/channels/done'
```

See also: [E207](#e207), [E301](#e301)

<a id="e301"></a>

## E301 — An open host service is not in the published language

**When**: A service under `open host service` is not in the proto files of the published language; for a published language of OpenAPI and AsyncAPI documents, a name that is no HTTP operation (operationId) and no channel of its documents; also any under the published language of a Rust crate, which has no service.

**Fix**: Correct the service's name, or add the service to the proto.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService, NoService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E302](#e302)

<a id="e302"></a>

## E302 — A file of a published language is not the context's, or has another package

**When**: A proto, a rule, a Rust crate or the place of the generated code of a published language does not belong to the context; or a proto's package, or a crate's name (with `-` written `_`), is not what the heading names; or a `crate` is no crate of the workspace at the map's `code rust` place, or the map has no `code rust`.

**Fix**: Correct the heading's package, or list the context's own files.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v2
  proto "b/v1/b.proto"
  open host service BService
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E301](#e301)

<a id="e303"></a>

## E303 — Customer and supplier is written on one side only

**When**: One of the customer's `upstream <supplier> customer` and the supplier's `downstream <customer> supplier` is missing.

**Fix**: Write it in the other file too.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta customer
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E306](#e306)

<a id="e304"></a>

## E304 — A conformist has a mapping or a `layer`

**When**: A `conformist` relationship has a `layer`, an `enum` mapping or a `term` mapping; a conformist uses the upstream's model as it is.

**Fix**: To map the model, make the role anticorruption layer; otherwise delete the mappings and the layer.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta conformist
  through b.v1
  layer dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E305](#e305)

<a id="e305"></a>

## E305 — A relationship that is not an anticorruption layer has a mapping or a `layer`

**When**: A relationship with no anticorruption layer among its roles (customer alone, say) has a mapping or a `layer`.

**Fix**: Add `, anticorruption layer` to the roles, or delete the mappings and the layer.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta customer
  through b.v1
  layer dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind

downstream Alpha supplier
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E304](#e304)

<a id="e306"></a>

## E306 — Roles that cannot go together

**When**: conformist is written with customer, or with anticorruption layer.

**Fix**: Keep one of them.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta conformist, customer
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind

downstream Alpha supplier
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E303](#e303)

<a id="e307"></a>

## E307 — A shared kernel is written on one side only, or lists different things

**When**: A shared kernel is written on one side only, the two sides list different things (other than each keeping a copy), or what it lists belongs to neither of the two.

**Fix**: Write `shared kernel with <context>` in both files, with the same entries.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

shared kernel with Beta
  proto "a/a.proto"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E308](#e308), [E202](#e202)

<a id="e308"></a>

## E308 — The copies of a shared kernel differ

**When**: Each side lists its own copy in the shared kernel, and the copies' bytes differ.

**Fix**: Make the copies the same.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

shared kernel with Beta
  proto "a/k/units.proto"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind

shared kernel with Alpha
  proto "b/k/units.proto"
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/k/units.proto`:

```proto
syntax = "proto3";
package k;
message Yen { int64 amount = 1; }
```

`b/k/units.proto`:

```proto
syntax = "proto3";
package k;
message Yen { int32 amount = 1; }
```

See also: [E307](#e307)

<a id="e309"></a>

## E309 — A partnership is written on one side only

**When**: `partnership with` is written on one side only.

**Fix**: Write it in the other file too.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

partnership with Beta
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E201](#e201)

<a id="e310"></a>

## E310 — Separate ways and another relationship cannot go together

**When**: Two contexts that go separate ways have another relationship too.

**Fix**: Delete one of them.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta conformist
  through b.v1

separate ways from Beta
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E206](#e206)

<a id="e311"></a>

## E311 — A relationship with itself

**When**: A relationship names the context it is written in.

**Fix**: Correct the other context's name.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

partnership with Alpha
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="e312"></a>

## E312 — The upstream does not publish a package of `through`

**When**: A package under `through` is not among the upstream's `published language`s.

**Fix**: Write a package the upstream publishes.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta conformist
  through b.v9
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E203](#e203)

<a id="e313"></a>

## E313 — The `layer` of an anticorruption layer is not the downstream's

**When**: A directory or file under `layer` does not belong to the downstream that writes the relationship; the layer is on the downstream's side.

**Fix**: Write a directory of the downstream.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta anticorruption layer
  through b.v1
  layer dir "b"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="w301"></a>

## W301 — Following the upstreams comes back where it started

**When**: Two or more contexts are, one after another, upstream of each other.

**Fix**: Consider one direction, or a partnership.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

published language a.v1
  proto "a/v1/a1.proto"

upstream Beta conformist
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind

upstream Alpha conformist
  through a.v1
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/v1/a1.proto`:

```proto
syntax = "proto3";
package a.v1;
enum AKind {
  A_KIND_UNSPECIFIED = 0;
  A_KIND_X = 1;
}
message A1 {}
```

<a id="e401"></a>

## E401 — A mapping leaves values of the upstream enum out

**When**: An anticorruption layer's `enum` mapping gives neither a value nor refuse for some value of the upstream enum (a proto's enum, or a schema with `enum` of OpenAPI and AsyncAPI documents): what comes when the upstream adds a value. The value that says nothing is set (a proto's value 0 `…_UNSPECIFIED`, a document's `null`) needs none.

**Fix**: Write `<upstream value> -> <value>` or `<upstream value> -> refuse "<why>"` for each.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta anticorruption layer
  through b.v1
  enum Kind -> alpha_kind
    KIND_ONE -> first
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E402](#e402), [W402](#w402)

<a id="e402"></a>

## E402 — A mapping has a value the upstream enum lacks

**When**: The left side of an `enum` mapping is not a value of the upstream enum.

**Fix**: Correct the value's name, or delete the line.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta anticorruption layer
  through b.v1
  enum Kind -> alpha_kind
    KIND_ONE -> first
    KIND_TWO -> second
    KIND_THREE -> third
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E401](#e401)

<a id="e403"></a>

## E403 — A mapping's value is not one of the target enum

**When**: The target is a proto enum, and a right side is not one of its values.

**Fix**: Make the right side a value of the target enum.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

published language a.v1
  proto "a/v1/a1.proto"

upstream Beta anticorruption layer
  through b.v1
  enum Kind -> enum AKind
    KIND_ONE -> A_KIND_X
    KIND_TWO -> A_KIND_Y
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/v1/a1.proto`:

```proto
syntax = "proto3";
package a.v1;
enum AKind {
  A_KIND_UNSPECIFIED = 0;
  A_KIND_X = 1;
}
message A1 {}
```

See also: [E401](#e401)

<a id="e404"></a>

## E404 — An upstream enum referred to has no mapping in the anticorruption layer

**When**: Downstream of an anticorruption layer, an artifact refers to an upstream enum (the messages it uses reaching it count), and there is no `enum` mapping for it.

**Fix**: Write `enum <upstream enum> -> <target>` and its value lines.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta anticorruption layer
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.B b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E401](#e401)

<a id="e405"></a>

## E405 — A rule's import and the mapping of the `.ctx` disagree

**When**: The target is the enum of a rule that takes the upstream enum in with `import proto`, and a value line of the `.ctx` differs from the rule's import (each value's name on the wire, as rulec says it).

**Fix**: Delete the value lines and leave the mapping to the rule, or make them the rule's.

**Reproduction**: put the files below in one directory, and run `ritsu sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta anticorruption layer
  through b.v1
  layer rulec "a/x.rule"
  enum Kind -> rulec "a/x.rule" enum kind
    KIND_ONE -> two
    KIND_TWO -> two
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`a/x.rule`:

```rule
rule x v1

import proto "../b/v1/b.proto" Kind -> kind
enum kind = one | two

inputs
  k : kind

outputs
  n : kind

table t
policy unique
| k   | -> n |
| one | one  |
| two | two  |
```

See also: [E401](#e401)

<a id="e406"></a>

## E406 — A word of the same name and another meaning crosses unmapped

**When**: An upstream term crosses into the downstream with the element it `means`, the downstream has a term of the same name (`also` included) not taken with `as`, and no anticorruption layer maps it.

**Fix**: Rename the downstream's term, take it with `as` if it means the same, or make the relationship an anticorruption layer and map it.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

terms
  kind "The kind Alpha gives to customers"

upstream Beta conformist
  through b.v1
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.B b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E407](#e407)

<a id="e407"></a>

## E407 — A word is mapped to a word of the same name and another meaning

**When**: What an anticorruption layer maps an upstream word to has the name of a downstream word of another meaning.

**Fix**: Give what it is mapped to another name.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

terms
  kind "The kind Alpha gives to customers"

upstream Beta anticorruption layer
  through b.v1
  enum Kind -> kind
    KIND_ONE -> first
    KIND_TWO -> second
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
import "b/v1/b.proto";
message A { b.v1.B b = 1; }
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E406](#e406)

<a id="e408"></a>

## E408 — What a term `means` is not in the context's published language

**When**: A term's `means` points outside the files of the context's own published languages.

**Fix**: Point at an element of the context's own published language, or delete the `means`.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

terms
  goods "What Alpha sells"
    means proto "b/v1/b.proto" message Plain
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E007](#e007)

<a id="e409"></a>

## E409 — A word of a `term` mapping is not in the glossary

**When**: The left of `term <upstream term> -> <term>` is not in the upstream's glossary, or the right is not in the downstream's.

**Fix**: Correct the word, or add it to the glossary.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta anticorruption layer
  through b.v1
  term no_such_word -> something
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E410](#e410)

<a id="e410"></a>

## E410 — What `as` takes cannot be taken

**When**: The term of `as` is not in the other context's glossary, or there is no relationship with that context.

**Fix**: Correct the term, or write a relationship with the other context.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

terms
  kind as Beta.kind
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E007](#e007)

<a id="w401"></a>

## W401 — A term crosses no boundary

**When**: A term `means` nothing of the context's published language, is not taken with `as`, is not what a layer maps to, and meets no word of the same name crossing from upstream. A glossary holds only the words that cross a boundary.

**Fix**: Make it a word that crosses, or delete it from the glossary.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

terms
  stranded "A word that goes out nowhere"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

<a id="w402"></a>

## W402 — The value 0 that says nothing is set is mapped

**When**: The value 0 whose name, without the enum's prefix, is unspecified is mapped; it marks that nothing is set, and needs no mapping.

**Fix**: Delete the line.

**Reproduction**: put the files below in one directory, and run `sakai check .` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"

upstream Beta anticorruption layer
  through b.v1
  enum Kind -> alpha_kind
    KIND_UNSPECIFIED -> nothing
    KIND_ONE -> first
    KIND_TWO -> second
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E401](#e401)

<a id="e501"></a>

## E501 — The settings of a tool cannot be written

**When**: `sakai build` finds that the map has no `code` line for the language, no context has code in it, a directory's name is not a module's (Python and Java), an entry names one file of code, a Python module sits right in the place of the code (import-linter cannot read it), a Java class is in the default package, ArchUnit has no place for the tests, or the place of the Go code has no go.mod.

**Fix**: Correct the map's `code` and `owns`, or where the code is, as the note says.

**Reproduction**: put the files below in one directory, and run `sakai build map.ctx --target import-linter` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

See also: [E502](#e502)

<a id="e502"></a>

## E502 — The settings on the disk differ from what the map writes now

**When**: `build --check` finds a settings file on the disk that differs from what the map writes now, or none. The note gives the first line that differs and what the map writes there.

**Fix**: Write it again with `sakai build`. Checked with another `--lang` than it was written with, the words of its descriptions differ: give the same one.

**Reproduction**: put the files below in one directory, and run `sakai build map.ctx --target import-linter --check` there.

`map.ctx`:

```ctx
map Map(m) v1
use context "alpha.ctx"
use context "beta.ctx"
covers "."
code python "py"
```

`alpha.ctx`:

```ctx
context Alpha(a) v1
owns
  dir "a", "py"
```

`beta.ctx`:

```ctx
context Beta(b) v1
owns
  dir "b"

published language b.v1
  proto "b/v1/b.proto"
  open host service BService

terms
  kind "The kind of thing Beta deals with"
    means enum Kind
```

`a/a.proto`:

```proto
syntax = "proto3";
package a;
message A {}
```

`b/v1/b.proto`:

```proto
syntax = "proto3";
package b.v1;
enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_ONE = 1;
  KIND_TWO = 2;
}
message B { Kind kind = 1; }
message Plain { string id = 1; }
service BService { rpc Get(B) returns (B); }
```

`py/k/x.py`:

```
X = 1
```

`py/.importlinter`:

```
# written by hand
```

See also: [E501](#e501)
