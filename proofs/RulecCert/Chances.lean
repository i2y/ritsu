/-
  The claims of `chances` (rulec's §15.224–§15.226, §15.243).

  A rule that marks inputs `random` says the caller draws each of them uniformly from the values it
  takes — the whole steps of a number's range, an enum's values, `false` and `true` — and a row of a
  `chances` table claims that for every combination of the other inputs its cells let in, the share of
  those draws whose answer passes the row's output cells is the chance it writes, or passes the
  comparisons it writes. This file says what such a claim means and proves that the checks the program
  runs on the certificate settle it, in each of the three ways the check counts.

  **Box by box** (`Claim`). The walk that settles the claims cuts every column the rule reads on its
  own boundaries and the claims'; a class is a coordinate on each axis that is not random, a box a
  coordinate on each random axis. Three things make the count exact:

  - **The coordinates of a random axis tile the values its input is drawn from** (`chain_flatMap`,
    `tiles_flatMap`): side by side from the first value to the last, with none left out and none twice,
    so the points of a box are the product of its coordinates' and every draw is in one box
    (`countP_cart_flatMap`).
  - **Inside a box, one row of each table answers, and the answer is one value**: every value the box
    holds stands on a coordinate of each table axis that the box meets (`nearOf_sound`), and the rows
    answer alike at all of those, so the value the tables answer at a draw is the one the program works
    out for the box, from the rows the rule's file is read back to (`cellOf_sound`, `Claim.box_const`).
  - **The arithmetic**: the hits over the points pass the claim, compared as rationals.

  `Claim.holds` puts the three together with the tiling of the axes that are not random (`FAx.cover`):
  whatever values the other inputs take inside the row's cells, the share of the draws whose answer
  passes is the one the classes count (`Claim.count_share`), and it passes the claim.

  **Point by point** (`RunClaim`). Where the answer moves inside a box, the check runs every draw for
  every combination of the other inputs the cells let in, and so does the program: the values the rule
  computes are worked out exactly (`envExact_sound`), and the tables answer as above
  (`RunClaim.pointHit_sound`). `RunClaim.holds` says that among the draws the door lets through, the
  share whose answer passes passes the claim.

  **By weight** (`WeightedClaim`). A fold that picks by weight walks a sequence the certificate carries
  element by element, read back out of the file; the program runs the walk at every draw
  (`WeightedClaim.pointHit_sound`), and `WeightedClaim.holds` says the same of it.
-/
import RulecCert.Axes

namespace RulecCert.Chances

open RulecCert

/-! ## Counting the points of a product -/

/-- Every way of picking one element of each list, the first list's pick outermost. -/
def cart {α : Type} : List (List α) → List (List α)
  | [] => [[]]
  | g :: gs => g.flatMap (fun v => (cart gs).map (v :: ·))

theorem sum_map_add {β : Type} (f g : β → Nat) :
    ∀ (l : List β), (l.map (fun b => f b + g b)).sum = (l.map f).sum + (l.map g).sum
  | [] => rfl
  | b :: l => by
    simp only [List.map_cons, List.sum_cons, sum_map_add f g l]
    omega

theorem sum_map_zero {β : Type} : ∀ (l : List β), (l.map (fun _ => (0 : Nat))).sum = 0
  | [] => rfl
  | _ :: l => by simp only [List.map_cons, List.sum_cons, sum_map_zero l]

/-- Two finite sums taken the other way round. -/
theorem sum_swap {α β : Type} (f : α → β → Nat) (m : List β) :
    ∀ (l : List α), (l.map (fun a => (m.map (f a)).sum)).sum = (m.map (fun b => (l.map (fun a => f a b)).sum)).sum
  | [] => by
    simp only [List.map_nil, List.sum_nil]
    exact (sum_map_zero m).symm
  | a :: l => by
    simp only [List.map_cons, List.sum_cons, sum_swap f m l]
    exact (sum_map_add (f a) (fun b => (l.map (fun a => f a b)).sum) m).symm

theorem sum_flatMap {α β : Type} (g : α → List β) (h : β → Nat) :
    ∀ (l : List α), ((l.flatMap g).map h).sum = (l.map (fun a => ((g a).map h).sum)).sum
  | [] => rfl
  | a :: l => by
    simp only [List.flatMap_cons, List.map_append, List.sum_append, List.map_cons, List.sum_cons, sum_flatMap g h l]

theorem countP_cons_map {α : Type} (E : List α → Bool) (v : α) (l : List (List α)) :
    (l.map (v :: ·)).countP E = l.countP (fun r => E (v :: r)) := by
  rw [List.countP_map]
  rfl

/-- **Counting a product box by box.** Where each list is the points of its coordinates one after
    another, the points of the product that pass `E` are the ones each box of coordinates holds,
    added up over every box. -/
theorem countP_cart_flatMap {α γ : Type} (pts : γ → List α) :
    ∀ (cs : List (List γ)) (E : List α → Bool),
      (cart (cs.map (fun c => c.flatMap pts))).countP E =
        ((cart cs).map (fun b => (cart (b.map pts)).countP E)).sum
  | [], E => by simp [cart]
  | c :: cs, E => by
    simp only [List.map_cons, cart, List.countP_flatMap]
    have hl : ∀ v, (List.countP E ∘ fun v => List.map (fun x => v :: x) (cart (List.map (fun c => c.flatMap pts) cs))) v
        = ((cart cs).map (fun b => (cart (b.map pts)).countP (fun r => E (v :: r)))).sum := by
      intro v
      simp only [Function.comp, countP_cons_map]
      exact countP_cart_flatMap pts cs (fun r => E (v :: r))
    rw [List.map_congr_left (fun v _ => hl v), sum_flatMap pts]
    rw [sum_flatMap (fun g => (cart cs).map (g :: ·)) (fun b => (cart (b.map pts)).countP E)]
    congr 1
    apply List.map_congr_left
    intro g _
    rw [sum_swap (fun v b => (cart (b.map pts)).countP (fun r => E (v :: r))) (cart cs) (pts g)]
    simp only [List.map_map]
    congr 1
    apply List.map_congr_left
    intro b _
    simp only [Function.comp, List.map_cons, cart, List.countP_flatMap]
    congr 1
    apply List.map_congr_left
    intro v _
    simp only [Function.comp, countP_cons_map]

/-- A box whose every point answers alike counts all its points or none. -/
theorem countP_const {α : Type} {E : List α → Bool} {l : List (List α)} {h : Bool}
    (hc : ∀ r ∈ l, E r = h) : l.countP E = if h then l.length else 0 := by
  cases h with
  | true =>
    simp only [↓reduceIte]
    rw [List.countP_eq_length]
    intro r hr
    exact hc r hr
  | false =>
    simp only [Bool.false_eq_true, ↓reduceIte]
    rw [List.countP_eq_zero]
    intro r hr
    simp [hc r hr]

theorem length_cart {α : Type} : ∀ (gs : List (List α)), (cart gs).length = (gs.map List.length).prod
  | [] => rfl
  | g :: gs => by
    simp only [cart, List.length_flatMap, List.length_map, length_cart gs, List.map_cons, List.prod_cons]
    rw [List.map_congr_left (fun _ _ => rfl), sum_map_const]
where
  sum_map_const {β : Type} {k : Nat} : ∀ (l : List β), (l.map (fun _ => k)).sum = l.length * k
    | [] => by simp
    | _ :: l => by
      simp only [List.map_cons, List.sum_cons, List.length_cons, sum_map_const l]
      rw [Nat.add_mul, Nat.one_mul, Nat.add_comm]

/-- Every point of a product picks one element of each list. -/
theorem mem_cart {α : Type} : ∀ {gs : List (List α)} {r : List α}, r ∈ cart gs →
    r.length = gs.length ∧ ∀ (j : Nat) (v : α) (g : List α), r[j]? = some v → gs[j]? = some g → v ∈ g
  | [], r, h => by
    simp only [cart, List.mem_singleton] at h
    subst h
    exact ⟨rfl, fun j v g hv _ => by simp at hv⟩
  | g :: gs, r, h => by
    simp only [cart, List.mem_flatMap, List.mem_map] at h
    obtain ⟨v, hv, r', hr', rfl⟩ := h
    obtain ⟨hlen, hin⟩ := mem_cart hr'
    refine ⟨by simp [hlen], fun j w g' hw hg => ?_⟩
    cases j with
    | zero =>
      simp only [List.getElem?_cons_zero, Option.some.injEq] at hw hg
      subst hw; subst hg
      exact hv
    | succ j => exact hin j w g' (by simpa using hw) (by simpa using hg)

/-! ## The values a random input is drawn from, and the coordinates that tile them -/

/-- The whole numbers from `a` to `b`, none where `b < a`. -/
def irange (a b : Int) : List Int := (List.range (b + 1 - a).toNat).map (fun (k : Nat) => a + (k : Int))

theorem length_irange (a b : Int) : (irange a b).length = (b + 1 - a).toNat := by
  simp [irange]

theorem mem_irange {a b n : Int} : n ∈ irange a b ↔ a ≤ n ∧ n ≤ b := by
  simp only [irange, List.mem_map, List.mem_range]
  constructor
  · rintro ⟨k, hk, rfl⟩
    omega
  · intro h
    refine ⟨(n - a).toNat, by omega, by omega⟩

/-- Two runs of whole numbers side by side make one. -/
theorem irange_append {a b c : Int} (h1 : a ≤ b + 1) (h2 : b ≤ c) :
    irange a b ++ irange (b + 1) c = irange a c := by
  have hsplit : (c + 1 - a).toNat = (b + 1 - a).toNat + (c + 1 - (b + 1)).toNat := by omega
  simp only [irange]
  rw [hsplit, List.range_add, List.map_append, List.map_map]
  congr 1
  apply List.map_congr_left
  intro k _
  simp only [Function.comp]
  omega

/-- **Index runs side by side**: the first begins at `lo`, each next one begins where the one before
    ends, the last ends at `hi`; a run may hold nothing (`j = i - 1`). -/
def chainB (lo hi : Int) : List (Int × Int) → Bool
  | [] => lo == hi + 1
  | ij :: rest => ij.1 == lo && decide (ij.1 ≤ ij.2 + 1) && chainB (ij.2 + 1) hi rest

theorem chainB_le {lo hi : Int} : ∀ {cs : List (Int × Int)}, chainB lo hi cs = true → lo ≤ hi + 1
  | [], h => by simp only [chainB, beq_iff_eq] at h; omega
  | ij :: rest, h => by
    simp only [chainB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    have := chainB_le h.2
    omega

/-- **Runs side by side are the whole run**: the numbers of each, one run after the other, are the
    numbers from `lo` to `hi`, each once. -/
theorem chain_flatMap {lo hi : Int} : ∀ {cs : List (Int × Int)}, chainB lo hi cs = true →
    cs.flatMap (fun ij => irange ij.1 ij.2) = irange lo hi
  | [], h => by
    simp only [chainB, beq_iff_eq] at h
    simp only [List.flatMap_nil, irange]
    rw [show (hi + 1 - lo).toNat = 0 by omega]
    rfl
  | ij :: rest, h => by
    simp only [chainB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    obtain ⟨⟨h1, h2⟩, h3⟩ := h
    have hle := chainB_le h3
    rw [List.flatMap_cons, chain_flatMap h3, ← h1]
    exact irange_append h2 (by omega)

/-- A value of a column: a number, or a word — an enum's value, a truth value, the absent value. -/
inductive Val where
  | num : Rat → Val
  | word : String → Val
  deriving DecidableEq, Repr, Inhabited

/-- A number's whole steps of `1/k` from the `i`-th to the `j`-th. -/
def steps (k : Nat) (i j : Int) : List Val := (irange i j).map (fun (n : Int) => Val.num ((n : Rat) / (k : Rat)))

/-- A coordinate of a random axis, with what says which points it holds: a number's interval and the
    run of whole steps of its grid inside it, or a word and whether the input is drawn from it (an
    ordered enum's `range` leaves some of its values out). -/
inductive RC where
  | num (x : Coord) (k : Nat) (i j : Int)
  | word (w : String) (drawn : Bool)
  deriving Repr, Inhabited

/-- The points a coordinate of a random axis holds. -/
def RC.pts : RC → List Val
  | .num _ k i j => steps k i j
  | .word w d => if d then [.word w] else []

/-- How many points it holds, worked out without listing them. -/
def RC.count : RC → Nat
  | .num _ _ i j => (j + 1 - i).toNat
  | .word _ d => if d then 1 else 0

theorem RC.count_eq (c : RC) : c.count = c.pts.length := by
  cases c with
  | num x k i j => simp [RC.count, RC.pts, steps, length_irange]
  | word w d => cases d <;> simp [RC.count, RC.pts]

/-- The first whole number at or above `x`, and the last at or below it. -/
def ceilI (x : Rat) : Int := -((-x).floor)
def floorI (x : Rat) : Int := x.floor

/-- The run of whole steps of `1/k` an interval of the axis holds: a point's step where it is one, the
    steps strictly between an interval's ends. -/
def runOf (k : Nat) : Coord → Option (Int × Int)
  | .exactly a => some (ceilI (a * k), floorI (a * k))
  | .between (some a) (some b) => some (floorI (a * k) + 1, ceilI (b * k) - 1)
  | .between _ _ => none

/-- A coordinate of a random axis is the one its interval says. -/
def RC.okB : RC → Bool
  | .num x k i j => decide (0 < k) && runOf k x == some (i, j)
  | .word _ _ => true

/-- What a coordinate of the walk stands for: a number's interval, or a word. -/
inductive WCoord where
  | num : Coord → WCoord
  | word : String → WCoord
  deriving Repr, Inhabited

def WCoord.holds : WCoord → Val → Prop
  | .num x, .num v => x.holds v
  | .word w, .word u => u = w
  | _, _ => False

def RC.coord : RC → WCoord
  | .num x _ _ _ => .num x
  | .word w _ => .word w

theorem floorI_le (x : Rat) : ((floorI x : Int) : Rat) ≤ x := Rat.floor_le x

theorem lt_floorI_add_one (x : Rat) : x < ((floorI x + 1 : Int) : Rat) := Rat.lt_floor_add_one x

theorem le_ceilI (x : Rat) : x ≤ ((ceilI x : Int) : Rat) := by
  have := Rat.floor_le (-x)
  simp only [ceilI, Rat.intCast_neg]
  have h2 := Rat.neg_le_neg this
  simpa using h2

theorem ceilI_sub_one_lt (x : Rat) : ((ceilI x - 1 : Int) : Rat) < x := by
  have := Rat.lt_floor_add_one (-x)
  simp only [ceilI, Rat.intCast_sub, Rat.intCast_neg, Rat.intCast_one]
  simp only [Rat.intCast_add, Rat.intCast_one] at this
  have h2 := Rat.neg_lt_neg this
  rw [Rat.neg_add, Rat.neg_neg] at h2
  rw [Rat.sub_eq_add_neg]
  exact h2

/-- **The points of a coordinate are values it holds.** -/
theorem RC.holds_of_mem {c : RC} (hok : c.okB = true) {v : Val} (hv : v ∈ c.pts) : c.coord.holds v := by
  cases c with
  | word w d =>
    cases d with
    | true =>
      simp only [RC.pts, ↓reduceIte, List.mem_singleton] at hv
      subst hv
      rfl
    | false => simp [RC.pts] at hv
  | num x k i j =>
    simp only [RC.okB, Bool.and_eq_true, decide_eq_true_eq, beq_iff_eq] at hok
    obtain ⟨hk, hrun⟩ := hok
    simp only [RC.pts, steps, List.mem_map] at hv
    obtain ⟨n, hn, rfl⟩ := hv
    obtain ⟨hi, hj⟩ := mem_irange.1 hn
    have hk' : (0 : Rat) < (k : Rat) := by exact_mod_cast hk
    show x.holds ((n : Rat) / (k : Rat))
    match x, hrun with
    | .exactly a, hrun =>
      simp only [runOf, Option.some.injEq, Prod.mk.injEq] at hrun
      obtain ⟨rfl, rfl⟩ := hrun
      have h1 : a * k ≤ (n : Rat) := Rat.le_trans (le_ceilI _) (Rat.intCast_le_intCast.2 hi)
      have h2 : (n : Rat) ≤ a * k := Rat.le_trans (Rat.intCast_le_intCast.2 hj) (floorI_le _)
      have : (n : Rat) = a * k := Rat.le_antisymm h2 h1
      show (n : Rat) / k = a
      rw [this, Rat.mul_div_cancel (Rat.ne_of_gt hk')]
    | .between (some a) (some b), hrun =>
      simp only [runOf, Option.some.injEq, Prod.mk.injEq] at hrun
      obtain ⟨rfl, rfl⟩ := hrun
      refine ⟨fun a' ha => ?_, fun b' hb => ?_⟩
      · cases ha
        have h1 : a * k < (n : Rat) := ltle (lt_floorI_add_one _) (Rat.intCast_le_intCast.2 hi)
        rw [Rat.lt_div_iff hk']
        exact h1
      · cases hb
        have h1 : (n : Rat) < b * k := lelt (Rat.intCast_le_intCast.2 hj) (ceilI_sub_one_lt _)
        rw [Rat.div_lt_iff hk']
        exact h1

/-! ### A random axis tiles the values its input is drawn from -/

/-- What a random input is drawn from: the whole steps of `1/k` from its range's low end to its high
    end, or its words — an enum's values (those its range lets in), `false` and `true`. -/
inductive RIn where
  | num (lo hi : Rat) (k : Nat)
  | words (ws : List String)
  deriving Repr, Inhabited

/-- **The values a random input is drawn from**, lowest first. -/
def RIn.grid : RIn → List Val
  | .num lo hi k => steps k (ceilI (lo * k)) (floorI (hi * k))
  | .words ws => ws.map Val.word

/-- The run of whole steps a coordinate holds, for the chain. -/
def RC.run : RC → Int × Int
  | .num _ _ i j => (i, j)
  | .word _ _ => (0, -1)

/-- The word a coordinate of a random enum or truth value is drawn as, where it is. -/
def RC.drawnWord : RC → Option String
  | .word w true => some w
  | _ => none

/-- **The tiling check of a random axis**: each coordinate the one its interval says, and for a number,
    their runs of whole steps side by side from the first value drawn to the last; for words, the words
    drawn are the input's, in order, each once. -/
def tilesB (inp : RIn) (cs : List RC) : Bool :=
  cs.all RC.okB &&
    match inp with
    | .num lo hi k =>
      cs.all (fun c => match c with
        | .num _ k' _ _ => k' == k
        | .word _ _ => false) &&
      chainB (ceilI (lo * k)) (floorI (hi * k)) (cs.map RC.run)
    | .words ws =>
      cs.all (fun c => match c with
        | .word _ _ => true
        | .num _ _ _ _ => false) &&
      cs.filterMap RC.drawnWord == ws

theorem flatMap_num_pts {k : Nat} : ∀ (cs : List RC),
    cs.all (fun c => match c with | .num _ k' _ _ => k' == k | .word _ _ => false) = true →
    cs.flatMap RC.pts = ((cs.map RC.run).flatMap (fun ij => irange ij.1 ij.2)).map
      (fun (n : Int) => Val.num ((n : Rat) / (k : Rat)))
  | [], _ => rfl
  | c :: cs, h => by
    simp only [List.all_cons, Bool.and_eq_true] at h
    obtain ⟨hc, hcs⟩ := h
    rw [List.flatMap_cons, flatMap_num_pts cs hcs, List.map_cons, List.flatMap_cons, List.map_append]
    congr 1
    match c, hc with
    | .num x k' i j, hc =>
      have : k' = k := eq_of_beq hc
      subst this
      rfl

theorem flatMap_word_pts : ∀ (cs : List RC),
    cs.all (fun c => match c with | .word _ _ => true | .num _ _ _ _ => false) = true →
    cs.flatMap RC.pts = (cs.filterMap RC.drawnWord).map Val.word
  | [], _ => rfl
  | c :: cs, h => by
    simp only [List.all_cons, Bool.and_eq_true] at h
    obtain ⟨hc, hcs⟩ := h
    rw [List.flatMap_cons, flatMap_word_pts cs hcs]
    match c, hc with
    | .word w true, _ => simp [RC.pts, RC.drawnWord]
    | .word w false, _ => simp [RC.pts, RC.drawnWord, List.filterMap_cons]

/-- **A random axis that passes the tiling check holds every value its input is drawn from, in
    order, each once**: its coordinates' points one after the other are the input's values. -/
theorem tiles_flatMap {inp : RIn} {cs : List RC} (h : tilesB inp cs = true) : cs.flatMap RC.pts = inp.grid := by
  simp only [tilesB, Bool.and_eq_true] at h
  obtain ⟨_, h⟩ := h
  cases inp with
  | num lo hi k =>
    simp only [Bool.and_eq_true] at h
    rw [flatMap_num_pts cs h.1, chain_flatMap h.2]
    rfl
  | words ws =>
    simp only [Bool.and_eq_true, beq_iff_eq] at h
    rw [flatMap_word_pts cs h.1, h.2]
    rfl

/-- The whole numbers at or above a value are the ones at or above its ceiling. -/
theorem ceilI_le_iff {x : Rat} {n : Int} : ceilI x ≤ n ↔ x ≤ (n : Rat) := by
  constructor
  · intro h
    exact Rat.le_trans (le_ceilI x) (Rat.intCast_le_intCast.2 h)
  · intro h
    refine Int.not_lt.1 (fun hlt => ?_)
    have h1 : n + 1 ≤ ceilI x := Int.add_one_le_of_lt hlt
    have h2 : (n : Rat) < x := by
      have := ceilI_sub_one_lt x
      have h3 : ((n : Int) : Rat) ≤ ((ceilI x - 1 : Int) : Rat) := Rat.intCast_le_intCast.2 (by omega)
      exact lelt h3 this
    exact absurd h (Rat.not_le.2 h2)

/-- The whole numbers at or below a value are the ones at or below its floor. -/
theorem le_floorI_iff {x : Rat} {n : Int} : n ≤ floorI x ↔ (n : Rat) ≤ x := by
  constructor
  · intro h
    exact Rat.le_trans (Rat.intCast_le_intCast.2 h) (floorI_le x)
  · intro h
    refine Int.not_lt.1 (fun hlt => ?_)
    have h1 : floorI x + 1 ≤ n := Int.add_one_le_of_lt hlt
    have h2 := lt_floorI_add_one x
    have h3 : ((floorI x + 1 : Int) : Rat) ≤ (n : Rat) := Rat.intCast_le_intCast.2 h1
    exact absurd h (Rat.not_le.2 (ltle h2 h3))

theorem le_div_iff' {a b c : Rat} (hc : 0 < c) : a ≤ b / c ↔ a * c ≤ b := by
  rw [← Rat.not_lt, ← Rat.not_lt, Rat.div_lt_iff hc]

theorem div_le_iff' {a b c : Rat} (hc : 0 < c) : a / c ≤ b ↔ a ≤ b * c := by
  rw [← Rat.not_lt, ← Rat.not_lt, Rat.lt_div_iff hc]

/-- **What a number is drawn from, said plainly**: a value `n/k` for a whole number `n`, inside the
    range. -/
theorem mem_grid_num {lo hi : Rat} {k : Nat} (hk : 0 < k) {v : Val} :
    v ∈ (RIn.num lo hi k).grid ↔ ∃ n : Int, v = .num ((n : Rat) / (k : Rat)) ∧ lo ≤ (n : Rat) / k ∧ (n : Rat) / k ≤ hi := by
  have hk' : (0 : Rat) < (k : Rat) := by exact_mod_cast hk
  simp only [RIn.grid, steps, List.mem_map, mem_irange]
  constructor
  · rintro ⟨n, ⟨h1, h2⟩, rfl⟩
    refine ⟨n, rfl, ?_, ?_⟩
    · rw [le_div_iff' hk']
      exact ceilI_le_iff.1 h1
    · rw [div_le_iff' hk']
      exact le_floorI_iff.1 h2
  · rintro ⟨n, rfl, h1, h2⟩
    refine ⟨n, ⟨?_, ?_⟩, rfl⟩
    · exact ceilI_le_iff.2 ((le_div_iff' hk').1 h1)
    · exact le_floorI_iff.2 ((div_le_iff' hk').1 h2)

/-! ## What the tables answer: at the values of a call, and at a cell of the walk -/

/-- Whether an interval of numbers holds a value. -/
def _root_.RulecCert.Coord.holdsB : Coord → Rat → Bool
  | .exactly a, v => v == a
  | .between lo hi, v =>
    (match lo with | some a => decide (a < v) | none => true) &&
    (match hi with | some b => decide (v < b) | none => true)

theorem _root_.RulecCert.Coord.holdsB_iff {x : Coord} {v : Rat} : x.holdsB v = true ↔ x.holds v := by
  cases x with
  | exactly a => simp [Coord.holdsB, Coord.holds]
  | between lo hi =>
    cases lo <;> cases hi <;> simp [Coord.holdsB, Coord.holds]

def WCoord.holdsB : WCoord → Val → Bool
  | .num x, .num v => x.holdsB v
  | .word w, .word u => u == w
  | _, _ => false

theorem WCoord.holdsB_iff {c : WCoord} {v : Val} : c.holdsB v = true ↔ c.holds v := by
  cases c with
  | num x => cases v with
    | num v => exact Coord.holdsB_iff
    | word u => simp [WCoord.holdsB, WCoord.holds]
  | word w => cases v with
    | num v => simp [WCoord.holdsB, WCoord.holds]
    | word u => simp [WCoord.holdsB, WCoord.holds]

/-- What a row writes into a column it decides: a value — a word, a number written in the row — the
    value of a name, or nothing this program reads. -/
inductive Out where
  | val : Val → Out
  | name : String → Out
  | none : Out
  deriving Repr, Inhabited

/-- A table, as the answers read it: its columns and the coordinates of each one's axis, its rows and
    policy, the columns it decides and what each row writes into them. -/
structure ATable where
  columns : List String
  coords : List (List WCoord)
  rows : List Row
  policy : Policy
  decides : List String
  /-- What each row writes into the columns the table decides, in their order, by row number. -/
  outs : Nat → List Out

/-- The coordinate of an axis a value stands on: the first that holds it. -/
def firstHolds : List WCoord → Val → Option Nat
  | [], _ => none
  | c :: cs, v => if c.holdsB v then some 0 else (firstHolds cs v).map (· + 1)

/-- The row of a table that answers at a point: under `first`, the earliest whose box takes it; under
    `unique`, the only one. -/
def ATable.rowAt (T : ATable) (p : Point) : Option Row :=
  match T.policy with
  | .first => T.rows.find? (fun r => inBox r.box p)
  | .unique =>
    match T.rows.filter (fun r => inBox r.box p) with
    | [r] => some r
    | _ => none

/-- What a row writes into a column the table decides. -/
def ATable.outOf (T : ATable) (r : Row) (col : String) : Out :=
  match T.decides.idxOf? col with
  | some i => ((T.outs r.index)[i]?).getD .none
  | none => .none

/-- The coordinate of one axis the value of its column stands on, at a call. -/
def ptEnv (env : String → Option Val) (cc : String × List WCoord) : Option Nat := (env cc.1).bind (firstHolds cc.2)

/-- **What a table answers a call with**, in a column it decides: the values of its columns stand on
    one coordinate of each axis, and the row that answers at that point writes the value. -/
def ATable.answer (T : ATable) (env : String → Option Val) (col : String) : Option Val := do
  let p ← (T.columns.zip T.coords).mapM (ptEnv env)
  let r ← T.rowAt p
  match T.outOf r col with
  | .val v => some v
  | .name n => env n
  | .none => none

/-- **The values of a call**: a column a table decides is what that table answers, read on the values
    of its own columns; any other name has the value the call gives it. `fuel` is how many tables deep
    the answer is read, at most. -/
def valueOf (Ts : List ATable) (env : String → Option Val) : Nat → String → Option Val
  | 0, n => if Ts.any (fun T => T.decides.contains n) then none else env n
  | fuel + 1, n =>
    match Ts.find? (fun T => T.decides.contains n) with
    | some T => T.answer (valueOf Ts env fuel) n
    | none => env n

/-- What a column holds at a cell of the walk: every value a coordinate of its axis holds; the values
    on the grid `1/k` from its `i`-th step to its `j`-th — what a random input is drawn from inside a
    box, and what an input that is not random takes inside a class — or one value the tables give it
    there. -/
inductive CVal where
  | coord : WCoord → CVal
  | run : Nat → Int → Int → CVal
  | val : Val → CVal
  deriving Repr, Inhabited

def CVal.holds : CVal → Val → Prop
  | .coord c, v => c.holds v
  | .run k i j, v => ∃ n : Int, i ≤ n ∧ n ≤ j ∧ v = .num ((n : Rat) / (k : Rat))
  | .val u, v => v = u

/-- Every value of a number's interval `y` is a value of `x`. -/
def coordInside (y x : Coord) : Bool :=
  match y with
  | .exactly a => x.holdsB a
  | .between ly hy =>
    match x with
    | .exactly _ => false
    | .between lx hx =>
      (match lx with
       | none => true
       | some l => match ly with | some a => decide (l ≤ a) | none => false) &&
      (match hx with
       | none => true
       | some h => match hy with | some b => decide (b ≤ h) | none => false)

/-- No value of a number's interval `y` is a value of `x`. -/
def coordApart (y x : Coord) : Bool :=
  match y with
  | .exactly a => !x.holdsB a
  | .between ly hy =>
    match x with
    | .exactly b => !(Coord.between ly hy).holdsB b
    | .between lx hx =>
      (match hy, lx with | some b, some l => decide (b ≤ l) | _, _ => false) ||
      (match hx, ly with | some h, some a => decide (h ≤ a) | _, _ => false)

theorem coordInside_sound {y x : Coord} (h : coordInside y x = true) {v : Rat} (hv : y.holds v) : x.holds v := by
  match y, x, h with
  | .exactly a, x, h =>
    have : v = a := hv
    subst this
    exact Coord.holdsB_iff.1 h
  | .between ly hy, .between lx hx, h =>
    simp only [coordInside, Bool.and_eq_true] at h
    obtain ⟨h1, h2⟩ := h
    refine ⟨fun l hl => ?_, fun u hu => ?_⟩
    · subst hl
      match ly, h1 with
      | some a, h1 =>
        have hla : l ≤ a := by simpa using h1
        exact lelt hla (hv.1 a rfl)
    · subst hu
      match hy, h2 with
      | some b, h2 =>
        have hbu : b ≤ u := by simpa using h2
        exact ltle (hv.2 b rfl) hbu

theorem coordApart_sound {y x : Coord} (h : coordApart y x = true) {v : Rat} (hv : y.holds v) : ¬ x.holds v := by
  intro hxv
  match y, x, h with
  | .exactly a, x, h =>
    have : v = a := hv
    subst this
    simp only [coordApart, Bool.not_eq_true'] at h
    have := Coord.holdsB_iff.2 hxv
    rw [this] at h
    exact absurd h (by simp)
  | .between ly hy, .exactly b, h =>
    simp only [coordApart, Bool.not_eq_true'] at h
    have hvb : v = b := hxv
    subst hvb
    have := Coord.holdsB_iff.2 hv
    rw [this] at h
    exact absurd h (by simp)
  | .between ly hy, .between lx' hx', h =>
    simp only [coordApart, Bool.or_eq_true] at h
    rcases h with h | h
    · match hy, lx', h with
      | some b, some l, h =>
        have hbl : b ≤ l := by simpa using h
        exact absurd (ltle (hxv.1 l rfl) (Rat.le_of_lt (hv.2 b rfl))) (Rat.not_lt.2 hbl)
    · match hx', ly, h with
      | some u, some a, h =>
        have hua : u ≤ a := by simpa using h
        exact absurd (ltle (hv.1 a rfl) (Rat.le_of_lt (hxv.2 u rfl))) (Rat.not_lt.2 hua)

/-- The whole steps of `1/k` a coordinate of a table's axis holds, from the first to the last: an end
    `none` where the coordinate runs on. -/
def tRun (k : Nat) : Coord → Option Int × Option Int
  | .exactly a => (some (ceilI (a * k)), some (floorI (a * k)))
  | .between lo hi => (lo.map (fun a => floorI (a * k) + 1), hi.map (fun b => ceilI (b * k) - 1))

/-- A whole number inside the run a pair of ends gives. -/
def inRun (r : Option Int × Option Int) (n : Int) : Prop :=
  (∀ a, r.1 = some a → a ≤ n) ∧ (∀ b, r.2 = some b → n ≤ b)

/-- **A step of `1/k` is held by a coordinate exactly where it is in the coordinate's run.** -/
theorem holds_iff_inRun {k : Nat} (hk : 0 < k) (x : Coord) (n : Int) :
    x.holds ((n : Rat) / (k : Rat)) ↔ inRun (tRun k x) n := by
  have hk' : (0 : Rat) < (k : Rat) := by exact_mod_cast hk
  cases x with
  | exactly a =>
    simp only [Coord.holds, tRun, inRun, Option.some.injEq, forall_eq']
    constructor
    · intro h
      have hn : (n : Rat) = a * k := by
        rw [← h, Rat.div_mul_cancel (Rat.ne_of_gt hk')]
      refine ⟨ceilI_le_iff.2 (by rw [hn]; exact Rat.le_refl), le_floorI_iff.2 (by rw [hn]; exact Rat.le_refl)⟩
    · rintro ⟨h1, h2⟩
      have hn : (n : Rat) = a * k := Rat.le_antisymm (le_floorI_iff.1 h2) (ceilI_le_iff.1 h1)
      rw [hn, Rat.mul_div_cancel (Rat.ne_of_gt hk')]
  | between lo hi =>
    simp only [Coord.holds, tRun, inRun]
    constructor
    · rintro ⟨h1, h2⟩
      refine ⟨fun a' ha' => ?_, fun b' hb' => ?_⟩
      · cases lo with
        | none => simp at ha'
        | some l =>
          simp only [Option.map_some, Option.some.injEq] at ha'
          subst ha'
          have h3 : l * k < (n : Rat) := (Rat.lt_div_iff hk').1 (h1 l rfl)
          refine Int.add_one_le_of_lt (Int.lt_iff_add_one_le.2 ?_)
          refine Int.add_one_le_of_lt ?_
          have h4 : ((floorI (l * k) : Int) : Rat) < (n : Rat) := lelt (floorI_le _) h3
          exact Rat.intCast_lt_intCast.1 h4
      · cases hi with
        | none => simp at hb'
        | some u =>
          simp only [Option.map_some, Option.some.injEq] at hb'
          subst hb'
          have h3 : (n : Rat) < u * k := (Rat.div_lt_iff hk').1 (h2 u rfl)
          have h4 : (n : Rat) < ((ceilI (u * k) : Int) : Rat) := ltle h3 (le_ceilI _)
          have h5 : n < ceilI (u * k) := Rat.intCast_lt_intCast.1 h4
          omega
    · rintro ⟨h1, h2⟩
      refine ⟨fun a ha => ?_, fun b hb => ?_⟩
      · subst ha
        have h3 := h1 _ rfl
        have h4 : (a * k) < (n : Rat) := ltle (lt_floorI_add_one _) (Rat.intCast_le_intCast.2 h3)
        exact (Rat.lt_div_iff hk').2 h4
      · subst hb
        have h3 := h2 _ rfl
        have h4 : (n : Rat) < b * k := lelt (Rat.intCast_le_intCast.2 h3) (ceilI_sub_one_lt _)
        exact (Rat.div_lt_iff hk').2 h4

/-- Whether a run meets the steps `i`–`j`. -/
def meetsB (r : Option Int × Option Int) (i j : Int) : Bool :=
  (match r.1 with | some a => decide (a ≤ j) | none => true) &&
  (match r.2 with | some b => decide (i ≤ b) | none => true)

/-- A run that starts at or below `cur`. -/
def startsBy : Option Int → Int → Bool
  | some a, cur => decide (a ≤ cur)
  | none, _ => true

/-- Whether the runs, taken in turn, hold every whole number from `cur` to `j`. -/
def coversFrom (j : Int) : Int → List (Option Int × Option Int) → Bool
  | cur, [] => decide (j < cur)
  | cur, r :: rest =>
    if startsBy r.1 cur then
      match r.2 with
      | none => true
      | some b => coversFrom j (max cur (b + 1)) rest
    else coversFrom j cur rest

theorem coversFrom_sound {j : Int} : ∀ (cur : Int) (rs : List (Option Int × Option Int)),
    coversFrom j cur rs = true → ∀ n, cur ≤ n → n ≤ j → ∃ r ∈ rs, inRun r n
  | cur, [], h, n, h1, h2 => by
    unfold coversFrom at h
    simp only [decide_eq_true_eq] at h
    omega
  | cur, (ra, rb) :: rest, h, n, h1, h2 => by
    unfold coversFrom at h
    by_cases hs : startsBy ra cur = true
    · simp only [hs, ↓reduceIte] at h
      have hsn : ∀ a, ra = some a → a ≤ n := by
        intro a ha
        subst ha
        simp only [startsBy, decide_eq_true_eq] at hs
        omega
      cases rb with
      | none => exact ⟨(ra, none), by simp, hsn, fun b hb => by cases hb⟩
      | some b =>
        simp only at h
        by_cases hnb : n ≤ b
        · exact ⟨(ra, some b), by simp, hsn, fun b' hb' => by cases hb'; exact hnb⟩
        · obtain ⟨r', hr', hin⟩ := coversFrom_sound (max cur (b + 1)) rest h n (by omega) h2
          exact ⟨r', List.mem_cons_of_mem _ hr', hin⟩
    · simp only [hs, Bool.false_eq_true, ↓reduceIte] at h
      obtain ⟨r', hr', hin⟩ := coversFrom_sound cur rest h n h1 h2
      exact ⟨r', List.mem_cons_of_mem _ hr', hin⟩

/-- The coordinates of a table's axis a cell's value may stand on: those it is not apart from. -/
def nearOf (cs : List WCoord) (cv : CVal) : List Nat :=
  (List.range cs.length).filter (fun c =>
    match cs[c]?, cv with
    | some tc, .val v => tc.holdsB v
    | some (.word w), .coord (.word u) => u == w
    | some (.num x), .coord (.num y) => !coordApart y x
    | some (.num x), .run k i j => meetsB (tRun k x) i j
    | _, _ => false)

/-- Every value of the cell stands on some coordinate of the axis: one value, held by one; a word, a
    coordinate of that word; an interval, held whole by one coordinate; a run of steps, held step by
    step by the coordinates it meets. -/
def coveredB (cs : List WCoord) (cv : CVal) : Bool :=
  match cv with
  | .val v => cs.any (fun tc => tc.holdsB v)
  | .coord (.word u) => cs.any (fun tc => match tc with | .word w => u == w | .num _ => false)
  | .coord (.num y) => cs.any (fun tc => match tc with | .num x => coordInside y x | .word _ => false)
  | .run k i j => decide (0 < k) && coversFrom j i ((nearOf cs (.run k i j)).filterMap (fun (c : Nat) =>
      match (cs[c]? : Option WCoord) with
      | some (WCoord.num x) => some (tRun k x)
      | _ => none))

theorem firstHolds_some {v : Val} : ∀ {cs : List WCoord} {c : WCoord}, c ∈ cs → c.holds v →
    ∃ i, firstHolds cs v = some i ∧ ∃ tc, cs[i]? = some tc ∧ tc.holds v
  | [], _, hc, _ => by simp at hc
  | d :: cs, c, hc, hv => by
    cases hd : d.holdsB v with
    | true => exact ⟨0, by simp [firstHolds, hd], d, rfl, WCoord.holdsB_iff.1 hd⟩
    | false =>
      rcases List.mem_cons.1 hc with rfl | hc'
      · exact absurd (WCoord.holdsB_iff.2 hv) (by rw [hd]; simp)
      · obtain ⟨i, hi, tc, htc, htcv⟩ := firstHolds_some hc' hv
        refine ⟨i + 1, by simp [firstHolds, hd, hi], tc, by simpa using htc, htcv⟩

/-- **A value of a cell stands on one of the coordinates `nearOf` keeps**: the first coordinate that
    holds it is among them, and where `coveredB` passes there is one. -/
theorem nearOf_sound {cs : List WCoord} {cv : CVal} (hcov : coveredB cs cv = true) {v : Val} (hv : cv.holds v) :
    ∃ i, firstHolds cs v = some i ∧ i ∈ nearOf cs cv := by
  -- some coordinate holds the value
  have hsome : ∃ c ∈ cs, c.holds v := by
    cases cv with
    | val u =>
      have : v = u := hv
      subst this
      simp only [coveredB, List.any_eq_true] at hcov
      obtain ⟨c, hc, h⟩ := hcov
      exact ⟨c, hc, WCoord.holdsB_iff.1 h⟩
    | coord c0 =>
      cases c0 with
      | word u =>
        cases v with
        | num _ => exact absurd hv (by simp [CVal.holds, WCoord.holds])
        | word w =>
          have : w = u := hv
          subst this
          simp only [coveredB, List.any_eq_true] at hcov
          obtain ⟨c, hc, h⟩ := hcov
          cases c with
          | word w' => exact ⟨.word w', hc, (beq_iff_eq.1 h)⟩
          | num _ => exact absurd h (by simp)
      | num y =>
        cases v with
        | word _ => exact absurd hv (by simp [CVal.holds, WCoord.holds])
        | num u =>
          simp only [coveredB, List.any_eq_true] at hcov
          obtain ⟨c, hc, h⟩ := hcov
          cases c with
          | num x => exact ⟨.num x, hc, coordInside_sound h hv⟩
          | word _ => exact absurd h (by simp)
    | run k i j =>
      obtain ⟨n, hi, hj, rfl⟩ := hv
      simp only [coveredB, Bool.and_eq_true, decide_eq_true_eq] at hcov
      obtain ⟨hk, hcov⟩ := hcov
      obtain ⟨r, hr, hin⟩ := coversFrom_sound i _ hcov n hi hj
      simp only [List.mem_filterMap] at hr
      obtain ⟨c, _, hc⟩ := hr
      split at hc
      · rename_i x hx
        simp only [Option.some.injEq] at hc
        subst hc
        exact ⟨.num x, List.mem_of_getElem? hx, (holds_iff_inRun hk x n).2 hin⟩
      · exact absurd hc (by simp)
  obtain ⟨c, hc, hcv⟩ := hsome
  obtain ⟨i, hi, tc, htc, htcv⟩ := firstHolds_some hc hcv
  refine ⟨i, hi, ?_⟩
  have hlen : i < cs.length := (List.getElem?_eq_some_iff.1 htc).1
  simp only [nearOf, List.mem_filter, List.mem_range, hlen, true_and, htc]
  cases cv with
  | val u =>
    have : v = u := hv
    subst this
    exact WCoord.holdsB_iff.2 htcv
  | coord c0 =>
    cases c0 with
    | word u =>
      cases tc with
      | word w =>
        cases v with
        | word w' =>
          have h1 : w' = w := htcv
          have h2 : w' = u := hv
          rw [← h1, ← h2]
          simp
        | num _ => exact absurd hv (by simp [CVal.holds, WCoord.holds])
      | num x =>
        cases v with
        | word _ => exact absurd htcv (by simp [WCoord.holds])
        | num _ => exact absurd hv (by simp [CVal.holds, WCoord.holds])
    | num y =>
      cases tc with
      | num x =>
        cases v with
        | num u =>
          cases hap : coordApart y x with
          | false => simp [hap]
          | true => exact absurd htcv (coordApart_sound hap hv)
        | word _ => exact absurd htcv (by simp [WCoord.holds])
      | word w =>
        cases v with
        | num _ => exact absurd htcv (by simp [WCoord.holds])
        | word _ => exact absurd hv (by simp [CVal.holds, WCoord.holds])
  | run k a b =>
    obtain ⟨n, ha, hb, rfl⟩ := hv
    cases tc with
    | word _ => exact absurd htcv (by simp [WCoord.holds])
    | num x =>
      have hk : 0 < k := by
        simp only [coveredB, Bool.and_eq_true, decide_eq_true_eq] at hcov
        exact hcov.1
      have hin := (holds_iff_inRun hk x n).1 htcv
      simp only [meetsB, Bool.and_eq_true]
      refine ⟨?_, ?_⟩
      · cases h1 : (tRun k x).1 with
        | none => rfl
        | some a' => simp only [decide_eq_true_eq]; have := hin.1 a' h1; omega
      · cases h2 : (tRun k x).2 with
        | none => rfl
        | some b' => simp only [decide_eq_true_eq]; have := hin.2 b' h2; omega

/-- The one value a cell holds, where it holds only one: a value, a word, a single point. -/
def CVal.single : CVal → Option Val
  | .val v => some v
  | .coord (.word w) => some (.word w)
  | .coord (.num (.exactly a)) => some (.num a)
  | .run k i j => if i = j && 0 < k then some (.num ((i : Rat) / (k : Rat))) else none
  | _ => none

theorem CVal.single_sound {cv : CVal} {u : Val} (h : cv.single = some u) {v : Val} (hv : cv.holds v) : v = u := by
  cases cv with
  | val w =>
    simp only [CVal.single, Option.some.injEq] at h
    subst h
    exact hv
  | coord c0 =>
    cases c0 with
    | word w =>
      simp only [CVal.single, Option.some.injEq] at h
      subst h
      cases v with
      | word w' =>
        have : w' = w := hv
        rw [this]
      | num _ => exact absurd hv (by simp [CVal.holds, WCoord.holds])
    | num x =>
      cases x with
      | exactly a =>
        simp only [CVal.single, Option.some.injEq] at h
        subst h
        cases v with
        | num u' =>
          have : u' = a := hv
          rw [this]
        | word _ => exact absurd hv (by simp [CVal.holds, WCoord.holds])
      | between _ _ => simp [CVal.single] at h
  | run k i j =>
    simp only [CVal.single] at h
    split at h
    · rename_i hij
      simp only [Bool.and_eq_true, decide_eq_true_eq] at hij
      simp only [Option.some.injEq] at h
      subst h
      obtain ⟨n, h1, h2, rfl⟩ := hv
      have : n = i := by omega
      rw [this]
    · exact absurd h (by simp)

/-- What a row writes into a column, at a cell: a value, or the one value the cell holds for a name. -/
def ATable.outAt (T : ATable) (cell : String → Option CVal) (r : Row) (col : String) : Option Val :=
  match T.outOf r col with
  | .val v => some v
  | .name n => cell n >>= CVal.single
  | .none => none

/-- The coordinates of each axis of a table a cell may stand on, where every value of the cell stands on
    one of them. -/
def nearFor (cell : String → Option CVal) (cc : String × List WCoord) : Option (List Nat) :=
  match cell cc.1 with
  | some cv => if coveredB cc.2 cv then some (nearOf cc.2 cv) else none
  | none => none

/-- What a table answers at one point a cell may stand on, in a column it decides. -/
def ATable.outCell (T : ATable) (cell : String → Option CVal) (col : String) (p : Point) : Option Val :=
  (T.rowAt p).bind (fun r => T.outAt cell r col)

/-- One value, where a list holds one value only. -/
def allSame : List Val → Option Val
  | v :: rest => if rest.all (· == v) then some v else none
  | [] => none

theorem allSame_mem {vs : List Val} {v : Val} (h : allSame vs = some v) : ∀ y ∈ vs, y = v := by
  match vs, h with
  | w :: rest, h =>
    simp only [allSame] at h
    split at h
    · rename_i hall
      simp only [Option.some.injEq] at h
      subst h
      intro y hy
      rcases List.mem_cons.1 hy with rfl | hy'
      · rfl
      · exact beq_iff_eq.1 (List.all_eq_true.1 hall y hy')
    · exact absurd h (by simp)

/-- **What a table answers at a cell**, in a column it decides: at every point the cell may stand on,
    the row that answers writes one and the same value. -/
def ATable.answerCell (T : ATable) (cell : String → Option CVal) (col : String) : Option Val := do
  let nears ← (T.columns.zip T.coords).mapM (nearFor cell)
  let vals ← (cart nears).mapM (T.outCell cell col)
  allSame vals

/-- **What a cell holds**: a column a table decides is the one value that table answers at the cell;
    any other column, what the cell holds for it. -/
def cellOf (Ts : List ATable) (cell : String → Option CVal) : Nat → String → Option CVal
  | 0, n => if Ts.any (fun T => T.decides.contains n) then none else cell n
  | fuel + 1, n =>
    match Ts.find? (fun T => T.decides.contains n) with
    | some T => (T.answerCell (cellOf Ts cell fuel) n).map CVal.val
    | none => cell n

theorem mapM_of_pointwise {α β : Type} {f g : α → Option β} (hfg : ∀ x b, f x = some b → g x = some b) :
    ∀ (l : List α) {p : List β}, l.mapM f = some p → l.mapM g = some p
  | [], p, h => by simpa using h
  | x :: l, p, h => by
    simp only [List.mapM_cons] at h ⊢
    cases hx : f x with
    | none => simp [hx] at h
    | some b =>
      cases hl : l.mapM f with
      | none => simp [hx, hl] at h
      | some bs =>
        simp only [hx, hl, Option.bind_eq_bind, Option.bind_some, Option.pure_def, Option.some.injEq] at h
        subst h
        simp [hfg x b hx, mapM_of_pointwise hfg l hl]

/-- The point a call's values stand on is one of the points the cell may stand on. -/
theorem point_mem_nears {cell : String → Option CVal} {env : String → Option Val}
    (hagree : ∀ n cv, cell n = some cv → ∃ v, env n = some v ∧ cv.holds v) :
    ∀ (cols : List (String × List WCoord)) {nears : List (List Nat)}, cols.mapM (nearFor cell) = some nears →
      ∃ p, cols.mapM (ptEnv env) = some p ∧ p ∈ cart nears
  | [], nears, h => by
    simp only [List.mapM_nil, Option.pure_def, Option.some.injEq] at h
    subst h
    exact ⟨[], rfl, by simp [cart]⟩
  | cc :: rest, nears, h => by
    simp only [List.mapM_cons] at h
    cases hn : nearFor cell cc with
    | none => simp [hn] at h
    | some ns =>
      cases hr : rest.mapM (nearFor cell) with
      | none => simp [hn, hr] at h
      | some nss =>
        simp only [hn, hr, Option.bind_eq_bind, Option.bind_some, Option.pure_def, Option.some.injEq] at h
        subst h
        simp only [nearFor] at hn
        cases hc : cell cc.1 with
        | none => simp [hc] at hn
        | some cv =>
          simp only [hc] at hn
          split at hn
          · rename_i hcov
            simp only [Option.some.injEq] at hn
            subst hn
            obtain ⟨v, hv, hcv⟩ := hagree _ _ hc
            obtain ⟨i, hi, hin⟩ := nearOf_sound hcov hcv
            obtain ⟨p', hp', hmem⟩ := point_mem_nears hagree rest hr
            refine ⟨i :: p', ?_, ?_⟩
            · simp [List.mapM_cons, ptEnv, hv, hi, hp']
            · simp only [cart, List.mem_flatMap, List.mem_map]
              exact ⟨i, hin, p', hmem, rfl⟩
          · exact absurd hn (by simp)

theorem mem_of_mapM {α β : Type} {f : α → Option β} :
    ∀ {l : List α} {vs : List β}, l.mapM f = some vs → ∀ x ∈ l, ∃ y ∈ vs, f x = some y
  | [], vs, _, x, hx => by simp at hx
  | a :: l, vs, h, x, hx => by
    simp only [List.mapM_cons] at h
    cases ha : f a with
    | none => simp [ha] at h
    | some b =>
      cases hl : l.mapM f with
      | none => simp [ha, hl] at h
      | some bs =>
        simp only [ha, hl, Option.bind_eq_bind, Option.bind_some, Option.pure_def, Option.some.injEq] at h
        subst h
        rcases List.mem_cons.1 hx with rfl | hx'
        · exact ⟨b, by simp, ha⟩
        · obtain ⟨y, hy, hfy⟩ := mem_of_mapM hl x hx'
          exact ⟨y, List.mem_cons_of_mem _ hy, hfy⟩

/-- **What a cell holds is what the call holds**, at every call whose values stand inside the cell:
    each column a table decides is, at the call, the one value the tables answer at the cell, and every
    other column the cell holds is one of the values it holds there. -/
theorem cellOf_sound (Ts : List ATable) {cell : String → Option CVal} {env : String → Option Val}
    (hagree : ∀ n cv, cell n = some cv → ∃ v, env n = some v ∧ cv.holds v) :
    ∀ (fuel : Nat) (n : String) (cv : CVal), cellOf Ts cell fuel n = some cv →
      ∃ v, valueOf Ts env fuel n = some v ∧ cv.holds v
  | 0, n, cv, h => by
    simp only [cellOf] at h
    split at h
    · exact absurd h (by simp)
    · rename_i hnot
      obtain ⟨v, hv, hcv⟩ := hagree n cv h
      refine ⟨v, ?_, hcv⟩
      simp only [valueOf]
      split
      · rename_i hyes
        exact absurd hyes hnot
      · exact hv
  | fuel + 1, n, cv, h => by
    have ih := cellOf_sound Ts hagree fuel
    simp only [cellOf] at h
    simp only [valueOf]
    split at h
    · rename_i T hT
      simp only [Option.map_eq_some_iff] at h
      obtain ⟨u, hu, rfl⟩ := h
      refine ⟨u, ?_, rfl⟩
      simp only [ATable.answerCell, Option.bind_eq_bind] at hu
      cases hn : (T.columns.zip T.coords).mapM (nearFor (cellOf Ts cell fuel)) with
      | none => simp [hn] at hu
      | some nears =>
        rw [hn] at hu
        simp only [Option.bind_some] at hu
        cases hvals : (cart nears).mapM (T.outCell (cellOf Ts cell fuel) n) with
        | none => simp [hvals] at hu
        | some vals =>
          rw [hvals] at hu
          simp only [Option.bind_some] at hu
          obtain ⟨p, hp, hpm⟩ := point_mem_nears ih (T.columns.zip T.coords) hn
          obtain ⟨y, hy, hpy⟩ := mem_of_mapM hvals p hpm
          have hyv : y = u := allSame_mem hu y hy
          subst hyv
          simp only [ATable.answer, Option.bind_eq_bind, hp, Option.bind_some]
          simp only [ATable.outCell] at hpy
          cases hr : T.rowAt p with
          | none => simp [hr] at hpy
          | some r =>
            simp only [hr, Option.bind_some] at hpy ⊢
            simp only [ATable.outAt] at hpy
            cases ho : T.outOf r n with
            | val w => simp only [ho, Option.some.injEq] at hpy ⊢; exact hpy
            | none => simp [ho] at hpy
            | name m =>
              simp only [ho] at hpy ⊢
              cases hm : cellOf Ts cell fuel m with
              | none => simp [hm] at hpy
              | some cvm =>
                rw [hm] at hpy
                have hs : cvm.single = some y := by simpa using hpy
                obtain ⟨w, hw, hcw⟩ := ih m cvm hm
                rw [hw, CVal.single_sound hs hcw]
    · exact hagree n cv h

/-! ## The cells of a claim -/

/-- A cell of a claim, as the certificate writes it: anything, the absent value, the words or numbers it
    takes or leaves out, comparisons with numbers, prefixes of strings. -/
inductive CCell where
  | any
  | nothing
  | words (ws : List String)
  | notWords (ws : List String)
  | nums (vs : List Rat)
  | notNums (vs : List Rat)
  | cmp (ts : List (Cmp × Rat))
  | prefixes (ps : List String)
  deriving Repr, Inhabited

/-- **Whether a value passes a cell of a claim**; `none` is no value at all. -/
def CCell.passes : CCell → Option Val → Bool
  | .any, _ => true
  | .nothing, v => v == none || v == some (.word "none")
  | .words ws, some (.word w) => ws.contains w
  | .notWords ws, some (.word w) => !ws.contains w
  | .nums vs, some (.num x) => vs.contains x
  | .notNums vs, some (.num x) => !vs.contains x
  | .cmp ts, some (.num x) => ts.all (fun t => cmpHolds t.1 x t.2)
  | .prefixes ps, some (.word w) => ps.any (fun p => p.isPrefixOf w)
  | _, _ => false

/-- Whether every value a coordinate of the walk holds passes the cell, read on the coordinate. -/
def CCell.insideB : CCell → WCoord → Bool
  | .any, _ => true
  | .nothing, .word w => w == "none"
  | .words ws, .word w => ws.contains w
  | .notWords ws, .word w => !ws.contains w
  | .nums vs, .num x => vs.any (fun w => admitsCmp x .eq w)
  | .notNums vs, .num x => !vs.any (fun w => admitsCmp x .eq w)
  | .cmp ts, .num x => ts.all (fun t => admitsCmp x t.1 t.2)
  | _, _ => false

/-- No value the cell compares against falls strictly inside the coordinate, so the cell holds of all
    of it or of none of it (§6.2's construction, checked). A cell of prefixes is not read this way. -/
def CCell.splitsB : CCell → WCoord → Bool
  | .nums vs, .num x => vs.all (splitsAt x)
  | .notNums vs, .num x => vs.all (splitsAt x)
  | .cmp ts, .num x => ts.all (fun t => splitsAt x t.2)
  | .prefixes _, _ => false
  | _, _ => true

theorem cmpHolds_iff {op : Cmp} {x y : Rat} : cmpHolds op x y = true ↔ op.holds x y := by
  cases op <;> simp [cmpHolds, Cmp.holds]

theorem word_beq (u w : String) : (Val.word u == Val.word w) = (u == w) := by
  by_cases h : u = w
  · subst h; simp
  · have h1 : (u == w) = false := by simp [h]
    have h2 : (Val.word u == Val.word w) = false := by
      rw [beq_eq_false_iff_ne]
      intro he
      exact h (Val.word.inj he)
    rw [h1, h2]

/-- **A cell read on a coordinate says what it says of every value there.** -/
theorem CCell.inside_iff {c : CCell} {x : WCoord} (hs : c.splitsB x = true) {v : Val} (hv : x.holds v) :
    c.insideB x = c.passes (some v) := by
  match c, x, v, hs, hv with
  | .any, _, _, _, _ => rfl
  | .nothing, .word w, .word u, _, hv =>
    have : u = w := hv
    subst this
    simp only [CCell.insideB, CCell.passes]
    by_cases h : u = "none"
    · subst h
      simp
    · have h1 : (u == "none") = false := by simp [h]
      have h2 : (some (Val.word u) == some (Val.word "none")) = false := by
        rw [beq_eq_false_iff_ne]
        intro he
        exact h (Val.word.inj (Option.some.inj he))
      have h3 : (some (Val.word u) == (none : Option Val)) = false := by
        rw [beq_eq_false_iff_ne]
        intro he
        cases he
      rw [h1, h2, h3]
      rfl
  | .nothing, .num _, .num _, _, _ => simp [CCell.insideB, CCell.passes]
  | .words ws, .word w, .word u, _, hv =>
    have : u = w := hv
    subst this
    rfl
  | .words _, .num _, .num _, _, _ => rfl
  | .notWords ws, .word w, .word u, _, hv =>
    have : u = w := hv
    subst this
    rfl
  | .notWords _, .num _, .num _, _, _ => rfl
  | .nums vs, .num x, .num u, hs, hv =>
    simp only [CCell.splitsB, List.all_eq_true] at hs
    simp only [CCell.insideB, CCell.passes]
    apply Bool.eq_iff_iff.2
    simp only [List.any_eq_true, List.contains_iff_mem]
    constructor
    · rintro ⟨w, hw, hadm⟩
      have := (admits_iff (hs w hw) hv).1 hadm
      have hu : u = w := this
      exact hu ▸ hw
    · intro hu
      exact ⟨u, hu, (admits_iff (hs u hu) hv).2 rfl⟩
  | .nums _, .word _, .word _, _, _ => rfl
  | .notNums vs, .num x, .num u, hs, hv =>
    simp only [CCell.splitsB, List.all_eq_true] at hs
    simp only [CCell.insideB, CCell.passes]
    congr 1
    apply Bool.eq_iff_iff.2
    simp only [List.any_eq_true, List.contains_iff_mem]
    constructor
    · rintro ⟨w, hw, hadm⟩
      have := (admits_iff (hs w hw) hv).1 hadm
      have hu : u = w := this
      exact hu ▸ hw
    · intro hu
      exact ⟨u, hu, (admits_iff (hs u hu) hv).2 rfl⟩
  | .notNums _, .word _, .word _, _, _ => rfl
  | .cmp ts, .num x, .num u, hs, hv =>
    simp only [CCell.splitsB, List.all_eq_true] at hs
    simp only [CCell.insideB, CCell.passes]
    apply Bool.eq_iff_iff.2
    simp only [List.all_eq_true]
    constructor
    · intro h t ht
      exact cmpHolds_iff.2 ((admits_iff (hs t ht) hv).1 (h t ht))
    · intro h t ht
      exact (admits_iff (hs t ht) hv).2 (cmpHolds_iff.1 (h t ht))
  | .cmp _, .word _, .word _, _, _ => rfl
  | .prefixes _, _, _, hs, _ => simp [CCell.splitsB] at hs

/-! ## A claim, and what it means -/

/-- The chance a claim writes: a value, or comparisons with values. -/
inductive ChanceTest where
  | is : Rat → ChanceTest
  | cmp : List (Cmp × Rat) → ChanceTest
  deriving Repr, Inhabited

def ChanceTest.holds : ChanceTest → Rat → Bool
  | .is q, p => p == q
  | .cmp ts, p => ts.all (fun t => cmpHolds t.1 p t.2)

/-- What a column that is not random takes: a number on the grid `st` inside its range — and the absent
    value, where it is optional — or one of its words. -/
inductive FAx where
  | num (st : Rat) (lo hi : Option Rat) (optional : Bool)
  | words (ws : List String)
  deriving Repr, Inhabited

def FAx.takes : FAx → Val → Prop
  | .num st lo hi _, .num v => onGrid st v ∧ (∀ l, lo = some l → l ≤ v) ∧ (∀ u, hi = some u → v ≤ u)
  | .num _ _ _ opt, .word w => opt = true ∧ w = "none"
  | .words ws, .word w => w ∈ ws
  | .words _, .num _ => False

/-- The numbers' intervals of an axis, in order, where every coordinate is one. -/
def numsOf : List WCoord → Option (List Coord)
  | [] => some []
  | .num x :: cs => (numsOf cs).map (x :: ·)
  | .word _ :: _ => none

/-- The words of an axis, in order, where every coordinate is one. -/
def wordsOf : List WCoord → Option (List String)
  | [] => some []
  | .word w :: cs => (wordsOf cs).map (w :: ·)
  | .num _ :: _ => none

/-- The coordinates after the absent value, where the axis starts with it. -/
def dropNone : List WCoord → Option (List WCoord)
  | .word w :: rest => if w = "none" then some rest else none
  | _ => none

theorem dropNone_eq : ∀ {cs rest : List WCoord}, dropNone cs = some rest → cs = .word "none" :: rest
  | .word w :: rest', rest, h => by
    simp only [dropNone] at h
    split at h
    · rename_i hw
      simp only [Option.some.injEq] at h
      subst h; subst hw
      rfl
    · exact absurd h (by simp)
  | [], _, h => by simp [dropNone] at h
  | .num _ :: _, _, h => by simp [dropNone] at h

/-- **The tiling check of an axis that is not random**: a number's coordinates tile its range on its
    grid (`axisTilesB`), after the absent value where it is optional; a word's coordinates are its words. -/
def FAx.tilesB : FAx → List WCoord → Bool
  | .num st lo hi opt, cs =>
    match (if opt then dropNone cs else some cs) with
    | some rest => match numsOf rest with
      | some xs => axisTilesB st lo hi xs
      | none => false
    | none => false
  | .words ws, cs => wordsOf cs == some ws

theorem numsOf_get : ∀ {cs : List WCoord} {xs : List Coord}, numsOf cs = some xs →
    ∀ {c : Nat} {x : Coord}, xs[c]? = some x → cs[c]? = some (.num x)
  | [], xs, h, c, x, hc => by
    simp only [numsOf, Option.some.injEq] at h
    subst h
    simp at hc
  | .num y :: cs, xs, h, c, x, hc => by
    simp only [numsOf, Option.map_eq_some_iff] at h
    obtain ⟨ys, hys, rfl⟩ := h
    cases c with
    | zero => simp at hc; subst hc; rfl
    | succ c => exact numsOf_get hys (by simpa using hc)
  | .word _ :: _, xs, h, _, _, _ => by simp [numsOf] at h

theorem wordsOf_mem : ∀ {cs : List WCoord} {ws : List String}, wordsOf cs = some ws →
    ∀ {w : String}, w ∈ ws → ∃ c : Nat, cs[c]? = some (WCoord.word w)
  | [], ws, h, w, hw => by
    simp only [wordsOf, Option.some.injEq] at h
    subst h
    simp at hw
  | .word u :: cs, ws, h, w, hw => by
    simp only [wordsOf, Option.map_eq_some_iff] at h
    obtain ⟨us, hus, rfl⟩ := h
    rcases List.mem_cons.1 hw with rfl | hw'
    · exact ⟨0, rfl⟩
    · obtain ⟨c, hc⟩ := wordsOf_mem hus hw'
      exact ⟨c + 1, by simpa using hc⟩
  | .num _ :: _, ws, h, _, _ => by simp [wordsOf] at h

/-- **Every value a column that is not random takes stands on a coordinate of its axis.** -/
theorem FAx.cover {ax : FAx} {cs : List WCoord} (h : ax.tilesB cs = true) {v : Val} (hv : ax.takes v) :
    ∃ (c : Nat) (x : WCoord), cs[c]? = some x ∧ x.holds v := by
  match ax, v, hv with
  | .num st lo hi opt, .num u, hv =>
    obtain ⟨hg, hl, hh⟩ := hv
    simp only [FAx.tilesB] at h
    split at h
    · rename_i rest hrest
      split at h
      · rename_i xs hxs
        obtain ⟨c, x, hc, hx⟩ := axisTilesB_cover h hg hl hh
        have hrc := numsOf_get hxs hc
        cases opt with
        | false =>
          simp only [Bool.false_eq_true, ↓reduceIte, Option.some.injEq] at hrest
          subst hrest
          exact ⟨c, .num x, hrc, hx⟩
        | true =>
          simp only [↓reduceIte] at hrest
          have := dropNone_eq hrest
          subst this
          exact ⟨c + 1, .num x, by simpa using hrc, hx⟩
      · exact absurd h (by simp)
    · exact absurd h (by simp)
  | .num st lo hi opt, .word w, hv =>
    obtain ⟨rfl, rfl⟩ := hv
    simp only [FAx.tilesB, ↓reduceIte] at h
    split at h
    · rename_i rest hrest
      have := dropNone_eq hrest
      subst this
      exact ⟨0, .word "none", rfl, rfl⟩
    · exact absurd h (by simp)
  | .words ws, .word w, hv =>
    simp only [FAx.tilesB, beq_iff_eq] at h
    obtain ⟨c, hc⟩ := wordsOf_mem h hv
    exact ⟨c, .word w, hc, rfl⟩

/-- What a coordinate of a random axis holds at a cell: a number's whole steps from the `i`-th to the
    `j`-th, or a word. -/
def RC.cell : RC → CVal
  | .num _ k i j => .run k i j
  | .word w _ => .coord (.word w)

theorem RC.cell_holds {c : RC} {v : Val} (hv : v ∈ c.pts) : c.cell.holds v := by
  cases c with
  | num x k i j =>
    simp only [RC.pts, steps, List.mem_map] at hv
    obtain ⟨n, hn, rfl⟩ := hv
    obtain ⟨h1, h2⟩ := mem_irange.1 hn
    exact ⟨n, h1, h2, rfl⟩
  | word w d =>
    cases d with
    | true =>
      simp only [RC.pts, ↓reduceIte, List.mem_singleton] at hv
      subst hv
      rfl
    | false => simp [RC.pts] at hv

/-- The `k` of a grid `1/k`, where the grid is one. -/
def gridK (st : Rat) : Option Nat :=
  let k := (st⁻¹).num.toNat
  if 0 < k ∧ st * (k : Rat) = 1 then some k else none

/-- What a coordinate of an axis that is not random holds at a cell: a number's whole steps of the grid
    its column takes, where both its ends are there, and otherwise every value of the coordinate. -/
def fixedCell (ax : FAx) (x : WCoord) : CVal :=
  match ax, x with
  | .num st _ _ _, .num c =>
    match gridK st with
    | some k =>
      match tRun k c with
      | (some a, some b) => .run k a b
      | _ => .coord (.num c)
    | none => .coord (.num c)
  | _, w => .coord w

theorem fixedCell_holds {ax : FAx} {x : WCoord} {v : Val} (hx : x.holds v) (ht : ax.takes v) :
    (fixedCell ax x).holds v := by
  match ax, x, v, hx, ht with
  | .num st lo hi opt, .num c, .num u, hx, ht =>
    simp only [fixedCell]
    cases hk : gridK st with
    | none => exact hx
    | some k =>
      simp only [gridK] at hk
      split at hk
      · rename_i hcond
        simp only [Option.some.injEq] at hk
        subst hk
        obtain ⟨hk0, hst⟩ := hcond
        obtain ⟨m, hm⟩ := ht.1
        -- the value is a whole number of steps of `1/k`
        have hu : u = (m : Rat) / ((st⁻¹).num.toNat : Rat) := by
          rw [hm, Rat.div_def]
          congr 1
          have hne : ((st⁻¹).num.toNat : Rat) ≠ 0 := by
            have : (0 : Rat) < ((st⁻¹).num.toNat : Rat) := by exact_mod_cast hk0
            exact Rat.ne_of_gt this
          have : st = ((st⁻¹).num.toNat : Rat)⁻¹ := by
            have h2 := congrArg (fun q => q * ((st⁻¹).num.toNat : Rat)⁻¹) hst
            rw [Rat.mul_assoc, Rat.mul_inv_cancel _ hne, Rat.mul_one, Rat.one_mul] at h2
            exact h2
          exact this
        have hin := (holds_iff_inRun hk0 c m).1 (by rw [← hu]; exact hx)
        cases htr : tRun ((st⁻¹).num.toNat) c with
        | mk a b =>
          cases a with
          | none => simp only [htr]; exact hx
          | some a =>
            cases b with
            | none => simp only [htr]; exact hx
            | some b =>
              simp only [htr]
              rw [htr] at hin
              exact ⟨m, hin.1 a rfl, hin.2 b rfl, by rw [hu]⟩
      · exact absurd hk (by simp)
  | .num _ _ _ _, .word w, v, hx, _ => exact hx
  | .words _, x, v, hx, _ => exact hx
  | .num _ _ _ _, .num _, .word _, hx, _ => exact absurd hx (by simp [WCoord.holds])

/-- **A claim of `chances`**, as the program reads it off the certificate: the tables and how deep the
    answers are read; the columns of the walk that are not random, what each takes and its coordinates;
    the random inputs, what each is drawn from and its coordinates; the claim's cells on the inputs that
    are not random and on the outputs it is about; the grid each numeric output rounds to; and the
    chance it writes. -/
structure Claim where
  tables : List ATable
  fuel : Nat
  fixedCols : List String
  fixedTakes : List FAx
  fixed : List (List WCoord)
  randCols : List String
  draws : List RIn
  rand : List (List RC)
  ins : List (String × CCell)
  outs : List (String × CCell)
  rounds : String → Option Rat
  test : ChanceTest

namespace Claim

/-- The values of a call at a draw: each random input the value drawn for it, every other name the one
    `env` gives it. -/
def extend (K : Claim) (env : String → Option Val) (r : List Val) : String → Option Val := fun n =>
  match K.randCols.idxOf? n with
  | some j => r[j]?
  | none => env n

/-- An output's value as the rule returns it: a number rounded to the grid its output rounds to. -/
def roundVal (rnd : Rat → Rat → Rat) (g : Option Rat) : Val → Val
  | .num v => match g with
    | some g => .num (rnd g v)
    | none => .num v
  | .word w => .word w

/-- Whether the outputs of a call pass the claim's output cells. -/
def passesAt (K : Claim) (rnd : Rat → Rat → Rat) (vals : String → Option Val) : Bool :=
  K.outs.all (fun oc => oc.2.passes ((vals oc.1).map (roundVal rnd (K.rounds oc.1))))

/-- **The share a claim is about**, at the values `env` gives the columns that are not random: of every
    way of drawing the random inputs — one value of each, from what it is drawn from — how many give
    outputs that pass the claim's output cells, and how many ways there are. -/
def share (K : Claim) (rnd : Rat → Rat → Rat) (env : String → Option Val) : Nat × Nat :=
  ((cart (K.draws.map RIn.grid)).countP (fun r => K.passesAt rnd (valueOf K.tables (K.extend env r) K.fuel)),
   (cart (K.draws.map RIn.grid)).length)

/-- **What a claim of `chances` says**: wherever the columns that are not random take values they can
    take and those values pass the claim's input cells, the share of the draws whose outputs pass the
    claim's output cells passes the chance it writes — under every rounding the outputs can be made
    with. -/
def Holds (K : Claim) : Prop :=
  ∀ (rnd : Rat → Rat → Rat), RoundsWell rnd → ∀ (env : String → Option Val),
    (∀ (i : Nat) (col : String) (ax : FAx), K.fixedCols[i]? = some col → K.fixedTakes[i]? = some ax →
      ∃ v, env col = some v ∧ ax.takes v) →
    (∀ oc ∈ K.ins, oc.2.passes (env oc.1) = true) →
    0 < (K.share rnd env).2 →
    K.test.holds (((K.share rnd env).1 : Rat) / ((K.share rnd env).2 : Rat)) = true

/-! ### What the program works out, box by box -/

/-- The cell of the walk at a class and a box: each column of an axis at its coordinate. -/
def cellAt (K : Claim) (cls : List WCoord) (box : List RC) : String → Option CVal := fun n =>
  match K.randCols.idxOf? n with
  | some j => (box[j]?).map RC.cell
  | none => match K.fixedCols.idxOf? n with
    | some i => match cls[i]?, K.fixedTakes[i]? with
      | some x, some ax => some (fixedCell ax x)
      | _, _ => none
    | none => none

/-- A value its output's rounding leaves as it is: a number on the grid it rounds to, or a word. -/
def keepsB (g : Option Rat) : Val → Bool
  | .num v => match g with
    | some g => decide (0 < g) && gridB g v
    | none => false
  | .word _ => true

/-- Whether one output of a cell passes its cell of the claim: `none` where the tables do not give it
    one value there that its rounding leaves as it is. -/
def outHit (K : Claim) (cls : List WCoord) (box : List RC) (oc : String × CCell) : Option Bool :=
  match oc.2 with
  | .any => some true
  | c =>
    match cellOf K.tables (K.cellAt cls box) K.fuel oc.1 with
    | some (.val v) => if keepsB (K.rounds oc.1) v then some (c.passes (some v)) else none
    | _ => none

/-- Every one of a list of answers there, and whether all of them are `true`. -/
def allSome : List (Option Bool) → Option Bool
  | [] => some true
  | x :: xs => match x, allSome xs with
    | some a, some b => some (a && b)
    | _, _ => none

/-- Whether the outputs of a cell pass the claim, worked out from the rows. -/
def hitAt (K : Claim) (cls : List WCoord) (box : List RC) : Option Bool := allSome (K.outs.map (K.outHit cls box))

/-- The points of a box and the hits among them: the product of its coordinates' points, and all of
    them or none. -/
def boxCount (K : Claim) (cls : List WCoord) (box : List RC) : Option (Nat × Nat) :=
  if (box.map RC.count).prod = 0 then some (0, 0)
  else (K.hitAt cls box).map (fun hit => ((box.map RC.count).prod, if hit then (box.map RC.count).prod else 0))

/-- Pairs added up, where every one is there. -/
def sumPairs : List (Option (Nat × Nat)) → Option (Nat × Nat)
  | [] => some (0, 0)
  | x :: xs => match x, sumPairs xs with
    | some (a, b), some (c, d) => some (a + c, b + d)
    | _, _ => none

/-- **The points and the hits of a class**, every box of the random axes counted once. -/
def count (K : Claim) (cls : List WCoord) : Option (Nat × Nat) := sumPairs ((cart K.rand).map (K.boxCount cls))

/-- The class passes the claim: its hits over its points pass the chance, where it has points. -/
def classOk (K : Claim) (cls : List WCoord) : Bool :=
  match K.count cls with
  | some (p, h) => p == 0 || K.test.holds ((h : Rat) / (p : Rat))
  | none => false

/-- The class lies inside the claim's input cells. -/
def insideAll (K : Claim) (cls : List WCoord) : Bool :=
  K.ins.all (fun oc => match K.fixedCols.idxOf? oc.1 with
    | some i => match cls[i]? with
      | some x => oc.2.insideB x
      | none => false
    | none => false)

/-- What the claim's reading has to be: the random axes tile what their inputs are drawn from, the
    other axes tile what their columns take, every input cell is read on its own axis and none of its
    values falls inside a coordinate there, and no column is both. -/
def wellB (K : Claim) : Bool :=
  K.randCols.length == K.draws.length && K.rand.length == K.draws.length &&
    (K.draws.zip K.rand).all (fun dc => tilesB dc.1 dc.2) &&
    K.fixedCols.length == K.fixed.length && K.fixedTakes.length == K.fixed.length &&
    (K.fixedTakes.zip K.fixed).all (fun af => af.1.tilesB af.2) &&
    K.ins.all (fun oc => match K.fixedCols.idxOf? oc.1 with
      | some i => match K.fixed[i]? with
        | some cs => cs.all oc.2.splitsB
        | none => false
      | none => false) &&
    K.fixedCols.all (fun n => !K.randCols.contains n)

/-- **The check of a claim**: its reading is well formed, and every class inside its input cells
    passes it. -/
def okB (K : Claim) : Bool := K.wellB && (cart K.fixed).all (fun cls => !K.insideAll cls || K.classOk cls)

end Claim

/-! ## What a pass settles -/

namespace Claim

theorem getElem?_of_idxOf? {l : List String} {n : String} {i : Nat} (h : l.idxOf? n = some i) : l[i]? = some n := by
  obtain ⟨hi, heq, _⟩ := List.idxOf?_eq_some_iff.1 h
  rw [List.getElem?_eq_getElem hi, heq]

theorem sumPairs_spec {β : Type} (f : β → Option (Nat × Nat)) (gP gH : β → Nat) :
    ∀ (l : List β), (∀ b ∈ l, ∀ p q, f b = some (p, q) → gP b = p ∧ gH b = q) →
      ∀ {P H : Nat}, sumPairs (l.map f) = some (P, H) → (l.map gP).sum = P ∧ (l.map gH).sum = H
  | [], _, P, H, h => by
    simp only [List.map_nil, sumPairs, Option.some.injEq, Prod.mk.injEq] at h
    obtain ⟨rfl, rfl⟩ := h
    exact ⟨rfl, rfl⟩
  | b :: l, hg, P, H, h => by
    simp only [List.map_cons, sumPairs] at h
    split at h
    · rename_i a c d e hb hl
      simp only [Option.some.injEq, Prod.mk.injEq] at h
      obtain ⟨rfl, rfl⟩ := h
      obtain ⟨h1, h2⟩ := hg b (by simp) a c hb
      obtain ⟨h3, h4⟩ := sumPairs_spec f gP gH l (fun b' hb' => hg b' (List.mem_cons_of_mem _ hb')) hl
      simp [List.map_cons, List.sum_cons, h1, h2, h3, h4]
    · exact absurd h (by simp)

theorem sumPairs_some {β : Type} (f : β → Option (Nat × Nat)) :
    ∀ (l : List β) {P H : Nat}, sumPairs (l.map f) = some (P, H) → ∀ b ∈ l, ∃ p q, f b = some (p, q)
  | [], _, _, _, b, hb => by simp at hb
  | x :: l, P, H, h, b, hb => by
    simp only [List.map_cons, sumPairs] at h
    split at h
    · rename_i a c d e hx hl
      rcases List.mem_cons.1 hb with rfl | hb'
      · exact ⟨a, c, hx⟩
      · exact sumPairs_some f l hl b hb'
    · exact absurd h (by simp)

theorem allSome_spec {β : Type} (f : β → Option Bool) :
    ∀ (l : List β) {b : Bool}, allSome (l.map f) = some b →
      (∀ x ∈ l, ∃ c, f x = some c) ∧ (b = l.all (fun x => (f x).getD false))
  | [], b, h => by
    simp only [List.map_nil, allSome, Option.some.injEq] at h
    subst h
    simp
  | x :: l, b, h => by
    simp only [List.map_cons, allSome] at h
    split at h
    · rename_i a c hx hl
      simp only [Option.some.injEq] at h
      subst h
      obtain ⟨hall, heq⟩ := allSome_spec f l hl
      refine ⟨fun y hy => ?_, ?_⟩
      · rcases List.mem_cons.1 hy with rfl | hy'
        · exact ⟨a, hx⟩
        · exact hall y hy'
      · simp only [List.all_cons, hx, Option.getD_some, heq]
    · exact absurd h (by simp)

theorem all_congr_mem {β : Type} {f g : β → Bool} : ∀ {l : List β}, (∀ x ∈ l, f x = g x) → l.all f = l.all g
  | [], _ => rfl
  | x :: l, h => by
    simp only [List.all_cons]
    rw [h x (by simp), all_congr_mem (fun y hy => h y (List.mem_cons_of_mem _ hy))]

/-- A value its output's rounding leaves as it is is the value the rule returns. -/
theorem roundVal_keeps {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd) {g : Option Rat} {v : Val}
    (h : keepsB g v = true) : roundVal rnd g v = v := by
  match v, g, h with
  | .num u, some g, h =>
    simp only [keepsB, Bool.and_eq_true, decide_eq_true_eq] at h
    simp only [roundVal]
    rw [rounds_onGrid hrnd h.1 (onGrid_of_gridB h.2)]
  | .word w, _, _ => rfl

/-- **Inside a box, every draw answers alike**: wherever the values of the columns that are not random
    stand inside the class's coordinates, every draw from the box's coordinates gives outputs that pass
    the claim exactly where the box's answer, worked out from the rows, does. -/
theorem box_const (K : Claim) (hw : K.wellB = true) {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd)
    {env : String → Option Val} {cls : List WCoord}
    (hcls : ∀ (i : Nat) (col : String) (x : WCoord), K.fixedCols[i]? = some col → cls[i]? = some x →
      ∃ v, env col = some v ∧ x.holds v)
    (htk : ∀ (i : Nat) (col : String) (ax : FAx), K.fixedCols[i]? = some col → K.fixedTakes[i]? = some ax →
      ∃ v, env col = some v ∧ ax.takes v)
    {box : List RC} (hbox : box ∈ cart K.rand) {hit : Bool} (hhit : K.hitAt cls box = some hit) :
    ∀ r ∈ cart (box.map RC.pts), K.passesAt rnd (valueOf K.tables (K.extend env r) K.fuel) = hit := by
  intro r hr
  simp only [wellB, Bool.and_eq_true, beq_iff_eq] at hw
  obtain ⟨⟨⟨⟨⟨⟨⟨hlr, hlrand⟩, htiles⟩, _⟩, _⟩, _⟩, _⟩, _⟩ := hw
  obtain ⟨hrlen, hrin⟩ := mem_cart hr
  obtain ⟨_, hboxin⟩ := mem_cart hbox
  -- every coordinate of the box is one its interval says
  have hok : ∀ (j : Nat) (c : RC), box[j]? = some c → c.okB = true := by
    intro j c hc
    have hj : j < box.length := (List.getElem?_eq_some_iff.1 hc).1
    have hjr : j < K.rand.length := by
      have := (mem_cart hbox).1
      omega
    have hjd : j < K.draws.length := by omega
    have hcs : K.rand[j]? = some K.rand[j] := List.getElem?_eq_getElem hjr
    have hmem : c ∈ K.rand[j] := hboxin j c _ hc hcs
    have hzip : (K.draws[j], K.rand[j]) ∈ K.draws.zip K.rand := by
      rw [List.mem_iff_getElem?]
      exact ⟨j, by simp [List.getElem?_zip_eq_some, List.getElem?_eq_getElem hjd, List.getElem?_eq_getElem hjr]⟩
    have ht := List.all_eq_true.1 htiles _ hzip
    simp only [tilesB, Bool.and_eq_true] at ht
    exact List.all_eq_true.1 ht.1 c hmem
  have hagree : ∀ n cv, K.cellAt cls box n = some cv → ∃ v, K.extend env r n = some v ∧ cv.holds v := by
    intro n cv hn
    simp only [cellAt] at hn
    simp only [extend]
    split at hn
    · rename_i j hj
      simp only [Option.map_eq_some_iff] at hn
      obtain ⟨c, hc, rfl⟩ := hn
      have hjb : j < box.length := (List.getElem?_eq_some_iff.1 hc).1
      have hjr : j < r.length := by
        simp only [List.length_map] at hrlen
        omega
      refine ⟨r[j], List.getElem?_eq_getElem hjr, ?_⟩
      have hin : r[j] ∈ c.pts := by
        have := hrin j r[j] c.pts (List.getElem?_eq_getElem hjr) (by simp [hc])
        exact this
      exact RC.cell_holds hin
    · rename_i hj
      split at hn
      · rename_i i hi
        split at hn
        · rename_i x ax hx hax
          simp only [Option.some.injEq] at hn
          subst hn
          obtain ⟨v, hv, hxv⟩ := hcls i n x (getElem?_of_idxOf? hi) hx
          obtain ⟨v', hv', htv⟩ := htk i n ax (getElem?_of_idxOf? hi) hax
          rw [hv] at hv'
          simp only [Option.some.injEq] at hv'
          subst hv'
          exact ⟨v, hv, fixedCell_holds hxv htv⟩
        · exact absurd hn (by simp)
      · exact absurd hn (by simp)
  have hsound := cellOf_sound K.tables hagree K.fuel
  simp only [hitAt] at hhit
  obtain ⟨hsome, heq⟩ := allSome_spec (K.outHit cls box) K.outs hhit
  rw [heq]
  simp only [passesAt]
  apply all_congr_mem
  intro oc hoc
  obtain ⟨c, hc⟩ := hsome oc hoc
  rw [hc, Option.getD_some]
  simp only [outHit] at hc
  split at hc
  · rename_i hany
    simp only [Option.some.injEq] at hc
    subst hc
    rw [hany]
    rfl
  · split at hc
    · rename_i v hv
      split at hc
      · rename_i hkeep
        simp only [Option.some.injEq] at hc
        subst hc
        obtain ⟨w, hw, hcw⟩ := hsound oc.1 (.val v) hv
        have : w = v := hcw
        subst this
        rw [hw]
        simp only [Option.map_some, roundVal_keeps hrnd hkeep]
      · exact absurd hc (by simp)
    · exact absurd hc (by simp)

/-- **The points and the hits of a class are the share of the draws**: where the values of the columns
    that are not random stand inside the class's coordinates, the draws whose outputs pass the claim
    are the hits the program counts box by box, and the draws are its points. -/
theorem count_share (K : Claim) (hw : K.wellB = true) {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd)
    {env : String → Option Val} {cls : List WCoord}
    (hcls : ∀ (i : Nat) (col : String) (x : WCoord), K.fixedCols[i]? = some col → cls[i]? = some x →
      ∃ v, env col = some v ∧ x.holds v)
    (htk : ∀ (i : Nat) (col : String) (ax : FAx), K.fixedCols[i]? = some col → K.fixedTakes[i]? = some ax →
      ∃ v, env col = some v ∧ ax.takes v)
    {P H : Nat} (hcount : K.count cls = some (P, H)) : K.share rnd env = (H, P) := by
  have hw' := hw
  simp only [wellB, Bool.and_eq_true, beq_iff_eq] at hw'
  obtain ⟨⟨⟨⟨⟨⟨⟨hlr, hlrand⟩, htiles⟩, _⟩, _⟩, _⟩, _⟩, _⟩ := hw'
  -- the draws are the coordinates' points, one axis after another
  have hgrids : K.draws.map RIn.grid = K.rand.map (fun c => c.flatMap RC.pts) := by
    apply List.ext_getElem
    · simp [hlrand]
    · intro j h1 h2
      simp only [List.getElem_map]
      have hjd : j < K.draws.length := by simpa using h1
      have hjr : j < K.rand.length := by omega
      have hzip : (K.draws[j], K.rand[j]) ∈ K.draws.zip K.rand := by
        rw [List.mem_iff_getElem?]
        exact ⟨j, by simp [List.getElem?_zip_eq_some, List.getElem?_eq_getElem hjd, List.getElem?_eq_getElem hjr]⟩
      exact (tiles_flatMap (List.all_eq_true.1 htiles _ hzip)).symm
  simp only [share, hgrids]
  -- each box: its points, and all of them or none
  let E := fun r => K.passesAt rnd (valueOf K.tables (K.extend env r) K.fuel)
  have hbox : ∀ box ∈ cart K.rand, ∀ p q, K.boxCount cls box = some (p, q) →
      (cart (box.map RC.pts)).length = p ∧ (cart (box.map RC.pts)).countP E = q := by
    intro box hb p q hbc
    have hlen : (cart (box.map RC.pts)).length = (box.map RC.count).prod := by
      rw [length_cart, List.map_map]
      congr 1
      apply List.map_congr_left
      intro c _
      exact (RC.count_eq c).symm
    simp only [boxCount] at hbc
    split at hbc
    · rename_i hz
      simp only [Option.some.injEq, Prod.mk.injEq] at hbc
      obtain ⟨rfl, rfl⟩ := hbc
      have h0 : (cart (box.map RC.pts)).length = 0 := by rw [hlen, hz]
      refine ⟨h0, ?_⟩
      rw [List.length_eq_zero_iff.1 h0]
      rfl
    · simp only [Option.map_eq_some_iff, Prod.mk.injEq] at hbc
      obtain ⟨hit, hh, rfl, rfl⟩ := hbc
      refine ⟨hlen, ?_⟩
      rw [countP_const (box_const K hw hrnd hcls htk hb hh), hlen]
  have hcount' : sumPairs ((cart K.rand).map (K.boxCount cls)) = some (P, H) := hcount
  obtain ⟨hP, hH⟩ := sumPairs_spec (K.boxCount cls) (fun box => (cart (box.map RC.pts)).length)
    (fun box => (cart (box.map RC.pts)).countP E) (cart K.rand) (fun box hb p q hbc => hbox box hb p q hbc) hcount'
  have hc := countP_cart_flatMap RC.pts K.rand E
  have hl := countP_cart_flatMap RC.pts K.rand (fun _ => true)
  simp only [List.countP_true] at hl
  rw [hc, hl, hH]
  congr 1

/-- A class of the walk at a call: one coordinate of each axis that is not random, each holding the
    value its column has at the call — there is one wherever every value stands on a coordinate. -/
theorem exists_class {env : String → Option Val} :
    ∀ (cols : List String) (fixed : List (List WCoord)), cols.length = fixed.length →
      (∀ (i : Nat) (col : String) (cs : List WCoord), cols[i]? = some col → fixed[i]? = some cs →
        ∃ (c : Nat) (x : WCoord), cs[c]? = some x ∧ ∃ v, env col = some v ∧ x.holds v) →
      ∃ cls ∈ cart fixed, ∀ (i : Nat) (col : String) (x : WCoord), cols[i]? = some col → cls[i]? = some x →
        ∃ v, env col = some v ∧ x.holds v
  | [], [], _, _ => ⟨[], by simp [cart], fun i col x hc _ => by simp at hc⟩
  | [], _ :: _, hl, _ => by simp at hl
  | _ :: _, [], hl, _ => by simp at hl
  | col :: cols, cs :: fixed, hl, h => by
    obtain ⟨c, x, hc, v, hv, hx⟩ := h 0 col cs rfl rfl
    obtain ⟨cls, hmem, hcls⟩ := exists_class cols fixed (by simpa using hl)
      (fun i col' cs' h1 h2 => h (i + 1) col' cs' (by simpa using h1) (by simpa using h2))
    refine ⟨x :: cls, ?_, ?_⟩
    · simp only [cart, List.mem_flatMap, List.mem_map]
      exact ⟨x, List.mem_of_getElem? hc, cls, hmem, rfl⟩
    · intro i col' x' h1 h2
      cases i with
      | zero =>
        simp only [List.getElem?_cons_zero, Option.some.injEq] at h1 h2
        subst h1; subst h2
        exact ⟨v, hv, hx⟩
      | succ i => exact hcls i col' x' (by simpa using h1) (by simpa using h2)

/-- **A claim of `chances` holds where its check passes.** Wherever the columns that are not random
    take values they can take and the inputs among them pass the claim's input cells, those values stand
    on a class of the walk (`FAx.cover`) that lies inside the cells (`CCell.inside_iff`), so the check
    has counted it; the draws whose outputs pass are its hits and the draws its points (`count_share`:
    the coordinates of the random axes tile what their inputs are drawn from, and inside a box one row of
    each table answers with one value), and the hits over the points pass the chance. -/
theorem holds (K : Claim) (h : K.okB = true) : K.Holds := by
  intro rnd hrnd env htakes hins hpos
  simp only [okB, Bool.and_eq_true] at h
  obtain ⟨hw, hall⟩ := h
  have hw' := hw
  simp only [wellB, Bool.and_eq_true, beq_iff_eq] at hw'
  obtain ⟨⟨⟨⟨⟨⟨⟨_, _⟩, _⟩, hfl⟩, htl⟩, hftiles⟩, hinsplit⟩, _⟩ := hw'
  have hcover : ∀ (i : Nat) (col : String) (cs : List WCoord), K.fixedCols[i]? = some col → K.fixed[i]? = some cs →
      ∃ (c : Nat) (x : WCoord), cs[c]? = some x ∧ ∃ v, env col = some v ∧ x.holds v := by
    intro i col cs hcol hcs
    have hi : i < K.fixed.length := (List.getElem?_eq_some_iff.1 hcs).1
    have hit : i < K.fixedTakes.length := by omega
    have hax : K.fixedTakes[i]? = some K.fixedTakes[i] := List.getElem?_eq_getElem hit
    obtain ⟨v, hv, htk⟩ := htakes i col _ hcol hax
    have hzip : (K.fixedTakes[i], cs) ∈ K.fixedTakes.zip K.fixed := by
      rw [List.mem_iff_getElem?]
      exact ⟨i, by simp [List.getElem?_zip_eq_some, hax, hcs]⟩
    have htile := List.all_eq_true.1 hftiles _ hzip
    obtain ⟨c, x, hc, hx⟩ := FAx.cover htile htk
    exact ⟨c, x, hc, v, hv, hx⟩
  obtain ⟨cls, hmem, hcls⟩ := exists_class K.fixedCols K.fixed hfl hcover
  obtain ⟨hclen, hclin⟩ := mem_cart hmem
  have hinside : K.insideAll cls = true := by
    simp only [insideAll]
    apply List.all_eq_true.2
    intro oc hoc
    have hs := List.all_eq_true.1 hinsplit oc hoc
    split at hs
    · rename_i i hi
      split at hs
      · rename_i cs hcs
        have hil : i < cls.length := by
          have := (List.getElem?_eq_some_iff.1 hcs).1
          omega
        have hx : cls[i]? = some cls[i] := List.getElem?_eq_getElem hil
        simp only [hx]
        have hxin : cls[i] ∈ cs := hclin i _ cs hx hcs
        have hsp := List.all_eq_true.1 hs _ hxin
        obtain ⟨v, hv, hxv⟩ := hcls i oc.1 cls[i] (getElem?_of_idxOf? hi) hx
        rw [CCell.inside_iff hsp hxv]
        have := hins oc hoc
        rw [hv] at this
        exact this
      · exact absurd hs (by simp)
    · exact absurd hs (by simp)
  have hok := List.all_eq_true.1 hall cls hmem
  rw [hinside] at hok
  simp only [Bool.not_true, Bool.false_or] at hok
  simp only [classOk] at hok
  split at hok
  · rename_i P H hcount
    have hshare := count_share K hw hrnd hcls htakes hcount
    rw [hshare] at hpos ⊢
    simp only at hpos ⊢
    have hP : (P == 0) = false := by
      rw [beq_eq_false_iff_ne]
      omega
    rw [hP, Bool.false_or] at hok
    exact hok
  · exact absurd hok (by simp)

end Claim

/-! ## Claims whose points are run one by one (rulec's §15.224)

Where the answer moves inside a box — a column the rule computes from a random input and one that is
not, a `constraint` between the two — the check runs the points one by one, for every combination of
the inputs that are not random its cells let in. So does the program: every value the rule computes is
worked out exactly (`evalExact`), which is the value under every rounding (`evalExact_sound`), and the
tables answer the call as they do anywhere (`valueOf`). -/

/-- A value the rule computes, as a column's value: a number, or a truth value as the word its axis
    writes. -/
def toVal : UVal → Val
  | (v, .other "bool") => .word (if v = 1 then "true" else "false")
  | (v, _) => .num v

/-- The inputs of a call, and every value the rule computes from them, one after another in the order
    the certificate lists them, under a rounding. -/
def envAt (rnd : Rat → Rat → Rat) (values : List (String × Expr)) (env : String → Option UVal) :
    String → Option UVal :=
  values.foldl (fun e ne => fun n => if n = ne.1 then eval rnd e ne.2 else e n) env

/-- The same, worked out exactly: `none` where a rounding meets a value off its grid. -/
def envExact (values : List (String × Expr)) (env : String → Option UVal) : Option (String → Option UVal) :=
  values.foldlM (fun e ne => (evalExact e ne.2).map (fun r => fun n => if n = ne.1 then r else e n)) env

/-- **What `envExact` works out is what every rounding gives.** -/
theorem envExact_sound {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd) :
    ∀ (values : List (String × Expr)) (env envE : String → Option UVal),
      envExact values env = some envE → envAt rnd values env = envE
  | [], env, envE, h => by
    simp only [envExact, List.foldlM_nil, Option.pure_def, Option.some.injEq] at h
    simp only [envAt, List.foldl_nil]
    exact h
  | ne :: values, env, envE, h => by
    simp only [envExact, List.foldlM_cons] at h
    cases he : evalExact env ne.2 with
    | none => simp [he] at h
    | some r =>
      simp only [he, Option.map_some] at h
      have hev := evalExact_sound hrnd ne.2 r he
      have := envExact_sound hrnd values (fun n => if n = ne.1 then r else env n) envE h
      simp only [envAt, List.foldl_cons]
      rw [← this]
      simp only [envAt]
      congr 1
      funext n
      rw [hev]

/-- What the names of a `constraint` line's side add up to, where each is a number. -/
def sumVals (env : String → Option Val) : List String → Option Rat
  | [] => some 0
  | n :: ns => match env n, sumVals env ns with
    | some (.num v), some r => some (v + r)
    | _, _ => none

/-- A `constraint` line holds of a call's inputs. -/
def consHolds (env : String → Option Val) (k : List String × Cmp × List String) : Bool :=
  match sumVals env k.1, sumVals env k.2.2 with
  | some a, some b => cmpHolds k.2.1 a b
  | _, _ => false

/-- **A claim whose points are run one by one**, as the program reads it: the tables, the values the
    rule computes and how deep the answers are read; the inputs that are not random and what each takes;
    the random inputs and what each is drawn from; the types of the numbers; the `constraint` lines the
    door holds the inputs to; the claim's cells, the outputs' grids and the chance it writes. -/
structure RunClaim where
  tables : List ATable
  fuel : Nat
  values : List (String × Expr)
  types : String → Option Ty
  fixedCols : List String
  fixedIn : List RIn
  randCols : List String
  draws : List RIn
  cons : List (List String × Cmp × List String)
  ins : List (String × CCell)
  outs : List (String × CCell)
  rounds : String → Option Rat
  test : ChanceTest

/-- The lowest a cell's comparisons let a number be, and the highest, from the range's ends. -/
def lowFrom (ts : List (Cmp × Rat)) (lo : Rat) : Rat :=
  ts.foldl (fun a t => match t.1 with
    | .ge | .gt => rmax a t.2
    | _ => a) lo

def highFrom (ts : List (Cmp × Rat)) (hi : Rat) : Rat :=
  ts.foldl (fun b t => match t.1 with
    | .le | .lt => rmin b t.2
    | _ => b) hi

theorem le_lowFrom {v : Rat} : ∀ (ts : List (Cmp × Rat)) (lo : Rat), lo ≤ v →
    (∀ t ∈ ts, cmpHolds t.1 v t.2 = true) → lowFrom ts lo ≤ v
  | [], lo, h, _ => h
  | t :: ts, lo, h, hts => by
    simp only [lowFrom, List.foldl_cons]
    have ht := cmpHolds_iff.1 (hts t (by simp))
    refine le_lowFrom ts _ ?_ (fun t' ht' => hts t' (List.mem_cons_of_mem _ ht'))
    match t, ht with
    | (.ge, w), ht => exact rmax_le h ht
    | (.gt, w), ht => exact rmax_le h (Rat.le_of_lt ht)
    | (.le, _), _ => exact h
    | (.lt, _), _ => exact h
    | (.eq, _), _ => exact h

theorem highFrom_le {v : Rat} : ∀ (ts : List (Cmp × Rat)) (hi : Rat), v ≤ hi →
    (∀ t ∈ ts, cmpHolds t.1 v t.2 = true) → v ≤ highFrom ts hi
  | [], hi, h, _ => h
  | t :: ts, hi, h, hts => by
    simp only [highFrom, List.foldl_cons]
    have ht := cmpHolds_iff.1 (hts t (by simp))
    refine highFrom_le ts _ ?_ (fun t' ht' => hts t' (List.mem_cons_of_mem _ ht'))
    match t, ht with
    | (.le, w), ht => exact le_rmin h ht
    | (.lt, w), ht => exact le_rmin h (Rat.le_of_lt ht)
    | (.ge, _), _ => exact h
    | (.gt, _), _ => exact h
    | (.eq, _), _ => exact h

/-- **The values an input takes that a cell lets in**: a comparison narrows a number's range before its
    steps are listed, and anything else is the whole of what it takes, kept where the cell passes. -/
def RIn.gridIn : RIn → CCell → List Val
  | .num lo hi k, .cmp ts => ((RIn.num (lowFrom ts lo) (highFrom ts hi) k).grid).filter (fun v => (CCell.cmp ts).passes (some v))
  | inp, c => inp.grid.filter (fun v => c.passes (some v))

/-- Every value an input takes that a cell lets in is among the ones `gridIn` lists. -/
theorem mem_gridIn {inp : RIn} {c : CCell} {v : Val} (hv : v ∈ inp.grid) (hc : c.passes (some v) = true) :
    v ∈ inp.gridIn c := by
  match inp, c with
  | .num lo hi k, .cmp ts =>
    simp only [RIn.gridIn, List.mem_filter]
    refine ⟨?_, hc⟩
    simp only [RIn.grid, steps, List.mem_map, mem_irange] at hv ⊢
    obtain ⟨n, ⟨h1, h2⟩, rfl⟩ := hv
    refine ⟨n, ⟨?_, ?_⟩, rfl⟩
    · -- the value passes every comparison, so it is at or above the narrowed low end
      have hall : ∀ t ∈ ts, cmpHolds t.1 ((n : Rat) / (k : Rat)) t.2 = true := by
        simp only [CCell.passes, List.all_eq_true] at hc
        exact hc
      by_cases hk : 0 < k
      · have hk' : (0 : Rat) < (k : Rat) := by exact_mod_cast hk
        have hlo : lo ≤ (n : Rat) / k := (le_div_iff' hk').2 (ceilI_le_iff.1 h1)
        exact ceilI_le_iff.2 ((le_div_iff' hk').1 (le_lowFrom ts lo hlo hall))
      · have hk0 : k = 0 := by omega
        subst hk0
        simpa using h1
    · have hall : ∀ t ∈ ts, cmpHolds t.1 ((n : Rat) / (k : Rat)) t.2 = true := by
        simp only [CCell.passes, List.all_eq_true] at hc
        exact hc
      by_cases hk : 0 < k
      · have hk' : (0 : Rat) < (k : Rat) := by exact_mod_cast hk
        have hhi : (n : Rat) / k ≤ hi := (div_le_iff' hk').2 (le_floorI_iff.1 h2)
        exact le_floorI_iff.2 ((div_le_iff' hk').1 (highFrom_le ts hi hhi hall))
      · have hk0 : k = 0 := by omega
        subst hk0
        simpa using h2
  | .num lo hi k, .any => simp [RIn.gridIn, List.mem_filter, hv, hc]
  | .num lo hi k, .nothing => simp [RIn.gridIn, List.mem_filter, hv, hc]
  | .num lo hi k, .words _ => simp [RIn.gridIn, List.mem_filter, hv, hc]
  | .num lo hi k, .notWords _ => simp [RIn.gridIn, List.mem_filter, hv, hc]
  | .num lo hi k, .nums _ => simp [RIn.gridIn, List.mem_filter, hv, hc]
  | .num lo hi k, .notNums _ => simp [RIn.gridIn, List.mem_filter, hv, hc]
  | .num lo hi k, .prefixes _ => simp [RIn.gridIn, List.mem_filter, hv, hc]
  | .words ws, c => simp [RIn.gridIn, List.mem_filter, hv, hc]

namespace RunClaim

/-- The inputs of a call: the values `xs` gives the inputs that are not random, and `rs` the random ones. -/
def inputs (K : RunClaim) (xs rs : List Val) : String → Option Val := fun n =>
  match K.randCols.idxOf? n with
  | some j => rs[j]?
  | none => match K.fixedCols.idxOf? n with
    | some i => xs[i]?
    | none => none

/-- The numbers among them, with their types, for the expressions. -/
def nums (K : RunClaim) (inp : String → Option Val) : String → Option UVal := fun n =>
  match inp n, K.types n with
  | some (.num v), some t => some (v, t)
  | _, _ => none

/-- The values of a call from the values of its names: the inputs, and the values the rule computes. -/
def withValues (K : RunClaim) (inp : String → Option Val) (env : String → Option UVal) : String → Option Val := fun n =>
  match inp n with
  | some v => some v
  | none => if K.values.any (fun ne => ne.1 == n) then (env n).map toVal else none

/-- **The values of a call**: its inputs, and every value the rule computes from them under the
    rounding. -/
def callAt (K : RunClaim) (rnd : Rat → Rat → Rat) (xs rs : List Val) : String → Option Val :=
  K.withValues (K.inputs xs rs) (envAt rnd K.values (K.nums (K.inputs xs rs)))

/-- The door the call passes: every `constraint` line holds of its inputs. -/
def door (K : RunClaim) (xs rs : List Val) : Bool := K.cons.all (consHolds (K.inputs xs rs))

/-- Whether the outputs of a call pass the claim's output cells. -/
def passesAt (K : RunClaim) (rnd : Rat → Rat → Rat) (vals : String → Option Val) : Bool :=
  K.outs.all (fun oc => oc.2.passes ((vals oc.1).map (Claim.roundVal rnd (K.rounds oc.1))))

/-- The inputs that are not random pass the claim's input cells. -/
def insPass (K : RunClaim) (xs : List Val) : Bool :=
  K.ins.all (fun oc => match K.fixedCols.idxOf? oc.1 with
    | some i => oc.2.passes xs[i]?
    | none => false)

/-- **What a claim run point by point says**: for every combination of values the inputs that are not
    random take and its cells let in, among the draws the door lets through, the share whose outputs pass
    the claim's output cells passes the chance — under every rounding. -/
def Holds (K : RunClaim) : Prop :=
  ∀ (rnd : Rat → Rat → Rat), RoundsWell rnd → ∀ xs ∈ cart (K.fixedIn.map RIn.grid), K.insPass xs = true →
    0 < ((cart (K.draws.map RIn.grid)).filter (K.door xs)).length →
    K.test.holds ((((cart (K.draws.map RIn.grid)).filter (K.door xs)).countP
        (fun rs => K.passesAt rnd (valueOf K.tables (K.callAt rnd xs rs) K.fuel)) : Rat) /
      (((cart (K.draws.map RIn.grid)).filter (K.door xs)).length : Rat)) = true

/-- Whether one output of an exactly worked out call passes its cell: `none` where its rounding would
    move it. -/
def outPass (K : RunClaim) (vals : String → Option Val) (oc : String × CCell) : Option Bool :=
  match oc.2 with
  | .any => some true
  | c =>
    match valueOf K.tables vals K.fuel oc.1 with
    | some v => if Claim.keepsB (K.rounds oc.1) v then some (c.passes (some v)) else none
    | none => some (c.passes none)

/-- **One draw, worked out exactly**: whether its outputs pass, where every value the rule computes is
    the same under every rounding and every output its rounding leaves as it is. -/
def pointHit (K : RunClaim) (xs rs : List Val) : Option Bool :=
  match envExact K.values (K.nums (K.inputs xs rs)) with
  | some env => Claim.allSome (K.outs.map (K.outPass (K.withValues (K.inputs xs rs) env)))
  | none => none

/-- The draws the door lets through, and the ones among them whose outputs pass. -/
def comboCount (K : RunClaim) (xs : List Val) : Option (Nat × Nat) :=
  Claim.sumPairs ((cart (K.draws.map RIn.grid)).map (fun rs =>
    if K.door xs rs then (K.pointHit xs rs).map (fun h => (1, if h then 1 else 0)) else some (0, 0)))

/-- The cell a claim writes on an input that is not random, `-` where it writes none. -/
def cellOn (K : RunClaim) (col : String) : CCell := (K.ins.lookup col).getD .any

/-- **The combinations a claim is run on**: for each input that is not random, the values it takes
    that the claim's cell on it lets in. -/
def combos (K : RunClaim) : List (List Val) :=
  cart ((K.fixedCols.zip K.fixedIn).map (fun ci => ci.2.gridIn (K.cellOn ci.1)))

/-- **The check of a claim run point by point**: every combination its cells let in passes it. -/
def okB (K : RunClaim) : Bool :=
  K.fixedCols.length == K.fixedIn.length && decide K.fixedCols.Nodup &&
  K.combos.all (fun xs => !K.insPass xs ||
    match K.comboCount xs with
    | some (d, h) => d == 0 || K.test.holds ((h : Rat) / (d : Rat))
    | none => false)

/-- An exactly worked out draw passes where the call passes, under every rounding. -/
theorem pointHit_sound (K : RunClaim) {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd) {xs rs : List Val}
    {hit : Bool} (h : K.pointHit xs rs = some hit) :
    K.passesAt rnd (valueOf K.tables (K.callAt rnd xs rs) K.fuel) = hit := by
  simp only [pointHit] at h
  split at h
  · rename_i env henv
    have heq : K.callAt rnd xs rs = K.withValues (K.inputs xs rs) env := by
      simp only [callAt, envExact_sound hrnd K.values _ env henv]
    rw [heq]
    obtain ⟨hsome, hall⟩ := Claim.allSome_spec (K.outPass (K.withValues (K.inputs xs rs) env)) K.outs h
    rw [hall]
    simp only [passesAt]
    apply Claim.all_congr_mem
    intro oc hoc
    obtain ⟨c, hc⟩ := hsome oc hoc
    rw [hc, Option.getD_some]
    simp only [outPass] at hc
    split at hc
    · rename_i hany
      simp only [Option.some.injEq] at hc
      subst hc
      rw [hany]
      rfl
    · split at hc
      · rename_i v hv
        split at hc
        · rename_i hkeep
          simp only [Option.some.injEq] at hc
          subst hc
          rw [hv]
          simp only [Option.map_some, Claim.roundVal_keeps hrnd hkeep]
        · exact absurd hc (by simp)
      · rename_i hv
        simp only [Option.some.injEq] at hc
        subst hc
        rw [hv]
        rfl
  · exact absurd h (by simp)

theorem length_filter_sum {β : Type} (p : β → Bool) :
    ∀ (l : List β), (l.filter p).length = (l.map (fun x => if p x then 1 else 0)).sum
  | [] => rfl
  | x :: l => by
    simp only [List.filter_cons, List.map_cons, List.sum_cons]
    cases hp : p x with
    | true => simp [length_filter_sum p l]; omega
    | false => simp [length_filter_sum p l]

theorem countP_filter_sum {β : Type} (p E : β → Bool) :
    ∀ (l : List β), ((l.filter p).countP E) = (l.map (fun x => if p x && E x then 1 else 0)).sum
  | [] => rfl
  | x :: l => by
    simp only [List.filter_cons, List.map_cons, List.sum_cons]
    cases hp : p x with
    | true =>
      cases he : E x with
      | true => simp [he, countP_filter_sum p E l]; omega
      | false => simp [he, countP_filter_sum p E l]
    | false => simp [countP_filter_sum p E l]

theorem mem_of_lookup {β : Type} {a : String} {c : β} : ∀ {l : List (String × β)}, l.lookup a = some c → (a, c) ∈ l
  | [], h => by simp [List.lookup] at h
  | (k, b) :: l, h => by
    simp only [List.lookup] at h
    split at h
    · rename_i hk
      simp only [Option.some.injEq] at h
      subst h
      have : a = k := beq_iff_eq.1 hk
      subst this
      simp
    · exact List.mem_cons_of_mem _ (mem_of_lookup h)

theorem idxOf?_of_nodup {l : List String} (hnd : l.Nodup) {i : Nat} {a : String} (h : l[i]? = some a) :
    l.idxOf? a = some i := by
  rw [List.idxOf?_eq_some_iff]
  obtain ⟨hi, ha⟩ := List.getElem?_eq_some_iff.1 h
  refine ⟨hi, ha, fun j hj heq => ?_⟩
  have hp := List.pairwise_iff_getElem.1 hnd j i (by omega) hi hj
  exact hp (heq.trans ha.symm)

/-- A combination of values the inputs take that passes the claim's cells is one the claim is run on. -/
theorem mem_combos (K : RunClaim) (hlen : K.fixedCols.length = K.fixedIn.length) (hnd : K.fixedCols.Nodup) {xs : List Val}
    (hxs : xs ∈ cart (K.fixedIn.map RIn.grid)) (hins : K.insPass xs = true) : xs ∈ K.combos := by
  obtain ⟨hl, hin⟩ := mem_cart hxs
  -- one value of each input's list, the one `xs` gives it
  have key : ∀ (cols : List String) (ins : List RIn) (vs : List Val), cols.length = ins.length →
      vs.length = ins.length →
      (∀ (i : Nat) (v : Val) (inp : RIn) (col : String), vs[i]? = some v → ins[i]? = some inp → cols[i]? = some col →
        v ∈ inp.gridIn (K.cellOn col)) →
      vs ∈ cart ((cols.zip ins).map (fun ci => ci.2.gridIn (K.cellOn ci.1))) := by
    intro cols
    induction cols with
    | nil =>
      intro ins vs hc hv _
      have : ins = [] := List.eq_nil_of_length_eq_zero (by simpa using hc.symm)
      subst this
      have : vs = [] := List.eq_nil_of_length_eq_zero (by simpa using hv)
      subst this
      simp [cart]
    | cons col cols ih =>
      intro ins vs hc hv hall
      match ins, vs, hc, hv with
      | inp :: ins, v :: vs, hc, hv =>
        simp only [List.zip_cons_cons, List.map_cons, cart, List.mem_flatMap, List.mem_map]
        refine ⟨v, hall 0 v inp col rfl rfl rfl, vs, ih ins vs (by simpa using hc) (by simpa using hv)
          (fun i v' inp' col' h1 h2 h3 => hall (i + 1) v' inp' col' (by simpa using h1) (by simpa using h2) (by simpa using h3)), rfl⟩
  refine key K.fixedCols K.fixedIn xs hlen (by simpa using hl) ?_
  intro i v inp col hv hinp hcol
  have hvg : v ∈ inp.grid := hin i v inp.grid hv (by simp [hinp])
  apply mem_gridIn hvg
  -- the claim's cell on the column lets the value in
  simp only [cellOn]
  cases hlk : K.ins.lookup col with
  | none => rfl
  | some c =>
    simp only [Option.getD_some]
    have hmem : (col, c) ∈ K.ins := mem_of_lookup hlk
    have := List.all_eq_true.1 hins (col, c) hmem
    simp only at this
    rw [idxOf?_of_nodup hnd hcol] at this
    simp only [hv] at this
    exact this

/-- **A claim run point by point holds where its check passes.** Every combination its cells let in was
    run, every draw the door lets through worked out exactly — the value of each name under every
    rounding — and the share of them whose outputs pass passes the chance. -/
theorem holds (K : RunClaim) (h : K.okB = true) : K.Holds := by
  intro rnd hrnd xs hxs hins hpos
  simp only [okB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
  have hx := List.all_eq_true.1 h.2 xs (mem_combos K h.1.1 h.1.2 hxs hins)
  rw [hins] at hx
  simp only [Bool.not_true, Bool.false_or] at hx
  split at hx
  · rename_i d hit hcount
    simp only [comboCount] at hcount
    have hspec := Claim.sumPairs_spec _
      (fun rs => if K.door xs rs then 1 else 0)
      (fun rs => if K.door xs rs && K.passesAt rnd (valueOf K.tables (K.callAt rnd xs rs) K.fuel) then 1 else 0)
      (cart (K.draws.map RIn.grid)) ?_ hcount
    · obtain ⟨hd, hh⟩ := hspec
      have hlen : ((cart (K.draws.map RIn.grid)).filter (K.door xs)).length = d := by
        rw [length_filter_sum]
        exact hd
      have hcnt : ((cart (K.draws.map RIn.grid)).filter (K.door xs)).countP
          (fun rs => K.passesAt rnd (valueOf K.tables (K.callAt rnd xs rs) K.fuel)) = hit := by
        rw [countP_filter_sum]
        exact hh
      rw [hlen] at hpos
      rw [hlen, hcnt]
      have hd0 : (d == 0) = false := by
        rw [beq_eq_false_iff_ne]
        omega
      rw [hd0, Bool.false_or] at hx
      exact hx
    · intro rs _ p q hpq
      by_cases hdoor : K.door xs rs = true
      · simp only [hdoor, ↓reduceIte, Option.map_eq_some_iff, Prod.mk.injEq] at hpq
        obtain ⟨hb, hhb, rfl, rfl⟩ := hpq
        have hE := pointHit_sound K hrnd hhb
        simp only [hdoor, ↓reduceIte, Bool.true_and, hE]
        exact ⟨trivial, trivial⟩
      · simp only [hdoor, Bool.false_eq_true, ↓reduceIte, Option.some.injEq, Prod.mk.injEq] at hpq
        obtain ⟨rfl, rfl⟩ := hpq
        simp [hdoor]
  · exact absurd hx (by simp)

end RunClaim

/-! ## Claims over a pick by weight (rulec's §15.226)

A fold that picks by weight walks the elements of a sequence: each element's verdict, read off the
tables, chooses its arm, and an arm that takes it puts the element's weight on a running total and takes
its value when the drawn value is below that total for the first time. The draw is below the total of the
weights — the door turns the rest away — unless they add up to 0, when the walk answers `exhausted`, and a
walk over no element answers `empty`. The claim names the sequence it is about, and the certificate
carries its elements, read back out of the file; the program runs the walk at every draw. -/

/-- One arm of a fold that picks by weight: the verdict it is for, and — where it takes the element —
    the value it takes and the weight it puts in. -/
structure WArm where
  verdict : String
  take : Option (Expr × Expr)
  deriving Repr, Inhabited

/-- **A claim over a pick by weight**, as the program reads it: what a claim run point by point has,
    and the walk — the elements of the sequence the claim names, field by field, the column of
    verdicts, the arms, the drawn input, the `empty` value, the `exhausted` one (`none` for `held`: what
    the walk took, or the `empty` value where it took nothing) and the column the walk answers. -/
structure WeightedClaim where
  tables : List ATable
  fuel : Nat
  values : List (String × Expr)
  types : String → Option Ty
  fixedCols : List String
  fixedIn : List RIn
  elems : List (List (String × Val))
  verdict : String
  arms : List WArm
  drawCol : String
  draw : RIn
  empty : Expr
  exhausted : Option Expr
  answer : String
  ins : List (String × CCell)
  outs : List (String × CCell)
  rounds : String → Option Rat
  test : ChanceTest

namespace WeightedClaim

/-- The inputs of a call: the values `xs` gives the inputs that are not random, and `r` the drawn one. -/
def inputs (K : WeightedClaim) (xs : List Val) (r : Val) : String → Option Val := fun n =>
  if n = K.drawCol then some r
  else match K.fixedCols.idxOf? n with
    | some i => xs[i]?
    | none => none

/-- The numbers among a call's values, with their types, for the expressions. -/
def nums (K : WeightedClaim) (vals : String → Option Val) : String → Option UVal := fun n =>
  match vals n, K.types n with
  | some (.num v), some t => some (v, t)
  | _, _ => none

/-- The values of a call from the values of its names: the inputs, and the values the rule computes. -/
def withValues (K : WeightedClaim) (inp : String → Option Val) (env : String → Option UVal) : String → Option Val := fun n =>
  match inp n with
  | some v => some v
  | none => if K.values.any (fun ne => ne.1 == n) then (env n).map toVal else none

/-- The values of a call before its walk: its inputs, and what the rule computes from them. -/
def callOf (K : WeightedClaim) (rnd : Rat → Rat → Rat) (xs : List Val) (r : Val) : String → Option Val :=
  K.withValues (K.inputs xs r) (envAt rnd K.values (K.nums (K.inputs xs r)))

/-- The values at one element: its fields, and the call's. -/
def elemEnv (e : List (String × Val)) (call : String → Option Val) : String → Option Val := fun n =>
  match e.lookup n with
  | some v => some v
  | none => call n

/-- What one element puts in, under a rounding: `some none` where its arm leaves it, `some (some (value,
    weight))` where it takes it; `none` where its verdict has no arm or a value of it is none. -/
def stepAt (K : WeightedClaim) (rnd : Rat → Rat → Rat) (call : String → Option Val) (e : List (String × Val)) :
    Option (Option (Val × Rat)) := do
  let v ← valueOf K.tables (elemEnv e call) K.fuel K.verdict
  let w ← match v with
    | .word w => some w
    | .num _ => none
  let arm ← K.arms.find? (fun a => a.verdict == w)
  match arm.take with
  | none => some none
  | some (ve, we) =>
    let x ← eval rnd (K.nums (elemEnv e call)) ve
    let q ← eval rnd (K.nums (elemEnv e call)) we
    some (some (toVal x, q.1))

/-- The value taken at the first element whose running total of weights the drawn value is below. -/
def pickBelow (r : Rat) : Rat → List (Val × Rat) → Option Val
  | _, [] => none
  | acc, (x, w) :: rest => if r < acc + w then some x else pickBelow r (acc + w) rest

/-- The total of the weights. -/
def total (ts : List (Val × Rat)) : Rat := ts.foldl (fun a xw => a + xw.2) 0

/-- **What the walk answers** at a draw, under a rounding. -/
def answerAt (K : WeightedClaim) (rnd : Rat → Rat → Rat) (xs : List Val) (r : Val) : Option Val :=
  let call := K.callOf rnd xs r
  let emptyV := (eval rnd (K.nums call) K.empty).map toVal
  if K.elems = [] then emptyV
  else
    match (K.elems.mapM (K.stepAt rnd call)).map (·.filterMap id) with
    | none => none
    | some ts =>
      match K.exhausted with
      | none =>
        match r with
        | .num q => match pickBelow q 0 ts with
          | some x => some x
          | none => emptyV
        | .word _ => none
      | some e => (eval rnd (K.nums call) e).map toVal

/-- The door the draw passes: below the total of the weights, unless they add up to 0. -/
def door (K : WeightedClaim) (rnd : Rat → Rat → Rat) (xs : List Val) (r : Val) : Bool :=
  if K.elems = [] then true
  else
    match (K.elems.mapM (K.stepAt rnd (K.callOf rnd xs r))).map (·.filterMap id), r with
    | some ts, .num q => total ts == 0 || decide (q < total ts)
    | _, _ => false

/-- The values of a call, the walk's answer among them. -/
def callAt (K : WeightedClaim) (rnd : Rat → Rat → Rat) (xs : List Val) (r : Val) : String → Option Val := fun n =>
  if n = K.answer then K.answerAt rnd xs r else K.callOf rnd xs r n

/-- Whether the outputs of a call pass the claim's output cells. -/
def passesAt (K : WeightedClaim) (rnd : Rat → Rat → Rat) (vals : String → Option Val) : Bool :=
  K.outs.all (fun oc => oc.2.passes ((vals oc.1).map (Claim.roundVal rnd (K.rounds oc.1))))

/-- The inputs that are not random pass the claim's input cells. -/
def insPass (K : WeightedClaim) (xs : List Val) : Bool :=
  K.ins.all (fun oc => match K.fixedCols.idxOf? oc.1 with
    | some i => oc.2.passes xs[i]?
    | none => false)

/-- **What a claim over a pick by weight says**: for every combination of values the inputs that are
    not random take and its cells let in, among the draws the door lets through, the share whose outputs
    pass the claim's output cells passes the chance — under every rounding. -/
def Holds (K : WeightedClaim) : Prop :=
  ∀ (rnd : Rat → Rat → Rat), RoundsWell rnd → ∀ xs ∈ cart (K.fixedIn.map RIn.grid), K.insPass xs = true →
    0 < (K.draw.grid.filter (K.door rnd xs)).length →
    K.test.holds ((((K.draw.grid.filter (K.door rnd xs)).countP
        (fun r => K.passesAt rnd (valueOf K.tables (K.callAt rnd xs r) K.fuel))) : Rat) /
      ((K.draw.grid.filter (K.door rnd xs)).length : Rat)) = true

/-! ### Worked out exactly -/

/-- The values of a call before its walk, worked out exactly. -/
def callExact (K : WeightedClaim) (xs : List Val) (r : Val) : Option (String → Option Val) :=
  (envExact K.values (K.nums (K.inputs xs r))).map (K.withValues (K.inputs xs r))

/-- An exact value as a column's: `none` where a rounding would move it. -/
def exactVal (env : String → Option UVal) (e : Expr) : Option (Option Val) :=
  (evalExact env e).map (fun r => r.map toVal)

/-- What one element puts in, worked out exactly. -/
def stepExact (K : WeightedClaim) (call : String → Option Val) (e : List (String × Val)) : Option (Option (Val × Rat)) := do
  let v ← valueOf K.tables (elemEnv e call) K.fuel K.verdict
  let w ← match v with
    | .word w => some w
    | .num _ => none
  let arm ← K.arms.find? (fun a => a.verdict == w)
  match arm.take with
  | none => some none
  | some (ve, we) =>
    let x ← (evalExact (K.nums (elemEnv e call)) ve).join
    let q ← (evalExact (K.nums (elemEnv e call)) we).join
    some (some (toVal x, q.1))

/-- What the walk answers, worked out exactly, once the elements are in: what it took, or the `empty`
    value; or the `exhausted` value. -/
def ansExact (K : WeightedClaim) (call : String → Option Val) (emptyV : Option Val) (ts : List (Val × Rat)) (r : Val) :
    Option (Option Val) :=
  match K.exhausted with
  | none =>
    match r with
    | .num q => some (match pickBelow q 0 ts with
      | some x => some x
      | none => emptyV)
    | .word _ => some none
  | some e => exactVal (K.nums call) e

/-- What the walk answers, worked out exactly: the `empty` value over no element. -/
def ansOf (K : WeightedClaim) (call : String → Option Val) (emptyV : Option Val) (ts : List (Val × Rat)) (r : Val) :
    Option (Option Val) :=
  if K.elems = [] then some emptyV else K.ansExact call emptyV ts r

/-- Whether one output of an exactly worked out call passes its cell: `none` where its rounding would
    move it. -/
def outPass (K : WeightedClaim) (vals : String → Option Val) (oc : String × CCell) : Option Bool :=
  match oc.2 with
  | .any => some true
  | c =>
    match valueOf K.tables vals K.fuel oc.1 with
    | some v => if Claim.keepsB (K.rounds oc.1) v then some (c.passes (some v)) else none
    | none => some (c.passes none)

/-- The values of a call with the walk's answer among them. -/
def withAnswer (K : WeightedClaim) (ans : Option Val) (call : String → Option Val) : String → Option Val := fun n =>
  if n = K.answer then ans else call n

/-- The elements as they come in, worked out exactly. -/
def tsExact (K : WeightedClaim) (call : String → Option Val) : Option (List (Val × Rat)) :=
  if K.elems = [] then some [] else (K.elems.mapM (K.stepExact call)).map (·.filterMap id)

/-- Whether the door lets a draw through, the elements in. -/
def doorOf (K : WeightedClaim) (ts : List (Val × Rat)) (r : Val) : Bool :=
  if K.elems = [] then true else
    match r with
    | .num q => total ts == 0 || decide (q < total ts)
    | .word _ => false

/-- One draw, worked out exactly: whether the door lets it through, and whether its outputs pass. -/
def pointHit (K : WeightedClaim) (xs : List Val) (r : Val) : Option (Bool × Bool) := do
  let call ← K.callExact xs r
  let emptyV ← exactVal (K.nums call) K.empty
  let ts ← K.tsExact call
  let ans ← K.ansOf call emptyV ts r
  let hit ← Claim.allSome (K.outs.map (K.outPass (K.withAnswer ans call)))
  some (K.doorOf ts r, hit)

/-- The draws the door lets through, and the ones among them whose outputs pass. -/
def comboCount (K : WeightedClaim) (xs : List Val) : Option (Nat × Nat) :=
  Claim.sumPairs (K.draw.grid.map (fun r => (K.pointHit xs r).map (fun dh =>
    if dh.1 then (1, if dh.2 then 1 else 0) else (0, 0))))

/-- The cell a claim writes on an input that is not random, `-` where it writes none. -/
def cellOn (K : WeightedClaim) (col : String) : CCell := (K.ins.lookup col).getD .any

/-- The combinations a claim is run on. -/
def combos (K : WeightedClaim) : List (List Val) :=
  cart ((K.fixedCols.zip K.fixedIn).map (fun ci => ci.2.gridIn (K.cellOn ci.1)))

/-- **The check of a claim over a pick by weight**: every combination its cells let in passes it. -/
def okB (K : WeightedClaim) : Bool :=
  K.fixedCols.length == K.fixedIn.length && decide K.fixedCols.Nodup &&
  K.combos.all (fun xs => !K.insPass xs ||
    match K.comboCount xs with
    | some (d, h) => d == 0 || K.test.holds ((h : Rat) / (d : Rat))
    | none => false)

/-- A combination of values the inputs take that passes the cells is one the claim is run on. -/
theorem mem_combos (K : WeightedClaim) (hlen : K.fixedCols.length = K.fixedIn.length) (hnd : K.fixedCols.Nodup)
    {xs : List Val} (hxs : xs ∈ cart (K.fixedIn.map RIn.grid)) (hins : K.insPass xs = true) : xs ∈ K.combos := by
  obtain ⟨hl, hin⟩ := mem_cart hxs
  have key : ∀ (cols : List String) (ins : List RIn) (vs : List Val), cols.length = ins.length →
      vs.length = ins.length →
      (∀ (i : Nat) (v : Val) (inp : RIn) (col : String), vs[i]? = some v → ins[i]? = some inp → cols[i]? = some col →
        v ∈ inp.gridIn (K.cellOn col)) →
      vs ∈ cart ((cols.zip ins).map (fun ci => ci.2.gridIn (K.cellOn ci.1))) := by
    intro cols
    induction cols with
    | nil =>
      intro ins vs hc hv _
      have : ins = [] := List.eq_nil_of_length_eq_zero (by simpa using hc.symm)
      subst this
      have : vs = [] := List.eq_nil_of_length_eq_zero (by simpa using hv)
      subst this
      simp [cart]
    | cons col cols ih =>
      intro ins vs hc hv hall
      match ins, vs, hc, hv with
      | inp :: ins, v :: vs, hc, hv =>
        simp only [List.zip_cons_cons, List.map_cons, cart, List.mem_flatMap, List.mem_map]
        refine ⟨v, hall 0 v inp col rfl rfl rfl, vs, ih ins vs (by simpa using hc) (by simpa using hv)
          (fun i v' inp' col' h1 h2 h3 => hall (i + 1) v' inp' col' (by simpa using h1) (by simpa using h2) (by simpa using h3)), rfl⟩
  refine key K.fixedCols K.fixedIn xs hlen (by simpa using hl) ?_
  intro i v inp col hv hinp hcol
  have hvg : v ∈ inp.grid := hin i v inp.grid hv (by simp [hinp])
  apply mem_gridIn hvg
  simp only [cellOn]
  cases hlk : K.ins.lookup col with
  | none => rfl
  | some c =>
    simp only [Option.getD_some]
    have hmem : (col, c) ∈ K.ins := RunClaim.mem_of_lookup hlk
    have := List.all_eq_true.1 hins (col, c) hmem
    simp only at this
    rw [RunClaim.idxOf?_of_nodup hnd hcol] at this
    simp only [hv] at this
    exact this

theorem callExact_sound (K : WeightedClaim) {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd) {xs : List Val} {r : Val}
    {call : String → Option Val} (h : K.callExact xs r = some call) : K.callOf rnd xs r = call := by
  simp only [callExact, Option.map_eq_some_iff] at h
  obtain ⟨env, henv, rfl⟩ := h
  simp only [callOf, envExact_sound hrnd K.values _ env henv]

theorem exactVal_sound {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd) {env : String → Option UVal} {e : Expr}
    {v : Option Val} (h : exactVal env e = some v) : (eval rnd env e).map toVal = v := by
  simp only [exactVal, Option.map_eq_some_iff] at h
  obtain ⟨r, hr, rfl⟩ := h
  rw [evalExact_sound hrnd e r hr]

theorem join_some {α : Type} {o : Option (Option α)} {x : α} (h : o.join = some x) : o = some (some x) := by
  cases o with
  | none => simp at h
  | some o' => cases o' with
    | none => simp at h
    | some y => simp at h; rw [h]

theorem stepExact_sound (K : WeightedClaim) {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd) {call : String → Option Val}
    {e : List (String × Val)} {s : Option (Val × Rat)} (h : K.stepExact call e = some s) : K.stepAt rnd call e = some s := by
  simp only [stepExact, Option.bind_eq_bind] at h
  simp only [stepAt, Option.bind_eq_bind]
  cases hv : valueOf K.tables (elemEnv e call) K.fuel K.verdict with
  | none => simp [hv] at h
  | some v =>
    simp only [hv, Option.bind_some] at h ⊢
    cases v with
    | num _ => simp at h
    | word w =>
      simp only at h ⊢
      cases ha : K.arms.find? (fun a => a.verdict == w) with
      | none => simp [ha] at h
      | some arm =>
        simp only [ha, Option.bind_some] at h ⊢
        cases ht : arm.take with
        | none => simp only [ht] at h ⊢; exact h
        | some vw =>
          obtain ⟨ve, we⟩ := vw
          simp only [ht] at h ⊢
          cases hx : (evalExact (K.nums (elemEnv e call)) ve).join with
          | none => simp [hx] at h
          | some x =>
            cases hq : (evalExact (K.nums (elemEnv e call)) we).join with
            | none => simp [hx, hq] at h
            | some q =>
              simp only [hx, hq, Option.bind_some] at h
              rw [evalExact_sound hrnd ve (some x) (join_some hx), evalExact_sound hrnd we (some q) (join_some hq)]
              simp only [Option.bind_some]
              exact h

theorem outPass_sound (K : WeightedClaim) {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd)
    {vals : String → Option Val} {hb : Bool} (hhit : Claim.allSome (K.outs.map (K.outPass vals)) = some hb) :
    K.passesAt rnd (valueOf K.tables vals K.fuel) = hb := by
  obtain ⟨hsome, hall⟩ := Claim.allSome_spec _ K.outs hhit
  rw [hall]
  simp only [passesAt]
  apply Claim.all_congr_mem
  intro oc hoc
  obtain ⟨c, hc'⟩ := hsome oc hoc
  rw [hc', Option.getD_some]
  simp only [outPass] at hc'
  split at hc'
  · rename_i hany
    simp only [Option.some.injEq] at hc'
    subst hc'
    rw [hany]
    rfl
  · split at hc'
    · rename_i v hv
      split at hc'
      · rename_i hkeep
        simp only [Option.some.injEq] at hc'
        subst hc'
        rw [hv]
        simp only [Option.map_some, Claim.roundVal_keeps hrnd hkeep]
      · exact absurd hc' (by simp)
    · rename_i hv
      simp only [Option.some.injEq] at hc'
      subst hc'
      rw [hv]
      rfl

/-- **An exactly worked out draw is the draw under every rounding**: whether the door lets it through,
    and whether its outputs pass. -/
theorem pointHit_sound (K : WeightedClaim) {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd) {xs : List Val} {r : Val}
    {d h : Bool} (hp : K.pointHit xs r = some (d, h)) :
    K.door rnd xs r = d ∧ K.passesAt rnd (valueOf K.tables (K.callAt rnd xs r) K.fuel) = h := by
  simp only [pointHit, Option.bind_eq_bind] at hp
  cases hc : K.callExact xs r with
  | none => simp [hc] at hp
  | some call =>
    have hcall := K.callExact_sound hrnd hc
    simp only [hc, Option.bind_some] at hp
    cases he : exactVal (K.nums call) K.empty with
    | none => simp [he] at hp
    | some emptyV =>
      have hempty := exactVal_sound hrnd he
      simp only [he, Option.bind_some] at hp
      cases hts : K.tsExact call with
      | none => simp [hts] at hp
      | some ts =>
        simp only [hts, Option.bind_some] at hp
        -- the elements come in under the rounding as they were worked out exactly
        have hts' : K.elems ≠ [] → (K.elems.mapM (K.stepAt rnd call)).map (·.filterMap id) = some ts := by
          intro hne
          simp only [tsExact, hne, ↓reduceIte, Option.map_eq_some_iff] at hts
          obtain ⟨ss, hss, rfl⟩ := hts
          exact Option.map_eq_some_iff.2 ⟨ss, mapM_of_pointwise (fun e s hs => K.stepExact_sound hrnd hs) K.elems hss, rfl⟩
        cases hans : K.ansOf call emptyV ts r with
        | none => simp [hans] at hp
        | some ans =>
          simp only [hans, Option.bind_some] at hp
          cases hhit : Claim.allSome (K.outs.map (K.outPass (K.withAnswer ans call))) with
          | none => simp [hhit] at hp
          | some hb =>
            simp only [hhit, Option.bind_some, Option.some.injEq, Prod.mk.injEq] at hp
            obtain ⟨rfl, rfl⟩ := hp
            have hansAt : K.answerAt rnd xs r = ans := by
              by_cases hnil : K.elems = []
              · simp only [ansOf, hnil, ↓reduceIte, Option.some.injEq] at hans
                subst hans
                simp only [answerAt, hnil, ↓reduceIte, hcall, hempty]
              · simp only [ansOf, hnil, ↓reduceIte] at hans
                simp only [answerAt, hnil, ↓reduceIte, hcall, hts' hnil, hempty]
                simp only [ansExact] at hans
                cases hx : K.exhausted with
                | none =>
                  rw [hx] at hans
                  cases r with
                  | num q =>
                    simp only [Option.some.injEq] at hans
                    rw [← hans]
                  | word _ =>
                    simp only [Option.some.injEq] at hans
                    rw [← hans]
                | some e =>
                  rw [hx] at hans
                  exact exactVal_sound hrnd hans
            refine ⟨?_, ?_⟩
            · by_cases hnil : K.elems = []
              · simp [door, doorOf, hnil]
              · simp only [door, doorOf, hnil, ↓reduceIte, hcall, hts' hnil]
                cases r <;> rfl
            · have hcallAt : K.callAt rnd xs r = K.withAnswer ans call := by
                funext n
                simp only [callAt, withAnswer, hcall, hansAt]
              rw [hcallAt]
              exact K.outPass_sound hrnd hhit

/-- **A claim over a pick by weight holds where its check passes.** Every combination its cells let in
    was run, at every draw the walk was worked out exactly — the elements' verdicts off the rows, their
    values and weights under every rounding — and the share of the draws the door lets through whose
    outputs pass passes the chance. -/
theorem holds (K : WeightedClaim) (h : K.okB = true) : K.Holds := by
  intro rnd hrnd xs hxs hins hpos
  simp only [okB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
  have hx := List.all_eq_true.1 h.2 xs (K.mem_combos h.1.1 h.1.2 hxs hins)
  rw [hins] at hx
  simp only [Bool.not_true, Bool.false_or] at hx
  split at hx
  · rename_i d hit hcount
    simp only [comboCount] at hcount
    have hspec := Claim.sumPairs_spec _
      (fun r => if K.door rnd xs r then 1 else 0)
      (fun r => if K.door rnd xs r && K.passesAt rnd (valueOf K.tables (K.callAt rnd xs r) K.fuel) then 1 else 0)
      K.draw.grid ?_ hcount
    · obtain ⟨hd, hh⟩ := hspec
      have hlen : (K.draw.grid.filter (K.door rnd xs)).length = d := by
        rw [RunClaim.length_filter_sum]
        exact hd
      have hcnt : (K.draw.grid.filter (K.door rnd xs)).countP
          (fun r => K.passesAt rnd (valueOf K.tables (K.callAt rnd xs r) K.fuel)) = hit := by
        rw [RunClaim.countP_filter_sum]
        exact hh
      rw [hlen] at hpos
      rw [hlen, hcnt]
      have hd0 : (d == 0) = false := by
        rw [beq_eq_false_iff_ne]
        omega
      rw [hd0, Bool.false_or] at hx
      exact hx
    · intro r _ p q hpq
      simp only [Option.map_eq_some_iff] at hpq
      obtain ⟨⟨dr, ht⟩, hpt, hpq⟩ := hpq
      obtain ⟨hdoor, hE⟩ := K.pointHit_sound hrnd hpt
      rw [hdoor, hE]
      cases dr with
      | true =>
        simp only [↓reduceIte, Prod.mk.injEq] at hpq
        obtain ⟨rfl, rfl⟩ := hpq
        cases ht <;> simp
      | false =>
        simp only [Bool.false_eq_true, ↓reduceIte, Prod.mk.injEq] at hpq
        obtain ⟨rfl, rfl⟩ := hpq
        simp
  · exact absurd hx (by simp)

end WeightedClaim

end RulecCert.Chances
