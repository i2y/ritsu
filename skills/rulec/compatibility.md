# Compatibility

From 1.0.0, rulec's version numbers follow [Semantic Versioning](https://semver.org/). This
page says what "compatible" means for each thing rulec reads or writes, so that a 1.x
release is a promise you can hold it to.

The short form: **a rule that passes `rulec check` under 1.0 passes under every 1.x, means
the same thing, and its generated code is called the same way and answers the same.**

## What every 1.x keeps

1. **The `.rule` language.** Every construct 1.0 accepts keeps its syntax and its meaning — the
   state machines (`machine`, `held`, `never`, `once`, `scenario`, and the values carried
   besides the state: a further `carry`, `initial <input> <value>`, `always`), the
   prerequisites of an enum's values (`prerequisites` with `start`, `requires`, `nothing` and
   `ordered by`), several sequences walked by any number of `count`, `sum`, `max`, `min` and
   `fold` lines in the order what they read gives them, a fold's answer named with `->`, what a
   rule reads out of the caller's object — a total, a largest or smallest value of one field over
   a collection (`from sum`, `from max`, `from min`), and a sequence taken from a collection
   (`elements … from`) — the inputs drawn at random with what is claimed of the chances of the
   answers (`random`, `chances`, a pick by weight with `take_weighted` and `draw`), the merging of
   files — open values (`open … combine`), contribution files (`contribution`, `to`, `after`,
   `into`, `extend`), open tables, clauses, enums and blocks, and the pins `signature:` and
   `meaning:` — a rule served as the Connect service a contract declares (`implements proto`), and
   the range a rule accepts for a value it works out (`define … range`, which no earlier version
   read: the line was a syntax error) included. An `apply` heading pinned with `sha256:`, as written before `meaning:` came, is read
   as it was. New constructs may be added.
2. **The names you may use.** The reserved words — the 65 words E009 refuses as a name,
   listed in `src/kw.rs` — are fixed at 1.0, and a test holds the list. A later release may
   add words to the language, but never to that list: a new word is read only where no name
   can stand, or gives way to a name the rule declares. A name that is legal under 1.0 stays
   legal. The words of the merging of files came this way: `open`, `combine`, `product`,
   `contribution`, `to`, `into` and `extend` are read by their place in the line, and a rule may
   name an input `open` or `to`. So did `implements`, read only at the head of a line outside
   every block: a rule may name an input `implements`. `never` was on the list already, and heads a
   line of its own outside `machine` only when a name, not a `:`, comes after it: a field of an element
   may still be called `never`.
3. **What the generated code answers.** For a rule that passes, the code generated in every
   language returns the same outputs for the same inputs under every 1.x, and so does the
   reference evaluator: a record replayed under a later 1.x gets the answer it got before.

   One thing changes before 1.0 in what the generated code answers. A fold that walks a sequence
   alone and stops used to leave the loop at the `stop`, so the elements after it were not held
   to their ranges and their `unique` fields: the code of every language answered an input the
   reference evaluator refuses. It now holds them and refuses the input, as `rulec run` did
   before and does (§15.328). The module of `tests/corpus/nationwide_freight.rule`, called with a
   first row the fold stops at and a second whose threshold is −1:

   ```python
   nationwide_freight([Element(Zone.REMOTE, 2000, 0), Element(Zone.KINKI, -1, 0)])
   # 0.55.2: 0
   # after:  RuleInputError: freight_rows[1].threshold is out of range: -1
   ```
