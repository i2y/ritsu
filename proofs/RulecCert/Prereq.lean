/-
  Which values of an enum need which before them (rulec's §15.222), as far as a certificate speaks
  about it.

  A `prerequisites` block is a finite graph over the values of one enum. A case holds the `start`
  values from the first, and can take a value it does not hold yet once it holds one value of every
  group the value needs: the generated `<name>_available`. A value written `requires nothing` needs one
  group that nothing fills, so no case takes it.

  The certificate hands over two orders of the values, and `RulecMain` checks them. One puts every
  value after every value it may need, the ones joined by `or` included (`topological`): then no value
  needs itself, however far its needs are followed (`no_cycle`). In the other, a case can take the
  values one after another from `start` (`reached`): then a case can come to hold every value of it
  (`reached_holds`), and the program checks that every value but those no case can take is in it.
  Those no case can take no case ever holds (`never_held`). Where the block is `ordered by` a table,
  the program reads each value's answer off the table's rows and checks it along every edge into a
  value a case can take: then no value is placed below anything it needs, however far its needs are
  followed (`order_along`).
-/

namespace RulecCert
namespace Prereq

/-- A `prerequisites` block over one enum: the values a case holds from the start, each value's
    `requires` line — every group needed, any one value of a group enough — and the values no case can
    take (`requires nothing`). -/
structure Graph where
  start : List String
  requires : List (String × List (List String))
  never : List String

/-- The groups a value needs: its own `requires` line's; none for a value on `start`; and for one no
    case can take, one empty group, which nothing fills. What the generated `<name>_requires`
    answers. -/
def Graph.groups (G : Graph) (v : String) : List (List String) :=
  if G.never.contains v then [[]] else (G.requires.lookup v).getD []

/-- A case that holds `held` can take `v`: it does not hold it yet, and holds a value of every group
    `v` needs. What the generated `<name>_available` answers. -/
def Graph.available (G : Graph) (held : List String) (v : String) : Prop :=
  v ∉ held ∧ ∀ g ∈ G.groups v, ∃ x ∈ g, x ∈ held

/-- **What a case can come to hold**: the `start` values, and then one value it can take at a time. -/
inductive Graph.Holds (G : Graph) : List String → Prop
  | start : Holds G G.start
  | take {held : List String} {v : String} : Holds G held → G.available held v → Holds G (held ++ [v])

/-- `x` is one of the values `v` needs: a group of `v` has it, alone or joined by `or`. -/
def Graph.Edge (G : Graph) (v x : String) : Prop := ∃ g ∈ G.groups v, x ∈ g

/-- A chain of needs, one edge long or more: `v` needs `x` through the values between them. -/
inductive Graph.Needs (G : Graph) : String → String → Prop
  | edge {v x : String} : G.Edge v x → Needs G v x
  | trans {v x y : String} : G.Edge v x → Needs G x y → Needs G v y

/-! ## The checks -/

/-- `x` stands before `v` in `order`. -/
def before (order : List String) (x v : String) : Bool :=
  order.contains x && order.contains v && decide (order.idxOf x < order.idxOf v)

/-- **The topological check**: every value with a `requires` line stands in `order` after every value
    it may need. -/
def topoOk (G : Graph) (order : List String) : Bool :=
  G.requires.all (fun p => (G.groups p.1).all (fun g => g.all (fun x => before order x p.1)))

/-- **The reach check**, over what `reached` has named so far (`seen`): no value twice, and each one on
    `start`, or with a value of each group it needs on `start` or named before it. -/
def reachedGo (G : Graph) : List String → List String → Bool
  | _, [] => true
  | seen, v :: rest =>
    !seen.contains v &&
      (G.start.contains v || (G.groups v).all (fun g => g.any (fun x => G.start.contains x || seen.contains x))) &&
      reachedGo G (seen ++ [v]) rest

def reachedOk (G : Graph) (reached : List String) : Bool := reachedGo G [] reached

/-- **The order check**: along every edge into a value a case can take, the value's answer is not
    below the answer of the value it needs. `key` places an answer: an ordered enum's value by its
    place, a number by itself. -/
def orderOk (G : Graph) (key : String → Option Rat) : Bool :=
  G.requires.all (fun p => (G.groups p.1).all (fun g => g.all (fun x =>
    G.never.contains x ||
      match key x, key p.1 with
      | some a, some b => decide (a ≤ b)
      | _, _ => false)))

/-! ## What they settle -/

/-- The `requires` line the groups of a value come from, where one of them has a value in it. -/
theorem groups_line {G : Graph} {v x : String} {g : List String} (hg : g ∈ G.groups v) (hx : x ∈ g) :
    G.never.contains v = false ∧ ∃ gs, (v, gs) ∈ G.requires ∧ G.groups v = gs := by
  unfold Graph.groups at hg ⊢
  cases hn : G.never.contains v with
  | true =>
    rw [hn] at hg
    simp only [ite_true, List.mem_singleton] at hg
    subst hg
    simp at hx
  | false =>
    refine ⟨rfl, ?_⟩
    rw [hn] at hg
    simp only [Bool.false_eq_true, ite_false] at hg ⊢
    cases hl : G.requires.lookup v with
    | none => rw [hl] at hg; simp at hg
    | some gs =>
      obtain ⟨l₁, l₂, hl', _⟩ := List.lookup_eq_some_iff.1 hl
      exact ⟨gs, by rw [hl']; simp, by simp⟩

/-- A value with an edge out of it is not one no case can take. -/
theorem not_never_of_edge {G : Graph} {v x : String} (he : G.Edge v x) : G.never.contains v = false := by
  obtain ⟨g, hg, hx⟩ := he
  exact (groups_line hg hx).1

/-- **Every value stands after every value it needs**, where the topological check passes. -/
theorem edge_before {G : Graph} {order : List String} (h : topoOk G order = true) {v x : String}
    (he : G.Edge v x) : order.idxOf x < order.idxOf v := by
  obtain ⟨g, hg, hx⟩ := he
  obtain ⟨_, gs, hmem, hgs⟩ := groups_line hg hx
  have hp := List.all_eq_true.1 h (v, gs) hmem
  simp only at hp
  have hb := List.all_eq_true.1 (List.all_eq_true.1 hp g hg) x hx
  simp only [before, Bool.and_eq_true, decide_eq_true_eq] at hb
  exact hb.2

theorem needs_before {G : Graph} {order : List String} (h : topoOk G order = true) {v x : String}
    (hn : G.Needs v x) : order.idxOf x < order.idxOf v := by
  induction hn with
  | edge he => exact edge_before h he
  | trans he _ ih => exact Nat.lt_trans ih (edge_before h he)

/-- **No cycle**: where the topological check passes, no value needs itself, however far its needs are
    followed — the values joined by `or` included. -/
theorem no_cycle {G : Graph} {order : List String} (h : topoOk G order = true) (v : String) :
    ¬ G.Needs v v :=
  fun hn => Nat.lt_irrefl _ (needs_before h hn)

/-- The walk the reach check makes, as a case takes the values: from what it holds — the `start` values
    and the values named so far — it takes each value not on `start` in turn. -/
theorem reachedGo_holds {G : Graph} :
    ∀ (rest seen held : List String), G.Holds held → (∀ x, x ∈ held ↔ x ∈ G.start ∨ x ∈ seen) →
      reachedGo G seen rest = true →
      ∃ held', G.Holds held' ∧ ∀ x, x ∈ held' ↔ x ∈ G.start ∨ x ∈ seen ∨ x ∈ rest
  | [], seen, held, hh, hiff, _ => ⟨held, hh, fun x => by rw [hiff x]; simp⟩
  | v :: rest, seen, held, hh, hiff, h => by
    simp only [reachedGo, Bool.and_eq_true, Bool.not_eq_true', Bool.or_eq_true, List.all_eq_true,
      List.any_eq_true] at h
    obtain ⟨⟨hnew, hcan⟩, hrest⟩ := h
    have hvs : v ∉ seen := fun hv => by rw [List.contains_iff_mem.2 hv] at hnew; exact absurd hnew (by simp)
    by_cases hst : G.start.contains v = true
    · -- A value on `start` is held from the first.
      have hvst : v ∈ G.start := List.contains_iff_mem.1 hst
      obtain ⟨held', hh', hiff'⟩ := reachedGo_holds rest (seen ++ [v]) held hh
        (fun x => by rw [hiff x]; simp only [List.mem_append, List.mem_singleton]; constructor
                     · rintro (h1 | h1)
                       · exact Or.inl h1
                       · exact Or.inr (Or.inl h1)
                     · rintro (h1 | h1 | h1)
                       · exact Or.inl h1
                       · exact Or.inr h1
                       · subst h1; exact Or.inl hvst) hrest
      refine ⟨held', hh', fun x => ?_⟩
      rw [hiff' x]
      simp only [List.mem_append, List.mem_cons, List.not_mem_nil, or_false]
      constructor
      · rintro (h1 | (h1 | h1) | h1)
        · exact Or.inl h1
        · exact Or.inr (Or.inl h1)
        · exact Or.inr (Or.inr (Or.inl h1))
        · exact Or.inr (Or.inr (Or.inr h1))
      · rintro (h1 | h1 | h1 | h1)
        · exact Or.inl h1
        · exact Or.inr (Or.inl (Or.inl h1))
        · exact Or.inr (Or.inl (Or.inr h1))
        · exact Or.inr (Or.inr h1)
    · -- Any other value is one the case can take now: it does not hold it, and holds a value of
      -- every group it needs.
      have hgroups : ∀ g ∈ G.groups v, ∃ x ∈ g, G.start.contains x = true ∨ seen.contains x = true := by
        rcases hcan with hs | hg
        · exact absurd hs hst
        · intro g hg'
          exact hg g hg'
      have havail : G.available held v := by
        refine ⟨fun hv => ?_, fun g hg => ?_⟩
        · rcases (hiff v).1 hv with h1 | h1
          · exact hst (List.contains_iff_mem.2 h1)
          · exact hvs h1
        · obtain ⟨x, hx, hx'⟩ := hgroups g hg
          refine ⟨x, hx, (hiff x).2 ?_⟩
          rcases hx' with h1 | h1
          · exact Or.inl (List.contains_iff_mem.1 h1)
          · exact Or.inr (List.contains_iff_mem.1 h1)
      obtain ⟨held', hh', hiff'⟩ := reachedGo_holds rest (seen ++ [v]) (held ++ [v]) (Graph.Holds.take hh havail)
        (fun x => by
          simp only [List.mem_append, List.mem_singleton]
          rw [hiff x]
          constructor
          · rintro ((h1 | h1) | h1)
            · exact Or.inl h1
            · exact Or.inr (Or.inl h1)
            · exact Or.inr (Or.inr h1)
          · rintro (h1 | h1 | h1)
            · exact Or.inl (Or.inl h1)
            · exact Or.inl (Or.inr h1)
            · exact Or.inr h1) hrest
      refine ⟨held', hh', fun x => ?_⟩
      rw [hiff' x]
      simp only [List.mem_append, List.mem_cons, List.not_mem_nil, or_false]
      constructor
      · rintro (h1 | (h1 | h1) | h1)
        · exact Or.inl h1
        · exact Or.inr (Or.inl h1)
        · exact Or.inr (Or.inr (Or.inl h1))
        · exact Or.inr (Or.inr (Or.inr h1))
      · rintro (h1 | h1 | h1 | h1)
        · exact Or.inl h1
        · exact Or.inr (Or.inl (Or.inl h1))
        · exact Or.inr (Or.inl (Or.inr h1))
        · exact Or.inr (Or.inr h1)

/-- **Every value of `reached` can be had**: where the reach check passes, a case that starts with the
    `start` values and takes the others in the order `reached` names them comes to hold them all. -/
theorem reached_holds {G : Graph} {reached : List String} (h : reachedOk G reached = true) :
    ∃ held, G.Holds held ∧ ∀ v ∈ reached, v ∈ held := by
  obtain ⟨held, hh, hiff⟩ := reachedGo_holds reached [] G.start Graph.Holds.start (fun x => by simp) h
  exact ⟨held, hh, fun v hv => (hiff v).2 (Or.inr (Or.inr hv))⟩

/-- **No case holds a value no case can take**: one written `requires nothing` (and not on `start`)
    needs a group that nothing fills. -/
theorem never_held {G : Graph} (hdis : ∀ v ∈ G.never, v ∉ G.start) :
    ∀ {held : List String}, G.Holds held → ∀ v ∈ G.never, v ∉ held := by
  intro held hh
  induction hh with
  | start => exact hdis
  | take _ havail ih =>
    rename_i held' w
    intro v hv hmem
    simp only [List.mem_append, List.mem_singleton] at hmem
    rcases hmem with h1 | h1
    · exact ih v hv h1
    · subst h1
      have hg : ([] : List String) ∈ G.groups v := by
        unfold Graph.groups
        rw [List.contains_iff_mem.2 hv]
        simp
      obtain ⟨x, hx, _⟩ := havail.2 [] hg
      simp at hx

/-- Along one edge into a value a case can take, the answers are in order. -/
theorem edge_order {G : Graph} {key : String → Option Rat} (h : orderOk G key = true) {v x : String}
    (he : G.Edge v x) (hx : G.never.contains x = false) :
    ∃ a b, key x = some a ∧ key v = some b ∧ a ≤ b := by
  obtain ⟨g, hg, hxg⟩ := he
  obtain ⟨_, gs, hmem, hgs⟩ := groups_line hg hxg
  have hp := List.all_eq_true.1 h (v, gs) hmem
  simp only at hp
  have hb := List.all_eq_true.1 (List.all_eq_true.1 hp g hg) x hxg
  rw [hx, Bool.false_or] at hb
  cases ha : key x with
  | none => rw [ha] at hb; exact absurd hb (by simp)
  | some a =>
    cases hb' : key v with
    | none => rw [ha, hb'] at hb; exact absurd hb (by simp)
    | some b =>
      rw [ha, hb'] at hb
      exact ⟨a, b, rfl, rfl, of_decide_eq_true hb⟩

/-- **No value is placed below anything it needs**, however far its needs are followed, where the
    order check passes and the value needed is one a case can take. -/
theorem order_along {G : Graph} {key : String → Option Rat} (h : orderOk G key = true) {v x : String}
    (hn : G.Needs v x) (hx : G.never.contains x = false) :
    ∃ a b, key x = some a ∧ key v = some b ∧ a ≤ b := by
  induction hn with
  | edge he => exact edge_order h he hx
  | trans he hn' ih =>
    rename_i v' y x'
    obtain ⟨a, c, hka, hkc, hac⟩ := ih hx
    -- The value between has an edge out of it, so a case can take it.
    have hy : G.never.contains y = false := by
      cases hn' with
      | edge he' => exact not_never_of_edge he'
      | trans he' _ => exact not_never_of_edge he'
    obtain ⟨c', b, hkc', hkb, hcb⟩ := edge_order h he hy
    rw [hkc] at hkc'
    cases hkc'
    exact ⟨a, b, hka, hkb, Rat.le_trans hac hcb⟩

end Prereq
end RulecCert
