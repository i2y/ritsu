/-
  A module of kaname as the model reads it (kaname's DESIGN 15.1): its functions, each a parameter
  list, the places its values go to, and a body of expressions with everything the check of types
  decided written into them.

  The Rust test writes a module in this form from what kaname's check found (`crates/ritsu-model/
  tests/kaname.rs`). A number is read in another unit only where kaname's interpreter reads it so
  (`Conv`: times a factor, plus a shift between two zeros, failing `overflow` where the value leaves
  int64). The step a numeric type says its values are on (`step`) is written on the expressions
  whose values the analysis bounds by it. The layers kaname calls are named by an index: a rule
  (`RuleD`), a dates file (`DatesD`); what they answer comes from outside (`Oracle`).

  A block is a chain: `letIn x v c body` names the value of `v`, read by `c`, and goes on to
  `body`, the rest of the block; the last expression of a chain answers.

  A lambda made a value (`lam`) is one of the module's (`Lam`): its parameters, its body, and the
  function it is written in (kaname's DESIGN 27.1). A function written as a value, and a function value
  a builtin is given by its name, are such lambdas (`wrapped.rs`): the lambda that calls it.
-/
import KanameCore.Value

namespace KanameCore

/-- A number read in another unit: times `f`, plus `shift` (the zeros of ℉ and ℃). It fails
    `overflow` at `line`, and the analysis keeps its place at the expression `expr`. -/
structure Conv where
  f : Rat
  shift : Rat
  expr : Nat
  line : Nat
  deriving Repr, Inhabited

/-- The ends of a declared range or count, in the unit of what it holds. -/
structure Range where
  min : End
  max : End
  deriving Repr, Inhabited, DecidableEq

/-- What a place declares of the values it takes (kaname's `Slot`, as `interp::outside` and
    `ranges::keeps` read it): whether it holds a list, a set or a dict (under its optionals), the
    count of its elements, members or pairs, the range of its numbers and dates (of the elements, the
    members, the values of a dict), whether it is a rate with a step and no range (held to 0% to 100%
    where a parameter reaches it), the fields of the record of a module it holds, each its own place,
    the fields of each value of an enum that has fields it holds (the one value, for the type of a
    value; DESIGN 23.2), each its own place, the place of the keys of a dict (DESIGN 24.4); and, where
    what it holds (an element, a member, a value of a dict) is a value of a record or an enum that
    holds itself, that type's number in the module's table (`Module.slots`, DESIGN 28.1): a call of
    kaname holds nothing of such a value, its fields held where it was built or came in, and a host's
    argument is held to the table's place, one deeper (`Slot.enterD`). -/
inductive Slot where
  | mk (list : Bool) (count : Option Range) (range : Option Range) (unit01 : Bool) (record : Bool)
      (fields : List (String × Slot)) (values : List (String × List (String × Slot))) (keys : Option Slot)
      (ref : Option Nat)
  deriving Repr, Inhabited

/-- How two values are compared: as numbers, as dates, as strings (by their code points, DESIGN
    25.1), as values of an ordered enum (by their places in its order), or as equal or not. -/
inductive Order where
  | num
  | date
  | str
  | enum (values : List String)
  | eq
  deriving Repr, Inhabited

/-- The order kaname puts the keys of a type in (DESIGN 24.1): numbers from the least, dates from
    the earliest, strings by their code points, `false` before `true`, the values of an enum in the
    order they are declared, a record field by field in the order they are declared. -/
inductive Key where
  | num
  | date
  | str
  | bool
  | enum (values : List String)
  | record (fields : List Key)
  deriving Repr, Inhabited

inductive CmpOp where
  | eq | ne | lt | le | gt | ge
  deriving DecidableEq, Repr, Inhabited

inductive ArOp where
  | add | sub | mul | div
  deriving DecidableEq, Repr, Inhabited

/-- A pattern of a `match` arm (DESIGN 23.3). A value of an enum is named by its declared name; none
    where the word names no value of the enum. -/
inductive Pat where
  | none
  | some (x : String)
  | bool (b : Bool)
  | enum (v : Option String)
  /-- A value of an enum that has fields, by its declared name, and the name the arm gives the value
      (`placed p`). -/
  | value (v : Option String) (bind : Option String)
  /-- A literal: a number in the unit of what the `match` splits, a date, a string. -/
  | lit (v : Val)
  /-- `else`: every value the arms above do not take. -/
  | rest
  deriving Repr, Inhabited

/-- The builtins that apply a function to each element of a list. -/
inductive Walk where
  | count | sum | any | all | map | filter | first | maxOf | minOf | running
  deriving DecidableEq, Repr, Inhabited

/-- The builtins of DESIGN 24.3 that apply a function to each element of a list (`last` from the
    end), or to each entry or value of a dict, in the order of the keys. -/
inductive WalkK where
  | last | sortBy | sortByDesc | maxBy | minBy | flatMap | filterMap | groupBy
  /-- `d.filter(e => …)`: each entry of a dict. -/
  | filterD
  /-- `d.map_values(v => …)`: each value of a dict. -/
  | mapValues
  deriving DecidableEq, Repr, Inhabited

/-- The builtins of DESIGN 24.3 that take no function: of a dict, of a set, of a list; `concat` of
    lists, `join` of strings and `numbers`; and the builtins of a string (DESIGN 25.2), and `to_text`
    of a list of code points. -/
inductive Prim where
  | get | put | removeD | hasD | keys | values | entries | merge
  | add | removeS | hasS | union | intersect | difference | toList
  | first | last | at | append | take | drop | slice | reverse | sort | distinct | contains | flatten
  | indexed | toSet | join | concat | numbers
  | chars | codePoints | toText | sliceT | takeT | dropT | indexOf | containsT | endsWith | split | words
  | replace | trim | trimStart | trimEnd | lowerAscii | upperAscii | padStart | padEnd | parseNumber | parseDate
  | charOf
  deriving DecidableEq, Repr, Inhabited

/-- How `text` writes a value: a number with its unit's spelling (a rate as a percentage, a
    `number` bare), or anything else as it is. -/
inductive TextFmt where
  | rate
  | number
  | unit (u : String)
  | plain
  deriving Repr, Inhabited

inductive Expr where
  | num (id line : Nat) (v : Rat)
  | date (id : Nat) (d : Int)
  | str (id : Nat) (s : String)
  | bool (id : Nat) (b : Bool)
  | none (id : Nat)
  | enum (id : Nat) (v : String)
  /-- A name in scope. -/
  | var (id line : Nat) (x : String)
  /-- A field of a record. -/
  | field (id line : Nat) (base : Expr) (f : String)
  | neg (id line : Nat) (x : Expr)
  | not (id line : Nat) (x : Expr)
  /-- `x in u`: the conversion, where the units differ. -/
  | inUnit (id line : Nat) (x : Expr) (c : Option Conv)
  /-- `a + b` and the rest: each side read as the interpreter reads it, and the step the answer's
      type says (none where it says none). -/
  | arith (id line : Nat) (op : ArOp) (a b : Expr) (ca cb : Option Conv) (step : Option Rat)
  /-- The days from one date to another. -/
  | ddiff (id : Nat) (a b : Expr)
  /-- A comparison: each side read in the unit they are compared in, how they are compared, and
      the steps of the two sides' types (what a branch narrows a side on). -/
  | cmp (id line : Nat) (op : CmpOp) (a b : Expr) (ca cb : Option Conv) (ord : Order) (sa sb : Option Rat)
  | and (id line : Nat) (a b : Expr)
  | or (id line : Nat) (a b : Expr)
  /-- `if`: each branch's answer read in the `if`'s type. -/
  | ite (id : Nat) (c : Expr) (t f : Expr) (ct cf : Option Conv)
  | mtch (id line : Nat) (s : Expr) (arms : List (List Pat × Expr × Option Conv))
  /-- `let x = v` and the rest of the block. -/
  | letIn (x : String) (v : Expr) (c : Option Conv) (body : Expr)
  /-- A block written under `let x =`: its answer read in the block's type. -/
  | block (id : Nat) (b : Expr) (c : Option Conv)
  /-- A function of the module, by its index: each argument with the parameter it goes to. -/
  | call (id line : Nat) (f : Nat) (args : List (Nat × Expr × Option Conv))
  /-- A rule, by its index: each argument with the input it goes to. -/
  | rule (id line : Nat) (r : Nat) (args : List (Nat × Expr × Option Conv))
  /-- A date of a dates file: the file, the date, each argument with the input it goes to. -/
  | dateCall (id line : Nat) (file : Nat) (date : String) (args : List (Nat × Expr × Option Conv))
  /-- A transfer built: its parameters in the order declared, and each argument with its parameter,
      the line it is written on, and whether the parameter is an amount. -/
  | transfer (id : Nat) (tid : String) (params : List String)
      (args : List (String × Expr × Option Conv × Nat × Bool))
  /-- A record built: its fields in the order declared (each its place and whether it holds a
      number or a date), whether it is a record of a module (or a value of an enum), the fields as
      written; for a value of an enum that has fields, its name (a value with no fields is built with
      none: its name alone); and the value the fields not written are taken from (`..x`, DESIGN
      23.4). -/
  | record (id line : Nat) (module : Bool) (fields : List (String × Slot × Bool))
      (inits : List (Nat × Expr × Option Conv × Nat)) (tag : Option String) (base : Option Expr)
  | list (id : Nat) (elems : List (Expr × Option Conv))
  /-- `count`, `sum`, `any`, `all`, `map`, `filter`, `first`, `max_of`, `min_of`, `running_totals`:
      the list, the function given (its parameter and body), its answers read as the builtin reads
      them, how `max_of` and `min_of` compare, and the step of the sum's or the weight's type. -/
  | walk (id line : Nat) (w : Walk) (list : Expr) (x : String) (body : Expr) (c : Option Conv)
      (ord : Order) (step : Option Rat)
  /-- `reduce(xs, first, (acc, x) => body)`: the running value read in the type of `first`, and the
      answer in the type of `reduce`. -/
  | reduce (id line : Nat) (list first : Expr) (acc x : String) (body : Expr) (c : Option Conv)
      (out : Option Conv)
  | minmax (id line : Nat) (isMin : Bool) (a b : Expr) (ca cb : Option Conv) (ord : Order)
  /-- `mod(x, n)`: the divisor a value or a constant, `zero` at `line` where it is 0 (DESIGN 24.3). -/
  | modOp (id line : Nat) (x n : Expr)
  | isqrt (id line : Nat) (x : Expr)
  /-- `allocate(t, c, s)`: `c` and `s` read in one unit, the answer in the type of `allocate`. -/
  | allocate (id line : Nat) (t c s : Expr) (cc cs : Option Conv) (out : Option Conv)
  /-- A rounding: the step read in the unit of the value, the answer in the type of the call, and
      every value of that type (what the analysis says where the step is not one value). -/
  | round (id line : Nat) (way : Way) (x g : Expr) (cg : Option Conv) (out : Option Conv) (top : Abs)
  | count1 (id : Nat) (list : Expr)
  | startsWith (id : Nat) (s p : Expr)
  | concat (id : Nat) (args : List Expr)
  | text (id : Nat) (x : Expr) (fmt : TextFmt)
  /-- A dict written out (DESIGN 24.2): each key and then its value, from the left, each read in the
      dict's type of keys or of values as it comes; the keys put in their order by `key`. -/
  | dict (id : Nat) (key : Key) (pairs : List (Expr × Option Conv × Expr × Option Conv))
  /-- A builtin that takes no function (DESIGN 24.3): its arguments, the dict, the set or the list it
      is written after first, each read in its place; the order of the keys it holds or orders; and,
      where what it is called on is no dict, set or list the analysis follows, every value of its
      answer's type (what the analysis says there). -/
  | prim (id line : Nat) (op : Prim) (key : Key) (args : List (Expr × Option Conv)) (top : Option Abs)
  /-- A builtin of DESIGN 24.3 that applies a function to each element of a list or to each entry or
      value of a dict: the function given (its parameter and body), its answers read as the builtin
      reads them, and the order of the keys it orders or holds. -/
  | walkK (id line : Nat) (w : WalkK) (coll : Expr) (x : String) (body : Expr) (c : Option Conv) (key : Key)
  /-- `xs.to_dict(x => key, y => value)`: for each element its key, then its value, each read in the
      dict's types. -/
  | toDict (id line : Nat) (key : Key) (list : Expr) (x : String) (kb : Expr) (kc : Option Conv)
      (y : String) (vb : Expr) (vc : Option Conv)
  /-- `d.merge_with(o, (a, b) => …)`: the two dicts, and the function given a key of both. -/
  | mergeWith (id line : Nat) (key : Key) (a b : Expr) (x y : String) (body : Expr) (c : Option Conv)
  /-- A lambda made a value (DESIGN 27.1): the lambda of the module by its id. -/
  | lam (id : Nat)
  /-- A function value called (DESIGN 27.3): what is called, and the arguments in order, each read in
      its parameter's unit; and every value of the type of its answer, marked as the type's (what a
      function the analysis does not see answers, DESIGN 27.4). -/
  | callV (id line : Nat) (f : Expr) (args : List (Expr × Option Conv)) (top : Abs)
  /-- What the check of types does not let through: never in a module that passes it. -/
  | bad (id line : Nat) (args : List Expr)
  deriving Repr, Inhabited

/-- An input of a rule: its name; the range a numeric input declares; the step it travels on the
    wire in (a rate's step, else 1), where it is a number; whether it may be none; whether rulec
    holds it, a date, to a range of days it declares (X3 (a), which ritsu-cross decides: a date
    input whose range is a koyomi date takes the date's days as a precondition, which X2 decides,
    and is not one); for a list the rule walks, the range of each numeric field of its elements;
    for an input of an ordered enum held to a range over its values (`range >=basic <=gold`), the
    values the range lets in (ritsu-ports' `ColumnType::Enum`). -/
structure RuleInput where
  name : String
  range : Option Range
  step : Option Rat
  opt : Bool
  dateRanged : Bool
  elements : Option (List (String × Option Range))
  values : Option (List String) := none
  deriving Repr, Inhabited

/-- A rule: its inputs, what each output can be over every input it takes (rulec's `output_values`,
    and the values of its enums), and its preconditions (X2, which ritsu-cross decides). -/
structure RuleD where
  name : String
  inputs : List RuleInput
  outputs : List (String × Abs)
  pres : List String
  deriving Repr, Inhabited

/-- An input of a dates file: its name, whether it is a date, and its range. -/
structure DateInput where
  name : String
  isDate : Bool
  min : Int
  max : Int
  deriving Repr, Inhabited

/-- What koyomi says of one date of a file: the inputs it reads, the first and last of its days
    over every input of the ranges, and the fewest and most days from its date input to it. -/
structure DateD where
  name : String
  reads : List String
  hull : Option (Int × Int)
  days : Option (Int × Int)
  deriving Repr, Inhabited

structure DatesD where
  inputs : List DateInput
  dates : List DateD
  deriving Repr, Inhabited

/-- A parameter: its name, its place, and every value it can be (what the analysis starts from:
    its type cut to its range, the ranges of the fields it reaches). -/
structure Param where
  name : String
  slot : Slot
  /-- Whether it holds a number or a date (no list, no optional): what a call narrows by its range. -/
  scalar : Bool
  abs : Abs
  deriving Repr, Inhabited

/-- A function: its parameters; the place of its answer; every value its result's type can be,
    cut to its range (`declared`); its body, and the answer's conversion to the result's type; and
    whether a call reads its body again with the values it gives (a generic function, kaname's
    DESIGN 26.5). -/
structure Fn where
  name : String
  line : Nat
  params : List Param
  ret : Slot
  retRanged : Bool
  declared : Abs
  body : Expr
  answer : Nat
  answerLine : Nat
  conv : Option Conv
  reread : Bool := false
  /-- Whether a parameter is of a function type (kaname's DESIGN 27.1): a call that does not read its
      body again takes what it declares (DESIGN 27.4). -/
  takesFn : Bool := false
  deriving Repr, Inhabited

/-- A lambda of the module made a value (kaname's DESIGN 27.1): its id, the function it is written in,
    its parameters each with every value of its type (marked as the type's: what its body is read with
    once where no call reads it with the values it is given, DESIGN 27.4), its body, and its body's
    answer read in the type of its answer. -/
structure Lam where
  id : Nat
  fn : Nat
  params : List (String × Abs)
  body : Expr
  conv : Option Conv
  deriving Repr, Inhabited

/-- A module: its functions, the rules and dates files it calls, and the rings of calls in the
    order the analysis goes in (a ring of more than one function, or one that calls itself, goes
    round). And what the checks across the languages (ritsu-cross's X2, X3 (a), X4, X6) came to at
    the places they decide, where the run hands their verdicts over (kaname's DESIGN 14.5): each
    place by its expression and what it is, and whether the check is shown to hold there. A place no
    verdict names waits for one. -/
structure Module where
  fns : List Fn
  rules : List RuleD
  dates : List DatesD
  rings : List (List Nat × Bool)
  verdicts : List (Nat × String × Bool) := []
  /-- The lambdas the module makes values of (kaname's DESIGN 27.1). -/
  lams : List Lam := []
  /-- The places of the records and the enums that hold themselves its parameters reach, by their
      numbers (`Slot.mk`'s `ref`, kaname's DESIGN 28.1): what a host's argument is held to inside a
      value of one. -/
  slots : List Slot := []
  deriving Repr, Inhabited

/-- What the layers kaname calls answer (kaname's DESIGN 15.1): a rule, its outputs for the values
    of its inputs or a refusal; a date, its day for the values of its inputs or that it stops. -/
structure Oracle where
  rule : Nat → List Val → Option (List Val)
  date : Nat → String → List (String × Int) → Option Int

end KanameCore
