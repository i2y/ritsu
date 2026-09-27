# Design

A `.flow` is parsed, its names and types are resolved (a rule's from what `rulec schema`,
`rulec certificate` and `rulec api` print), and the typed tree it becomes is checked along the flow:
the states of its cases, what is given a value where, every arm, every way out. The reference
interpreter runs it, the scenarios play it, and five generators build it.

## Why a language of its own

rulec decides; it leaves out running a workflow on purpose: the steps themselves, the waits between
them, and the side effects on other systems. A rulec state machine keeps its state with the caller, and
the functions rulec generates stay pure. dandori takes that other half, as a language of its own
outside rulec, for two reasons. Waits, side effects and retries are not rows of a table, which is what
lets rulec treat a state machine as one more declaration. And rulec can prove what it proves because it
is small; a workflow inside it would end that.

The price is a second set of checks, generators and tests. The checks of the decisions themselves
(complete, free of overlaps, units, rounding) stay with rulec, so dandori's checks keep to the shape of
the workflow and the states of its cases.

## Six principles

- **P1. The decisions live in rulec.** A `.flow`'s expressions build values (records, lists, strings
  with values put in) but have no comparison, arithmetic or logic, and a flow branches only by matching
  an enum, a bool, or a value that may be absent. Putting a record together for a task, or an order
  number into a message, decides nothing, so it is allowed; anything that compares or computes is a
  rule, where rulec proves it complete.
- **P2. dandori stays outside rulec.** It reads only the JSON rulec's command line prints, and rulec
  does not know dandori.
- **P3. One reference interpreter says what a `.flow` means,** and what each platform runs is held to
  it.
- **P4. A loop says how many times it may go round,** and there is no recursion, so the length of a
  run's history has a bound.
- **P5. What cannot be known before the run is checked where it comes in.** What the other side
  answers is checked when it arrives, and a value that does not fit fails the run there.
- **P6. A feature whose meaning would differ between the platforms is built only where it can mean
  the same,** and refused elsewhere with E050: cleaning up after a cancellation (`on cancel`) and
  events sent to a workflow by name (`event`) are Temporal's for now. A clause that only changes how or
  at what cost something runs (`queue`, `image`, a rule's `local`) does nothing where it does not
  apply.

P6 came with making Temporal the main platform. Before it, the language had only what every platform
could run; but shutting out what only Temporal can mean narrows what can be written for Temporal, and
giving such a feature another meaning elsewhere would break P3. So it is built, with the reference
interpreter's meaning, where it can be, and refused where it cannot. The reference interpreter still
gives one meaning.

## The whole record

[DESIGN.md](https://github.com/i2y/dandori/blob/main/DESIGN.md), in Japanese, gives the reasons for
these and for every other decision, the designs that were dropped, what is left, and what was run to
check it all.
