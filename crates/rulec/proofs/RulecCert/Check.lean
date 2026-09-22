/-
  The checks a certificate has to pass, as functions (DESIGN §15.97).

  These are not a description of `tools/recheck.py`: they are the thing itself. `Main` reads
  a certificate and runs exactly these, and `RulecCert.Sound` proves that a `true` from each
  one settles the matching claim in `RulecCert.Semantics`. A check here therefore has to
  stay **decidable and small** — one set intersection, one lookup, one walk of a tree the
  certificate carries — because a check that searched would be the tool run twice.

  Two of them take a `Bool` function the certificate cannot supply by itself: `ruledOut`,
  which says a whole box is unreachable, and `possible`, which says one point is reachable.
  Those are questions about the values behind the coordinates, and `RulecCert.Sieve`
  answers them from the `constraints` the certificate carries. Here they are parameters,
  so that the soundness proofs say exactly what they depend on.
-/
import RulecCert.Semantics

namespace RulecCert

/-! ## The rows are distinct

A row is named by its number, and everything below looks a row up by that number. Two rows
sharing one number would leave the pair between them unexamined — neither is earlier than
the other, so the overlap check would pass over it in silence. Nothing in `rulec` emits
such a certificate; the check is here because the proof needs it, and a check the proof
needs is a check a forged document has to pass. -/
def rowsDistinct (rows : List Row) : Bool :=
  rows.Pairwise (fun r q => r.index ≠ q.index)

/-- Every row states one set of coordinates per axis. A row that states fewer, or more, is
    describing a different space from the one the axes set out, and nothing below would
    line up with it. -/
def rowsShaped (arities : List Arity) (rows : List Row) : Bool :=
  rows.all (fun r => r.box.length == arities.length)

/-! ## Overlap (E105) -/

/-- Two boxes part on an axis when the coordinates they take there do not meet. -/
def partsOn (a b : Box) (axis : Nat) : Bool :=
  match a[axis]?, b[axis]? with
  | some xs, some ys => xs.all (fun x => !ys.contains x)
  | _, _ => false

/-- **The overlap check.** For each pair of rows, the certificate names an axis; the pair
    parts there, or the pair is one the certificate does not claim apart and is listed as such.
    A pair that is listed as undecided is not proved apart — `Sound` carries that as a
    hypothesis, and `Main` prints the count rather than hiding it. -/
def pairsPart (rows : List Row) (told : Nat → Nat → Option Nat)
    (undecided : Nat → Nat → Bool) : Bool :=
  rows.all (fun r =>
    rows.all (fun q =>
      if r.index < q.index then
        match told r.index q.index with
        | some axis => partsOn r.box q.box axis
        | none => undecided r.index q.index
      else true))

/-! ## Completeness (E101)

The cover is a tree the certificate carries, and the check is a walk of it. The children of
a node are a `Kids` list rather than a `List Cover` so that the tree is a plain mutual
inductive: the check below is then structural, and the induction in `Sound` is an ordinary
one over a size that `Kids.get?_size_lt` bounds. -/

mutual

/-- A node of the cover. -/
inductive Cover where
  /-- One child per coordinate of the axis at this depth. That the children tile the axis
      is then a matter of the node's shape, not of trust. -/
  | split : Kids → Cover
  /-- This row takes every point below here. -/
  | row : Nat → Cover
  /-- No input reaches any point below here. -/
  | impossible : Cover
  /-- The table above cannot produce this. **Stated, not proved** — re-checking it needs
      the upstream table's own region, which the certificate does not carry. -/
  | upstream : Cover

/-- The children of a `split`, in coordinate order. -/
inductive Kids where
  | nil : Kids
  | cons : Cover → Kids → Kids

end

namespace Kids

def get? : Kids → Nat → Option Cover
  | .nil, _ => none
  | .cons k _, 0 => some k
  | .cons _ ks, n + 1 => ks.get? n

end Kids

/-! How big a cover is. Only the proof uses this: the walk above is structural, and this is
what lets the induction in `Sound` step from a node to one of its children. -/