4. **How the generated code is called.** The names of modules, functions, types, fields and
   errors, the integers on the wire (§10.2), the record a call writes, the `_traced`,
   `_record`, `_from` and `_chances` functions — everything [`rulec api`](formats.md#api)
   describes. New functions may appear beside them.

   One thing changes before 1.0 in how the generated Rust is called: the sentence of its error.
   `RuleError::Input { what, value }` and `RuleError::Contradiction { what }` keep their names
   and their fields, and `what` is a `Said` where 0.55.2 had a `&'static str` (§15.324). A
   `Said` holds the words of the sentence and the index of each element it names, and puts the
   sentence together where it is shown: refusing an element still makes no string. Every module
   rulec writes in Rust has it, the modules of rules that read no sequence among them, and so
   does the Wasm module built from one. A caller that shows the sentence (`{what}`,
   `what.to_string()`) or compares it with a string (`what == "amount is out of range"`) builds
   as it did. One that hands it on as a `&str` hands on the string it shows, and a pattern that
   writes a sentence becomes a guard:

   ```rust
   // 0.55.2: what is a &'static str
   Err(RuleError::Input { what: "amount is out of range", .. }) => "413 too large".to_string(),
   Err(RuleError::Input { what, .. }) => bad_request(what),
   // after: what is a Said
   Err(RuleError::Input { what, .. }) if what == "amount is out of range" => "413 too large".to_string(),
   Err(RuleError::Input { what, .. }) => bad_request(&what.to_string()),
   ```

   The last two lines build against a module of 0.55.2 as well, so a caller can change first.

   A rule where a name made from an enum meets one of the rule's own in a language (`rule band`
   with an enum `band` of its own, `rule band_label`) is written with a number on the enum's type
   there, `Band2` (§15.326), and `rulec api` says so. 0.55.2 wrote that language's code for such
   a rule so that it did not compile or load, or so that one of the two names hid the other; a
   call that worked keeps working, for the rule's function keeps its name.
5. **The command line.** Commands, flags, their values, and the exit codes: 0 notes only, 1
   findings, 2 bad arguments or a file that cannot be read. New commands and flags may be
   added.
6. **Diagnostic codes.** A code keeps its meaning. A code that is retired stays in the ledger,
   marked as retired, and its number is never given to anything else. New codes may be added;
   a new warning leaves a rule that passed passing (W901, a key written in a rule, came this
   way, and means the same in every language of ritsu). Two numbers took a new meaning before
   1.0, when a rule came to walk more than one sequence and to walk one twice (§15.219): E031,
   which had said that a rule needs a second walk, says that one item reads the elements of two
   sequences, and E020, which had said that a rule declares two, says that nothing walks one.
   E044 narrowed before 1.0, when an `apply` came to read a rule that applies another, walks a
   sequence or produces an enum of its own (§15.230): it says that applies go round, or that the
   callee cannot be read or does not pass `check`, and a rule it stopped for the other three now
   passes. E033 and E040 say of a value a contribution adds and of a contribution's pin what they
   say of an imported value and of an apply's pin (§15.228, §15.230).
7. **Machine-readable formats.** Every `--format json` output, the vectors, the fixtures and
   the replay manifest, the `adapter/1` and `extract/1` protocols, the certificate, the MCP
   tools with their arguments and resources, the GitHub Action's inputs, and the names of the
   release archives and of the packages beside them (`i2y/tap/ritsu` in Homebrew, the `.deb`
   and the `.rpm`) keep every field they have at 1.0, with the same meaning. A 1.x may add a
   field, and may add a value to a set of values the documentation lists — a new diagnostic
   code, a new `what` in the `domain` of `diff`. A new construct may give a field a new shape
   where it is written, and nowhere else: a `constraint` that adds inputs up is the one place
   its `left` or `right` is a list of names, in the certificate and under `preconditions` alike,
   and an `apply` pinned with `meaning:` the one place its entry of `applies` has an empty
   `sha256`, beside `meaning`.
   **A reader should skip a key it does not know, and treat a value it does not know as it
   treats an unknown key.** Where a format carries
   `v`, that is the version of its shape: `check` is at 2 and the certificate at 1, and an
   output without `v` is at 1. A re-checker of certificates refuses a `v` it was not written
   for, as `tools/recheck.py` and the Lean program in `proofs/` do, and a reason it does not
   know: a certificate of the next version may give a leaf about the rows of a table above one
   of five reasons the re-checkers of 0.55.2 do not know (`earlier`, `misses`, `reach`,
   `writes`, `disjoint`, [formats.md](formats.md)), and they refuse that leaf.

   The next version writes the JSON of `explain` in one shape in every language of ritsu: one
   code as one indented object, and two or more codes, or `--all`, as one indented array of the
   objects, in the order given ([formats.md](formats.md)). 0.55.2's `rulec explain --format json`
   and `geas explain --json` wrote an object a line, so a reader that took their output a line at
   a time reads it as one JSON value instead. The objects keep their keys. koyomi, chobo, yuen and
   sakai, whose `explain` refused `--format json` (exit 2), write it in the same shape:

   ```text
   $ rulec explain --all --format json                    # 0.55.2
   {"code":"E001","severity":"error","title":"Unterminated string",…}
   {"code":"E002","severity":"error","title":"Unreadable character",…}
   …
   $ rulec explain --all --format json                    # after
   [
     {
       "code": "E001",
       "severity": "error",
       "title": "Unterminated string",
   …
   ```