mutual
def Cover.size : Cover → Nat
  | .split kids => kids.size + 1
  | .row _ => 1
  | .impossible => 1
  | .upstream => 1

def Kids.size : Kids → Nat
  | .nil => 0
  | .cons k ks => k.size + ks.size + 1
end

/-- Does the box take every coordinate of every axis from `depth` on? -/
def spansRest (box : Box) (arities : List Arity) (depth : Nat) : Bool :=
  (List.range (arities.length - depth)).all (fun k =>
    match box[depth + k]?, arities[depth + k]? with
    | some xs, some n => (List.range n).all (fun c => xs.contains c)
    | _, _ => false)

/-- Does the box take the coordinates this path has already fixed? -/
def takesPath (box : Box) (path : Point) : Bool :=
  (List.range path.length).all (fun i =>
    match box[i]?, path[i]? with
    | some xs, some c => xs.contains c
    | _, _ => false)

mutual

/-- **The completeness check.** A `split` must have one child per coordinate of the axis at
    its depth; a `row` must name a row that takes the path so far and every coordinate
    below it; an `impossible` leaf must be a box `ruledOut` really rules out — the whole
    box, not one corner of it, which is the mistake §15.98 was about. Nothing here
    searches: the tree is given, and each leaf is one test. -/
def coverOk (arities : List Arity) (rows : List Row) (ruledOut : Point → Bool) :
    Cover → Point → Bool
  | .split kids, path =>
    match arities[path.length]? with
    | none => false
    | some n => kidsOk arities rows ruledOut kids path 0 n
  | .row i, path =>
    match rows.find? (fun r => r.index == i) with
    | none => false
    | some r => takesPath r.box path && spansRest r.box arities path.length
  | .impossible, path => ruledOut path
  -- A box the tables above rule out is no longer a leaf of its own: the facts they give
  -- are part of the sieve, so such a box arrives here as `.impossible` and is checked
  -- like any other. A document that still writes this leaf gets no free pass (§15.115).
  | .upstream, _ => false

/-- The children of a `split`, checked one coordinate at a time. `left` counts the
    coordinates the axis still owes: the list has to run out exactly when the axis does,
    which is what makes "the children tile the axis" a matter of shape. -/
def kidsOk (arities : List Arity) (rows : List Row) (ruledOut : Point → Bool) :
    Kids → Point → Nat → Nat → Bool
  | .nil, _, _, left => left == 0
  | .cons k ks, path, c, left =>
    left != 0 && coverOk arities rows ruledOut k (path ++ [c]) &&
      kidsOk arities rows ruledOut ks path (c + 1) (left - 1)

end

/-! Whether the cover leans on an upstream claim anywhere. A certificate that does is still
checked, but what comes out is weaker, and `Main` says so rather than printing "ok". -/

mutual
def Cover.leansOnUpstream : Cover → Bool
  | .split kids => kids.leansOnUpstream
  | .row _ => false
  | .impossible => false
  | .upstream => true

def Kids.leansOnUpstream : Kids → Bool
  | .nil => false
  | .cons k ks => k.leansOnUpstream || ks.leansOnUpstream
end

/-! ## Reachability (E102) -/

/-- **The reachability check.** For each row the certificate names a point *and the values
    behind it*; the point has to be in the space, inside that row's box, and — under
    `first` — outside every earlier row's box, and the values have to show that the rule
    really is asked about it. Without the values the claim would be "this row's box is not
    empty", which is not what E102 says. -/
def reachOk (arities : List Arity) (claimed rows : List Row) (policy : Policy)
    (askedOk : Point → List Rat → Bool) (witness : Nat → Option (Point × List Rat)) : Bool :=
  claimed.all (fun r =>
    match witness r.index with
    | none => false
    | some (p, v) =>
      inSpace arities p && askedOk p v && inBox r.box p &&
        (policy != Policy.first ||
          rows.all (fun q => decide (r.index ≤ q.index) || !inBox q.box p)))

end RulecCert