<!-- crates.io is on hold (DESIGN §15.158). Once the crate is published, name it in item 7
as well: "the crate `rulec`", after `i2y/tap/ritsu` in Homebrew. -->

## What a 1.x may change

- **Prose.** Diagnostic titles, notes and hints, `--help`, the text reports, what `rulec doc`
  writes, and this documentation. Read codes and JSON keys, not sentences (§11 principle 5).

  The next version names the element a refusal is about. A reason that is one element of a
  sequence puts the element's place before the field — the index from 0 in the input's array —
  and a reason two elements give together names both after the sentence, the earlier first.
  Where more than one element is at fault, every language names the first. The reference
  evaluator, the code of every language with its runner, the MCP servers, the Connect service
  and the ports ritsu's other languages call through say the same (§15.324):

  ```text
  $ echo '{"lines":[{"quantity":1,"price":100},{"quantity":10,"price":1000001}]}' | rulec run order_line_subtotals.rule --input -
  refused (input): subtotal is out of range            # 0.55.2
  refused (input): lines[1].subtotal is out of range   # after
  $ echo '{"lines":[{"quantity":1,"price":100},0]}' | rulec run order_line_subtotals.rule --input -
  refused (input): an element of lines is not an object   # 0.55.2
  refused (input): lines[1] is not an object              # after
  $ echo '{"buildings":[{"building":"temple"},{"building":"library"},{"building":"temple"}]}' | rulec run province_learning.rule --input -
  refused (input): buildings has two elements with the same building                                  # 0.55.2
  refused (input): buildings has two elements with the same building (buildings[0] and buildings[2])  # after
  ```

  The projection function (`<alias>_from`, which Python, TypeScript, JavaScript, Ruby and PHP have
  and Go, Swift, Java, Rust, SQL, NumPy and Wasm do not) names the element too: a value it reads out
  of an element of the caller's collection is named by its path with the element's index in it, the
  index the rule's door names the same element by. One call to `order_handling_fee_from` of the
  corpus, whose second line is wrong in one way and then another:

  ```text
  quantity "abc": order.lines[].quantity is not an integer: 'abc'    # 0.55.2
                  order.lines[1].quantity is not an integer: 'abc'   # after
  quantity 500:   quantity is out of range: 500                      # 0.55.2
                  lines[1].quantity is out of range: 500             # after
  ```

  A sentence of `rulec check` about a contract (E120, E121, E122) still writes a path in an
  element with `[]`, `order.lines[].sku`: it is about a place the contract has in every element,
  not about one element of a call.

  The kind of the refusal (`input` or `contradiction`) and the value carried beside the sentence
  stay as they were. A caller that compared such a sentence with a string compares it with the
  new one. The vectors change with it: a line of `vectors/<alias>.refused.jsonl` says the new
  sentence, and a case about one element puts the element at fault second and third, after one
  the door takes ([formats.md](formats.md#vectors-rulec-vectors-generatedvectorsaliasjsonl)), so
  `rulec gen --check` asks for them to be written again. The reasons `rulec fixtures` and
  `rulec replay` group records by keep naming a field of an element with its sequence,
  `lines[].amount`.

  The next version also says one thing of an input with two or more things wrong with it, by every
  way the rule is reached: the first in one order. Each argument is read for its kind, in the order
  the function takes them; then each input is held to its range, then the constraints and the cap
  of a sequence; then what the function works out, element by element
  ([reference](reference.md#an-input-with-two-things-wrong), §15.328). The reference evaluator, and
  the ways that read in another order, say another reason than they did for some such inputs, and
  the kind of the refusal can change with it, where the walk meets a contradiction before an element
  outside its range:

  ```text
  $ echo '{"freight_rows":[{"row_zone":"kinki","threshold":0,"row_fee":0},{"row_zone":"kinki","threshold":0,"row_fee":0},{"row_zone":"kinki","threshold":-1,"row_fee":0}]}' | rulec run nationwide_freight.rule --input -
  refused (input): threshold is out of range                                                                     # 0.55.2
  refused (contradiction): fold verdict: two elements matched a take_unique (freight_rows[0] and freight_rows[1])   # after
  ```

  An input with one thing wrong is refused with the sentence it was. `vectors/<alias>.refused.jsonl`
  gains an input with two things wrong for each pair of places the door looks at that the rule has,
  so `rulec gen --check` asks for the vectors to be written again. The port ritsu's other languages
  call a rule through keeps to the same order, in its own words, and now refuses a whole number
  int64 does not hold where it reads it, as every other way does; it let one through on a side of
  a range no end was written for.
- **The text of the generated code.** Its layout, comments and local names, and its header,
  which names the version of rulec that wrote it. Upgrading rulec therefore means running
  `rulec gen` again and committing what changed; `rulec gen --check` fails until you do. CI
  installs one exact release (`uses: i2y/ritsu@v1.0.0`), and there is no moving tag such as
  `@v1`: the generated files would go stale under it without a change of yours.

  The next version writes the comments of the code, and the text the code returns, in the language
  of `--lang`, and in English without it: `RITSU_LANG` and a language's own variable (such as
  `KOYOMI_LANG`) change only the text a command prints. So does every other file a command writes
  for people or programs to keep: a page of `doc`, the JSON of `api`, the project `ritsu ringi init`
  writes. This holds for `ritsu gen`, the rules of its packages included, and for the languages' own
  commands, `rulec gen` run on its own among them.
  A project that set `RITSU_LANG=ja` to have Japanese comments gets English ones, and
  `ritsu gen --check` fails until it gives `--lang ja`, which writes what 0.55.2 wrote:

  ```text
  $ RITSU_LANG=ja ritsu gen . --target go                 # 0.55.2
  生成しました: generated/go/rules/shipping/shipping.go
  生成しました: generated/go/units/units.go
  生成しました: generated/go/doc.go
  $ sed -n 2p generated/go/rules/shipping/shipping.go
  // もと: shipping.rule（rule shipping v1、sha256:0bb2ba9a247c926d）

  $ RITSU_LANG=ja ritsu gen . --target go --check         # after (exit 1)
  生成物が古いか手で編集されています: generated/go/rules/shipping/shipping.go
  $ ritsu gen . --target go --check --lang ja             # after (exit 0)
  $ RITSU_LANG=ja ritsu gen . --target go                 # after
  生成しました: generated/go/rules/shipping/shipping.go
  $ sed -n 2p generated/go/rules/shipping/shipping.go
  // Source: shipping.rule (rule shipping v1, sha256:0bb2ba9a247c926d)
  ```
- **The names the code of a gate keeps for itself, and the reserved words of its languages.** The
  code `sekisho gen` and `ritsu gen` write of a gate declares names of its own, and a type of the
  gate written as one of them is W402 at `check` and stops `gen` of that target with E401. The next
  version adds two of them. One is the entry that asks the gate by the name of an action
  (`authorize` in TypeScript and Python, `Authorize` in the code of `--target go`); its error for an
  action the gate does not declare is of a new kind, `action`: the TypeScript type
  `SekishoErrorKind` and the Python `Literal` of the kinds gain the value, and a `switch` that names
  every kind needs a case for it. The other is the lists of the roles: `roles()` gives every role
  the gate declares, and `rolesOf(t)` (`roles_of(t)` in Python; `Roles()` and `RolesOf(t)` in the
  code of `--target go`) the roles a principal of the type `t` can hold. The code refuses every
  action to one who holds a role their type cannot, so the code that signs someone in keeps only
  those before it asks. A type named `Authorize`, `Roles` or `RolesOf` is W402 for the code of
  `--target go`, and `gen --target go` stops with E401 until its alias changes (no gate of ritsu's
  tree has one).

  ```text
  resource Authorize
    description "A slip that lets its holder be answered"

  $ sekisho check slips.gate
  slips.gate: ok — 1 action, 1 policy (1 permit, 0 forbids), 0 expectations, 0 separations   # 0.55.2
  warning[W402]: slips.gate:13:1: The type `Authorize` is written `Authorize` in the code of Go, a name the code uses itself   # after
  $ ritsu sekisho gen slips.gate --target go
  generated: generated/go/authz/slips/slips.go           # 0.55.2
  error[E401]: slips.gate:13:1: The type `Authorize` is written `Authorize` in the code of `--target go`, a name the code uses itself   # after
  ```

  The next version says a reserved word of a target's language the same way. 0.55.2 refused a name
  spelled as a reserved word of any language a gate's code is written in with E008 at `check`,
  wherever the name stood, so no target of the gate could be written, Cedar's included. The next
  version keeps E008 for the reserved words of Cedar, which every gate is written in, and says a
  reserved word of a target's language with W402 for that target, where its code writes the name as
  it is: a field of Python's records (an attribute), a method of Python's `Store` (a type, in lower
  case), and the alias of the file where it names Python's module, the package of `--target go` (its
  `_` dropped), or, in TypeScript, the module the index of a package `ritsu gen` writes exports (a
  reserved word of strict mode, such as `interface`). `gen` of that target stops with E401 and
  writes nothing, and a run of the other targets writes them. A gate 0.55.2 refused for such a name
  passes `check` (exit 1 to 0), and `gen` writes every target that does not write the name as it is.
  Two forms 0.55.2 passed, writing code that does not load, now stop `gen` of their target: a type
  whose name in lower case is a keyword of Python (`Return`), and an alias that is a keyword in the
  code of `--target go` once its `_` are dropped (`se_lect`).

  ```text
  resource Parcel
    description "A parcel the depot holds"
    attributes
      type : kind

  action delete

  $ sekisho check depot.gate
  error[E008]: depot.gate:18:5: `type` is a reserved word of Go, and cannot be a name in the generated code   # 0.55.2
  error[E008]: depot.gate:20:8: `delete` is a reserved word of TypeScript, and cannot be a name in the generated code   # 0.55.2
  depot.gate: ok — 1 action, 1 policy (1 permit, 0 forbids), 0 expectations, 0 separations   # after

  resource Return
    description "Goods a customer sends back"

  $ sekisho check returns.gate
  returns.gate: ok — 1 action, 1 policy (1 permit, 0 forbids), 0 expectations, 0 separations   # 0.55.2
  warning[W402]: returns.gate:13:1: The type `Return` is written `return` in the code of Python, a reserved word of the language   # after
  $ ritsu sekisho gen returns.gate --target python
  generated: generated/python/authz/returns.py           # 0.55.2, which does not load (SyntaxError)
  error[E401]: returns.gate:13:1: The type `Return` is written `return` in the code of `--target python`, a reserved word of the language   # after
  ```
- **A pass that should have been a failure.** When a 1.x finds that an earlier version let
  through something this page or [what it proves](https://i2y.github.io/ritsu/rulec/checks/) says
  it catches — a gap or an overlap it missed, a line it read into nothing — the fix makes that
  rule fail. A false green is not kept for the sake of compatibility. Each such change is
  named in the release notes, with the kind of rule it can affect.

  The next version stops four kinds of rule 0.55.2 passed, each a rule that does not answer what
  it declares. An output that nothing decides (no table writes it after `->`, no `define` or
  `derive` has its name, no `result` line), an output a table or a definition reads above what
  decides it, and an output read in the expression of the `define` that decides it are E151, and a
  table whose heading has no `->` is E152. 0.55.2 answered such an output with no value, and the
  code it generated failed on every call or did not compile. A step written on a rate the rule
  works with and does not answer — on a table's heading, or on the type of a `derive` or a
  `define` — holds what the rate takes: a cell off it, or a name or an expression that can come to
  a value off it, is E114, where 0.55.2 worked on with the value between two steps.

  ```text
  outputs
    fee    : money[JPY]  round down(1JPY)
    review : bool

  table review_of
  policy unique
  | fee       | -> review |
  | <1000JPY  | false     |
  | >=1000JPY | true      |

  result fee = down(amount * 3%, 1JPY)

  $ rulec run read_result.rule amount=50000
  fee = 1500JPY
  review = none                                 # 0.55.2: the table read the fee before it had one
  $ rulec check read_result.rule
  ok read_result.rule                           # 0.55.2
  error[E151]: The output `fee` is read before it is decided   # after

  | item_kind | -> markup : rate[step 1%] |
  | books     | 0.5%                      |

  $ rulec check markup.rule
  ok markup.rule                                # 0.55.2: a book is marked up 0.5%
  error[E114]: `0.5%` does not sit on this column's step   # after
  ```

  A row 0.55.2 named with W128 — no input was found that reaches it, and none was shown not to —
  is E102 where the check now shows that none does. The check works a value the rule computes out
  again over the row's cells: as the inputs the cells hold times numbers, added up, on the grid an
  input's own step gives it, inside the range the rule accepts for a `define`, over whole numbers,
  and through the rows of a table above that decide a value from it. A rule with such a row passed
  0.55.2 with the warning, and fails now:

  ```text
  define sheets : number = gifts * 2

  table note_of
  policy first
  | sheets | -> note |
  | 3      | true    |
  | -      | false   |

  $ rulec check odd_cell.rule
  ok odd_cell.rule                                     # 0.55.2 (W128 on row 1)
  error[E102]: Unreachable row: row 1 never matches    # after
   `sheets` only comes to multiples of 2, and this row asks for none of them.
  ```
- **An answer that disagreed with the reference evaluator.** If the code generated in some
  language answered differently from the reference evaluator, the fix makes it agree, and
  the release notes name it.
- **An input a door let through.** If the code generated in some language answered an input
  outside its contract that the reference evaluator refuses — a value of another kind, an input
  that is not there, a combination a `constraint` or a `never` line rules out, a value a `define`
  works out outside the range the rule accepts for it — the fix makes it refuse the input,
  as the other languages do, so a call that was answered can fail from then on. The sentence a
  door says is prose. The release notes name the change.
- **A range that was read wider than it is written.** If the door and `rulec check` both read
  a range wider than the rule writes it, the fix reads it as it is written: a call that was
  answered can be refused from then on, by the reference evaluator and by the code of every
  language, and the check asks for rows over the range as written. The release notes name the
  change. 0.55.2 and the versions before it read two ends that way: an end written with `>` or `<`
  was read as taking the value it writes, and a walk was not held to the bottom of the range of a
  `sum` or a `count`.

  ```text
  inputs
    amount : money[JPY]  range >0JPY <100JPY

  $ rulec run strict_input.rule amount=0
  big = false                                   # 0.55.2
  refused (input): amount is out of range       # after
  $ rulec run strict_input.rule amount=100
  big = true                                    # 0.55.2
  refused (input): amount is out of range       # after

  sum amount over lines of price  range >=1JPY <=100_000JPY

  $ echo '{"lines":[]}' | rulec run sum_lower.rule --input -
  big = none                                    # 0.55.2: a table that passed as complete had no row
  refused (input): amount is out of range       # after
  ```

  `>0JPY` is 1 yen and up, `<100JPY` is up to 99 yen, on the step the value is held at (one unit of
  an amount or a quantity, the step a rate declares, one day of a date). A total or a count that
  ends under a bottom above zero — an empty sequence, for one — is refused with the sentence one
  past the top gets. `rulec check` moves with the reading: a table needs no row for the value such
  an end writes, a row written only for values outside a range is E102, and a `derive`, a `max`,
  a `min` or an output whose range is written with `>` or `<` and whose value can come to that
  end is E112 or E140. An open value whose terms come to such an end of its range is E131, and a
  rule that passes one to a rule it applies, whose input leaves it out, is E043. A sequence taken
  from a contract (`elements … from`) whose `sum` or `count` starts above zero is E122 where the
  contract lets a collection come in under that bottom — an empty one, where it writes no
  `minItems` — since the generated code now refuses what the contract's validation let through.
  A rule that writes its ranges with `>=` and `<=`, and whose `sum` and `count` lines start at 0,
  is read as it was — its code and its answers are the same — with one difference in the check: a
  row that takes only
  values outside the range of a `count`, a `sum`, a `max`, a `min` or a fold's answer was a warning
  (W133) and is E102, as a row outside the range of an input always was.
  To find the calls this touches among the ones you have recorded, run
  `rulec fixtures lint <records.jsonl> <file.rule>` with the new version: it names the records
  the rule no longer takes, by the reason (`the sum amount is under the bottom of its declared range, 500JPY`).

  Two more passes of 0.55.2 fail for the same reason, a call the rule's own door turns away that
  `rulec check` did not look at. A `sum` or a `count` whose range no sequence the rule takes can
  come to is E150: its bottom is past what the longest sequence reaches with every element at
  the top of its range (or with each value of a `unique` field once), or its top is under what
  the shortest one comes to. 0.55.2 passed such a rule and answered it without holding the
  bottom; held to the range as written, it would refuse every call. And a sequence taken from a
  contract is E122 where it has a `unique` field, or where the weights of a `take_weighted` over
  it can total past int64 with what the contract lets through: the code 0.55.2 generated refused
  those requests already, and `rulec check` said nothing.

  ```text
  elements lines
    price : money[JPY]  range >=0JPY <=100JPY
    gift  : bool

  count gifts over lines where gift  range >=0 <=5
  sum amount over lines of price  range >=1_000JPY <=100_000JPY

  $ rulec check bulk.rule
  ok bulk.rule                                  # 0.55.2 (W133 on each row of the table over amount)
  error[E150]: The range of the sum does not meet the values it can reach   # after
   lines has at most 5 elements (the top of the count gifts), and price is at most 100JPY in each: 5 × 100JPY = 500JPY, short of the bottom 1000JPY of this range. The generated code would refuse every call once the walk is over.
  $ echo '{"lines":[]}' | rulec run bulk.rule --input -
  big = false                                   # 0.55.2
  ```

  The vectors `rulec gen --extra vectors` writes change with the reading: every end a range
  writes has a case standing on it, and a table that reads a value worked out from a `sum` or a
  `count` has the two cases on either side of each boundary it writes on that value (0.55.2 left
  those boundaries without their pair). A boundary a table writes on a value worked out on a step
  has its two cases on the values the value takes: 2 and 4 sheets for `<4` over
  `sheets = gifts * 2`, which is always even, where 0.55.2 asked for 3, which no input gives. The
  boundaries of a walk whose `sum` or `count` starts above zero have their cases inside that
  bottom, and the ends and the boundaries of a value worked out from two answers of a walk have
  theirs.
  `rulec gen --check` asks for the vectors to be written again.

  The checks `ritsu check` makes across the languages are held to the same. The next version
  stops a workflow's call that gives a number which the gate counts in another unit or step
  (E909). The task below gives its `fee` as `rate[step 0.1%]`, so the code generated from the
  flow sends 5 for 0.5%, and the action's `input` reads the same 5 as `rate[step 1%]`, 5%. 0.55.2
  passed the pair when the task declared the error of a denial, as this one does, and said only a
  W909 when it did not.

  ```text
  task refund_order(orderId: string, fee: rate[step 0.1%] range >=0% <=10%)
    http POST orders "/orders/{orderId}/refunds"
    errors denied = 403

  flow
    refund_order(orderId: order, fee: 5)

  action refund_order                           # refunds.gate
    guards orders refundOrder
    input
      fee : rate[step 1%]  range >=0% <=10%

  $ ritsu check .
  ritsu check: 2 files (dandori 1, sekisho 1): all pass; borders between the languages: 1 checked, 0 undecided   # 0.55.2
  warning[dandori W106]: returns.flow:15:37: `5`, written for `fee`, counts steps of 0.1%: it is 0.5%   # after
  error[ritsu E909]: returns.flow:15: The task `refund_order` gives `fee` as rate[step 0.1%], and the action `refund_order` of refunds.gate reads its input `fee` as rate[step 1%]   # after
    = The two read one number as two values: the flow's 0.5% is 5% to the gate.
  ```

  The warning beside it is dandori's: the next version reads a number written with its unit where
  its type is known, and asks for the unit where a whole number counts steps of a rate other than
  1%. Written `fee: 0.5%`, the flow sends the same 5, and `ritsu check` says the same E909 of it
  without the warning (0.55.2 did not read `0.5%` there, E001).
- **A `diff` that called two versions the same.** If `rulec diff` said that every input gets the
  same answer from two versions that do not take the same inputs, or do not answer them alike,
  the fix makes it name the change and exit 1. The release notes name it. 0.55.2 and the versions
  before it said so, with exit 0, of two versions that differ only in what a field of an element
  takes (its range, its type, `unique`), or only in an expression that reads a `count` or a `sum`
  no table tests (`define big_sale : bool = total_fee > 500` moved to `> 600`). The first is
  listed under `domain` now (`field_range` and the other kinds [formats.md](formats.md) lists),
  and the second is compared over the sequences both versions take, cut by what the `count` or
  the `sum` comes to (`total_fee >=501 <=600`: `big_sale: true → false`). With `--fixtures`, a record the new version refuses for a field of an
  element names the field with its sequence, `lines[].price`, as the format says; 0.55.2 wrote
  `price`.
- **An input a door refused inside its contract.** If the code generated in some language, or
  the reference evaluator, refused an input its contract allows — a whole number written with
  a point or an exponent, `1000.0` or `1e3` — the fix makes it take the input, so a call that
  failed can be answered from then on. The release notes name the change.
- **How long a check takes,** as long as a rule that finished within the default budget
  still does.

## What is not covered

- **The Rust library** (`src/lib.rs`). Its modules are public so that the tests, ritsu (its
  commands and its playground) and `rulec mcp` can reach them, not as an interface; they change
  with the tool. Use the command line, `rulec mcp`, or the generated code.
- **ritsu's playground and the site.**
- **Everything under `experiments/`.**
