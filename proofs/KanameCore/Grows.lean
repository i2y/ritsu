/-
  What the analysis keeps only grows (kaname's DESIGN 15.2.3): every place kept so far is kept to
  the end, taking what it takes and every value it held (joined with what comes later), and every
  function called stays called. And the scope it goes on in keeps its module, the answers of the
  functions analysed already, and its ring.

  The theorems of `Sound.lean` find, where the evaluation fails, a place that says so as the
  analysis reaches it; this file is what carries that place to the end of the function, through
  every branch, arm and round the analysis goes on with.
-/
import KanameCore.AbsSound

namespace KanameCore

-- ── What a place takes ───────────────────────────────────────────────────────

mutual
theorem Slot.eq_of_beq : ∀ {a b : Slot}, Slot.beq a b = true → a = b
  | .mk l c r u rc fs, .mk l' c' r' u' rc' fs', h => by
    simp only [Slot.beq, Bool.and_eq_true, beq_iff_eq] at h
    obtain ⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩ := h
    subst h1 h2 h3 h4 h5
    rw [Slot.eq_of_beqFields h6]
theorem Slot.eq_of_beqFields : ∀ {xs ys : List (String × Slot)}, Slot.beqFields xs ys = true → xs = ys
  | [], [], _ => rfl
  | (n, s) :: xs, (m, t) :: ys, h => by
    simp only [Slot.beqFields, Bool.and_eq_true, beq_iff_eq] at h
    obtain ⟨⟨h1, h2⟩, h3⟩ := h
    rw [h1, Slot.eq_of_beq h2, Slot.eq_of_beqFields h3]
  | [], _ :: _, h => by simp [Slot.beqFields] at h
  | _ :: _, [], h => by simp [Slot.beqFields] at h
end

theorem What.eq_of_beq {a b : What} (h : a.beq b = true) : a = b := by
  cases a <;> cases b <;> simp [What.beq] at h ⊢ <;> first | exact h | exact Slot.eq_of_beq h

-- ── Growing ──────────────────────────────────────────────────────────────────

/-- A place kept later for one kept now: what it takes, and every value it held. -/
def PlaceLE (p p' : Place) : Prop :=
  p'.kind = p.kind ∧ p'.what = p.what ∧ (∀ v, Mem p.value v → Mem p'.value v) ∧
    (okNum p'.value = true → okNum p.value = true)

theorem PlaceLE.refl (p : Place) : PlaceLE p p := ⟨rfl, rfl, fun _ h => h, fun h => h⟩

theorem PlaceLE.trans {p q r : Place} (h1 : PlaceLE p q) (h2 : PlaceLE q r) : PlaceLE p r :=
  ⟨h2.1.trans h1.1, h2.2.1.trans h1.2.1, fun v h => h2.2.2.1 v (h1.2.2.1 v h), fun h => h1.2.2.2 (h2.2.2.2 h)⟩

/-- Every place kept is of one of the five kinds of the effects. -/
def KindsOK (o : Out) : Prop := ∀ p ∈ o.places, p.kind ∈ kindOrder

/-- What the analysis keeps only grows. -/
structure Grows (o o' : Out) : Prop where
  places : ∀ p ∈ o.places, ∃ p' ∈ o'.places, PlaceLE p p'
  calls : ∀ g ∈ o.calls, g ∈ o'.calls
  widened : o.widened = true → o'.widened = true
  kinds : KindsOK o → KindsOK o'

theorem Grows.refl (o : Out) : Grows o o :=
  ⟨fun p h => ⟨p, h, PlaceLE.refl p⟩, fun _ h => h, fun h => h, fun h => h⟩

theorem Grows.trans {o₁ o₂ o₃ : Out} (h1 : Grows o₁ o₂) (h2 : Grows o₂ o₃) : Grows o₁ o₃ where
  places p hp := by
    obtain ⟨q, hq, hpq⟩ := h1.places p hp
    obtain ⟨r, hr, hqr⟩ := h2.places q hq
    exact ⟨r, hr, hpq.trans hqr⟩
  calls g hg := h2.calls g (h1.calls g hg)
  widened h := h2.widened (h1.widened h)
  kinds h := h2.kinds (h1.kinds h)

/-- The relations a call of a rule keeps change no place, no call, nothing widened. -/
theorem keep_places (o : Out) (e : Nat) (rels : List (String × Rel × String)) : (o.keep e rels).places = o.places := by
  unfold Out.keep; split <;> rfl

theorem keep_calls (o : Out) (e : Nat) (rels : List (String × Rel × String)) : (o.keep e rels).calls = o.calls := by
  unfold Out.keep; split <;> rfl

theorem keep_widened (o : Out) (e : Nat) (rels : List (String × Rel × String)) : (o.keep e rels).widened = o.widened := by
  unfold Out.keep; split <;> rfl

theorem grows_keep {o₀ o : Out} (h : Grows o₀ o) (e : Nat) (rels : List (String × Rel × String)) :
    Grows o₀ (o.keep e rels) where
  places p hp := by rw [keep_places]; exact h.places p hp
  calls g hg := by rw [keep_calls]; exact h.calls g hg
  widened hw := by rw [keep_widened]; exact h.widened hw
  kinds hk := fun p hp => h.kinds hk p (by rw [keep_places] at hp; exact hp)

theorem place_same {p : Place} {e : Nat} {key : String} {kind : Kind} {what : What} (h : p.same e key kind what = true) :
    p.kind = kind ∧ p.what = what := by
  simp only [Place.same, Bool.and_eq_true, beq_iff_eq] at h
  exact ⟨h.1.2, What.eq_of_beq h.2⟩

/-- **A place kept grows**: joined with the new values, or the new place put after. -/
theorem grows_place_self (o : Out) (e : Nat) (key : String) (kind : Kind) (what : What) (v : Abs)
    (hk : kind ∈ kindOrder) : Grows o (o.place e key kind what v) where
  places p hp := by
    unfold Out.place
    split
    · refine ⟨if p.same e key kind what then { p with value := p.value.join v } else p, ?_, ?_⟩
      · exact List.mem_map.mpr ⟨p, hp, rfl⟩
      · split
        · exact ⟨rfl, rfl, fun _ h => mem_join_left h, fun h => okNum_join' h⟩
        · exact PlaceLE.refl p
    · exact ⟨p, List.mem_append_left _ hp, PlaceLE.refl p⟩
  calls g hg := by unfold Out.place; split <;> exact hg
  widened h := by unfold Out.place; split <;> exact h
  kinds h := by
    unfold Out.place
    split
    · intro p hp
      obtain ⟨q, hq, rfl⟩ := List.mem_map.mp hp
      split
      · exact h q hq
      · exact h q hq
    · intro p hp
      rcases List.mem_append.mp hp with hp | hp
      · exact h p hp
      · simp at hp; subst hp; exact hk

/-- The place a value is given to holds it, as what it takes. -/
theorem place_holds (o : Out) (e : Nat) (key : String) (kind : Kind) (what : What) (v : Abs) :
    ∃ p ∈ (o.place e key kind what v).places, p.kind = kind ∧ p.what = what ∧
      (∀ c, Mem v c → Mem p.value c) ∧ (okNum p.value = true → okNum v = true) := by
  unfold Out.place
  split
  · rename_i h
    obtain ⟨q, hq, hs⟩ := List.any_eq_true.mp h
    refine ⟨{ q with value := q.value.join v }, List.mem_map.mpr ⟨q, hq, by simp [hs]⟩, ?_⟩
    obtain ⟨h1, h2⟩ := place_same hs
    exact ⟨h1, h2, fun _ h => mem_join_right h, fun h => (okNum_join D _ _ h).2⟩
  · exact ⟨⟨e, key, kind, what, v⟩, List.mem_append_right _ (by simp), rfl, rfl, fun _ h => h, fun h => h⟩

theorem grows_place {o o₁ : Out} (h : Grows o o₁) {e : Nat} {key : String} {kind : Kind} {what : What} {v : Abs}
    (hk : kind ∈ kindOrder := by decide) : Grows o (o₁.place e key kind what v) :=
  h.trans (grows_place_self o₁ e key kind what v hk)

theorem grows_record {o o₁ : Out} (h : Grows o o₁) {e : Nat} {v : Abs} : Grows o (o₁.record e v) := by
  refine h.trans ⟨fun p hp => ⟨p, ?_, PlaceLE.refl p⟩, fun g hg => ?_, fun hw => ?_, fun hk => ?_⟩ <;>
    unfold Out.record <;> split <;> assumption

theorem grows_fuel {o o₁ : Out} (h : Grows o o₁) : Grows o (outOfFuel o₁) := grows_place h

theorem grows_unknown {o o₁ : Out} (h : Grows o o₁) {e : Nat} : Grows o (unknown o₁ e) := grows_place h

theorem grows_convA {o o₁ : Out} (h : Grows o o₁) {c : Option Conv} {a : Abs} : Grows o (convA o₁ c a).2 := by
  cases c with
  | none => exact h
  | some c =>
    simp only [convA]
    split
    · split
      · exact h
      · exact grows_place h
    · split
      · exact grows_place h
      · exact h

theorem grows_heldA {o o₁ : Out} (h : Grows o o₁) {e : Nat} {a : Abs} : Grows o (heldA o₁ e a).2 := by
  simp only [heldA]
  exact grows_place h

theorem grows_calls {o o₁ : Out} (h : Grows o o₁) {f : Nat} : Grows o { o₁ with calls := o₁.calls ++ [f] } :=
  h.trans ⟨fun p hp => ⟨p, hp, PlaceLE.refl p⟩, fun _ hg => List.mem_append_left _ hg, fun hw => hw, fun hk => hk⟩

theorem grows_widen {o o₁ : Out} (h : Grows o o₁) : Grows o { o₁ with widened := true } :=
  h.trans ⟨fun p hp => ⟨p, hp, PlaceLE.refl p⟩, fun _ hg => hg, fun _ => rfl, fun hk => hk⟩

-- ── The scope keeps its module ───────────────────────────────────────────────

/-- Two scopes of one module, with the same answers of the functions analysed and the same ring. -/
def Same (A A' : ACtx) : Prop := A'.M = A.M ∧ A'.results = A.results ∧ A'.ring = A.ring

theorem Same.refl (A : ACtx) : Same A A := ⟨rfl, rfl, rfl⟩

theorem Same.trans {A B C : ACtx} (h1 : Same A B) (h2 : Same B C) : Same A C :=
  ⟨h2.1.trans h1.1, h2.2.1.trans h1.2.1, h2.2.2.trans h1.2.2⟩

theorem same_bind {A B : ACtx} (h : Same A B) {x : String} {a : Abs} : Same A (B.bind x a) := h
theorem same_bindNew {A B : ACtx} (h : Same A B) {x : String} {a : Abs} : Same A (B.bindNew x a) := h
theorem same_facts {A B : ACtx} (h : Same A B) {fs : List (Sym × Rel × Sym)} : Same A { B with facts := fs } := h

theorem same_narrowTo {A B : ACtx} (h : Same A B) {s : Sym} {a : Abs} : Same A (B.narrowTo s a) := by
  unfold ACtx.narrowTo; split
  · exact h
  · exact h

/-- Closes `Same A X` where `X` is `B` narrowed, named or given facts, step by step. -/
macro "same_close" h:term : tactic => `(tactic| (
  repeat (first
    | apply same_narrowSide
    | apply same_narrowCount
    | apply same_narrowTo
    | apply same_facts
    | apply same_bindNew
    | apply same_bind)
  exact $h))

theorem same_assume {A B : ACtx} (h : Same A B) {x : Expr} {r : Range} {c : Option Conv} : Same A (assume B x r c) := by
  simp only [assume]
  repeat' split
  all_goals first | exact h | exact same_narrowTo h

theorem same_narrowCount {A B : ACtx} (h : Same A B) {x : Expr} {vy : Abs} {op : CmpOp} :
    Same A (narrowCount B x vy op) := by
  simp only [narrowCount]
  repeat' split
  all_goals first | exact h | exact same_narrowTo h

theorem same_narrowSide {A B : ACtx} (h : Same A B) {s : Option Sym} {conv : Option Conv} {me other : Abs} {op : CmpOp}
    {step : Option Rat} : Same A (narrowSide B s conv me other op step) := by
  simp only [narrowSide]
  repeat' split
  all_goals first | exact h | exact same_narrowTo h

theorem same_addFact {A B : ACtx} (h : Same A B) {op : CmpOp} {syA syB : Option Sym} {ca cb : Option Conv}
    {xa xb : Abs} : Same A (addFact B op syA syB ca cb xa xb) := by
  simp only [addFact]
  repeat' split
  all_goals first | exact h | exact same_facts h

theorem same_narrowCounts {A B : ACtx} (h : Same A B) {op : CmpOp} {a b : Expr} {ca cb : Option Conv} {va vb : Abs} :
    Same A (narrowCounts B op a b ca cb va vb) := by
  simp only [narrowCounts]
  split
  · exact same_narrowCount (same_narrowCount h)
  · exact h

theorem same_narrowCmp {A B : ACtx} (h : Same A B) {op : CmpOp} {a b : Expr} {ca cb : Option Conv} {sa sb : Option Rat}
    {va vb : Abs} : Same A (narrowCmp B op a b ca cb sa sb va vb) := by
  simp only [narrowCmp]
  split
  · exact same_narrowCounts h
  · exact same_narrowSide (same_narrowSide (same_addFact (same_narrowCounts h)))

theorem same_apply {A : ACtx} : ∀ (nar : Nar) (t : Bool) (B : ACtx), Same A B → Same A (nar.apply t B)
  | .none, _, B, h => h
  | .sym _, _, B, h => same_narrowTo h
  | .cmp .., _, B, h => same_narrowCmp h
  | .not x, t, B, h => same_apply x _ B h
  | .and x y, t, B, h => by
    simp only [Nar.apply]
    split
    · exact same_apply y _ _ (same_apply x _ _ h)
    · exact h
  | .or x y, t, B, h => by
    simp only [Nar.apply]
    split
    · exact same_apply y _ _ (same_apply x _ _ h)
    · exact h


-- ── The calls and what is built ──────────────────────────────────────────────

theorem foldl_inv {α β : Type} (P : β → Prop) (f : β → α → β) (hf : ∀ b a, P b → P (f b a)) :
    ∀ (l : List α) (b : β), P b → P (l.foldl f b)
  | [], _, h => h
  | a :: l, b, h => foldl_inv P f hf l (f b a) (hf b a h)

theorem callStep_ok {o₀ : Out} {A₀ : ACtx} {fn : Fn} :
    ∀ (acc : Out × ACtx) arg, Grows o₀ acc.1 ∧ Same A₀ acc.2 → Grows o₀ (callStep fn acc arg).1 ∧ Same A₀ (callStep fn acc arg).2 := by
  rintro ⟨o1, A1⟩ ⟨⟨k, x, c⟩, v⟩ ⟨h1, h2⟩
  simp only [callStep]
  split
  · exact ⟨grows_convA h1, h2⟩
  · refine ⟨grows_place (grows_convA h1), ?_⟩
    split
    · split
      · exact same_assume h2
      · exact h2
    · exact h2

theorem callAbs_ok {o₀ o : Out} {A₀ A : ACtx} (ho : Grows o₀ o) (hA : Same A₀ A) {f : Nat}
    {args : List (Nat × Expr × Option Conv)} {vals : List Abs} :
    Grows o₀ (callAbs A o f args vals).2.1 ∧ Same A₀ (callAbs A o f args vals).2.2 := by
  simp only [callAbs]
  split
  · exact ⟨ho, hA⟩
  · rename_i fn _
    have := foldl_inv (fun (acc : Out × ACtx) => Grows o₀ acc.1 ∧ Same A₀ acc.2) (callStep fn) callStep_ok
      (args.zip vals) (o, A) ⟨ho, hA⟩
    exact ⟨grows_calls this.1, this.2⟩

theorem grows_enumPlace {o₀ o : Out} (h : Grows o₀ o) {x : Expr} {inp : RuleInput} {v : Abs} :
    Grows o₀ (enumPlace x inp v o) := by
  unfold enumPlace
  split
  · exact grows_place h
  · exact h

theorem ruleStep_ok {o₀ : Out} {A₀ : ACtx} {rd : RuleD} :
    ∀ (acc : Out × ACtx) arg, Grows o₀ acc.1 ∧ Same A₀ acc.2 → Grows o₀ (ruleStep rd acc arg).1 ∧ Same A₀ (ruleStep rd acc arg).2 := by
  rintro ⟨o1, A1⟩ ⟨⟨k, x, c⟩, v⟩ ⟨h1, h2⟩
  simp only [ruleStep]
  split
  · exact ⟨grows_convA h1, h2⟩
  · split
    · split
      · refine ⟨grows_place (grows_convA h1), ?_⟩
        split
        · exact h2
        · exact same_assume h2
      · split
        · exact ⟨grows_place (grows_convA h1), h2⟩
        · exact ⟨grows_enumPlace (grows_convA h1), h2⟩
    · refine ⟨?_, h2⟩
      refine foldl_inv (fun o' => Grows o₀ o') _ ?_ _ _ (grows_convA h1)
      intro o' col h
      simp only [elemStep]
      split
      · exact grows_place h
      · exact h

theorem ruleAbs_ok {o₀ o : Out} {A₀ A : ACtx} (ho : Grows o₀ o) (hA : Same A₀ A) {id r : Nat}
    {args : List (Nat × Expr × Option Conv)} {vals : List Abs} :
    Grows o₀ (ruleAbs A o id r args vals).2.1 ∧ Same A₀ (ruleAbs A o id r args vals).2.2 := by
  simp only [ruleAbs]
  split
  · exact ⟨ho, hA⟩
  · rename_i rd _
    have := foldl_inv (fun (acc : Out × ACtx) => Grows o₀ acc.1 ∧ Same A₀ acc.2) (ruleStep rd) ruleStep_ok
      (args.zip vals) (o, A) ⟨ho, hA⟩
    exact ⟨grows_keep (foldl_inv (fun o' => Grows o₀ o') _ (fun o' _ h => grows_place h) _ _ this.1) _ _, this.2⟩

theorem dateStep_ok {o₀ : Out} {A₀ : ACtx} {fd : DatesD} {reads : List String} :
    ∀ (acc : Out × ACtx × Option Abs) arg, Grows o₀ acc.1 ∧ Same A₀ acc.2.1 →
      Grows o₀ (dateStep fd reads acc arg).1 ∧ Same A₀ (dateStep fd reads acc arg).2.1 := by
  rintro ⟨o1, A1, g⟩ ⟨⟨k, x, c⟩, v⟩ ⟨h1, h2⟩
  simp only [dateStep]
  split
  · exact ⟨grows_convA h1, h2⟩
  · split
    · exact ⟨grows_place (grows_convA h1), same_assume h2⟩
    · exact ⟨grows_place (grows_convA h1), same_assume h2⟩

theorem dateAbs_ok {o₀ o : Out} {A₀ A : ACtx} (ho : Grows o₀ o) (hA : Same A₀ A) {file : Nat} {date : String}
    {args : List (Nat × Expr × Option Conv)} {vals : List Abs} :
    Grows o₀ (dateAbs A o file date args vals).2.1 ∧ Same A₀ (dateAbs A o file date args vals).2.2 := by
  simp only [dateAbs]
  split
  · exact ⟨ho, hA⟩
  · rename_i fd _
    exact foldl_inv (fun (acc : Out × ACtx × Option Abs) => Grows o₀ acc.1 ∧ Same A₀ acc.2.1) (dateStep fd _) dateStep_ok
      (args.zip vals) (o, A, none) ⟨ho, hA⟩

theorem transferAbs_ok {o₀ o : Out} (ho : Grows o₀ o) {args : List (String × Expr × Option Conv × Nat × Bool)}
    {vals : List Abs} : Grows o₀ (transferAbs o args vals).2 := by
  simp only [transferAbs]
  refine foldl_inv (fun (acc : List (String × Abs) × Out) => Grows o₀ acc.2) _ ?_ (args.zip vals) ([], o) ho
  rintro ⟨out, o1⟩ ⟨⟨p, x, c, l, amount⟩, v⟩ h1
  simp only [transferStep]
  split
  · exact grows_place (grows_convA h1)
  · exact grows_convA h1

theorem fieldStep_ok {o₀ : Out} {A₀ : ACtx} {module : Bool} {given : List (Nat × Abs × Expr × Option Conv)} :
    ∀ (acc : List (String × Abs) × Out × ACtx) fk, Grows o₀ acc.2.1 ∧ Same A₀ acc.2.2 →
      Grows o₀ (fieldStep module given acc fk).2.1 ∧ Same A₀ (fieldStep module given acc fk).2.2 := by
  rintro ⟨out, o1, A1⟩ ⟨⟨f, slot, scalar⟩, k⟩ ⟨g1, g2⟩
  dsimp only at g1 g2
  simp only [fieldStep]
  split
  · split
    · refine ⟨grows_place g1, ?_⟩
      split
      · split
        · exact same_assume g2
        · exact g2
      · exact g2
    · exact ⟨g1, g2⟩
  · exact ⟨g1, g2⟩

theorem recordAbs_ok {o₀ o : Out} {A₀ A : ACtx} (ho : Grows o₀ o) (hA : Same A₀ A) {module : Bool}
    {fields : List (String × Slot × Bool)} {inits : List (Nat × Expr × Option Conv × Nat)} {vals : List Abs} :
    Grows o₀ (recordAbs A o module fields inits vals).2.1 ∧ Same A₀ (recordAbs A o module fields inits vals).2.2 := by
  simp only [recordAbs]
  have h1 := foldl_inv (fun (acc : List (Nat × Abs × Expr × Option Conv) × Out) => Grows o₀ acc.2) initStep
    (fun ⟨given, o1⟩ ⟨⟨k, x, c, l⟩, v⟩ g => by simp only [initStep]; exact grows_convA g) (inits.zip vals) ([], o) ho
  exact foldl_inv (fun (acc : List (String × Abs) × Out × ACtx) => Grows o₀ acc.2.1 ∧ Same A₀ acc.2.2) _ fieldStep_ok
    fields.zipIdx ([], _, A) ⟨h1, hA⟩


theorem callAbs_grows {o₀ o : Out} (ho : Grows o₀ o) {A : ACtx} {f : Nat} {args : List (Nat × Expr × Option Conv)}
    {vals : List Abs} : Grows o₀ (callAbs A o f args vals).2.1 := (callAbs_ok ho (Same.refl A)).1
theorem callAbs_same {A₀ A : ACtx} (hA : Same A₀ A) {o : Out} {f : Nat} {args : List (Nat × Expr × Option Conv)}
    {vals : List Abs} : Same A₀ (callAbs A o f args vals).2.2 := (callAbs_ok (Grows.refl o) hA).2
theorem ruleAbs_grows {o₀ o : Out} (ho : Grows o₀ o) {A : ACtx} {id r : Nat} {args : List (Nat × Expr × Option Conv)}
    {vals : List Abs} : Grows o₀ (ruleAbs A o id r args vals).2.1 := (ruleAbs_ok ho (Same.refl A)).1
theorem ruleAbs_same {A₀ A : ACtx} (hA : Same A₀ A) {o : Out} {id r : Nat} {args : List (Nat × Expr × Option Conv)}
    {vals : List Abs} : Same A₀ (ruleAbs A o id r args vals).2.2 := (ruleAbs_ok (Grows.refl o) hA).2
theorem dateAbs_grows {o₀ o : Out} (ho : Grows o₀ o) {A : ACtx} {file : Nat} {date : String}
    {args : List (Nat × Expr × Option Conv)} {vals : List Abs} : Grows o₀ (dateAbs A o file date args vals).2.1 :=
  (dateAbs_ok ho (Same.refl A)).1
theorem dateAbs_same {A₀ A : ACtx} (hA : Same A₀ A) {o : Out} {file : Nat} {date : String}
    {args : List (Nat × Expr × Option Conv)} {vals : List Abs} : Same A₀ (dateAbs A o file date args vals).2.2 :=
  (dateAbs_ok (Grows.refl o) hA).2
theorem recordAbs_grows {o₀ o : Out} (ho : Grows o₀ o) {A : ACtx} {module : Bool} {fields : List (String × Slot × Bool)}
    {inits : List (Nat × Expr × Option Conv × Nat)} {vals : List Abs} :
    Grows o₀ (recordAbs A o module fields inits vals).2.1 := (recordAbs_ok ho (Same.refl A)).1
theorem recordAbs_same {A₀ A : ACtx} (hA : Same A₀ A) {o : Out} {module : Bool} {fields : List (String × Slot × Bool)}
    {inits : List (Nat × Expr × Option Conv × Nat)} {vals : List Abs} :
    Same A₀ (recordAbs A o module fields inits vals).2.2 := (recordAbs_ok (Grows.refl o) hA).2

theorem lamResult_grows {o₀ o : Out} (h : Grows o₀ o) {id : Nat} {w : Walk} {count : Iv} {elem v : Abs}
    {step : Option Rat} : Grows o₀ (lamResult o id w count elem v step).2 := by
  unfold lamResult
  split
  · exact h
  · exact h
  · exact h
  · exact grows_heldA (grows_place h)
  · exact grows_place h

-- ── Every function of the analysis ───────────────────────────────────────────

/-- What every function of the analysis keeps, at one number of steps. -/
structure SAt (m : Nat) : Prop where
  ev : ∀ A e o, Grows o (ev A m e o).out ∧ Same A (ev A m e o).ctx
  evI : ∀ A e o, Grows o (evI A m e o).out ∧ Same A (evI A m e o).ctx
  branch : ∀ A b c o, Grows o (branch A m b c o).2
  andOr : ∀ A isAnd a b o, Grows o (andOr A m isAnd a b o).out ∧ Same A (andOr A m isAnd a b o).ctx
  arms : ∀ A sy sv arms out o, Grows o (matchArms A m sy sv arms out o).2
  args : ∀ A args o, Grows o (evArgs A m args o).2.1 ∧ Same A (evArgs A m args o).2.2
  args5 : ∀ A args o, Grows o (evArgs5 A m args o).2.1 ∧ Same A (evArgs5 A m args o).2.2
  inits : ∀ A inits o, Grows o (evInits A m inits o).2.1 ∧ Same A (evInits A m inits o).2.2
  list : ∀ A args o, Grows o (evList A m args o).2.1 ∧ Same A (evList A m args o).2.2
  elems : ∀ A elems elem k o, Grows o (listAbs A m elems elem k o).out ∧ Same A (listAbs A m elems elem k o).ctx
  lam1 : ∀ A x elem body c o, Grows o (lam1 A m x elem body c o).2
  cond : ∀ A x elem body c o, Grows o (condition A m x elem body c o).2
  walk : ∀ A id w list x body c ord step o,
    Grows o (walkAbs A m id w list x body c ord step o).out ∧ Same A (walkAbs A m id w list x body c ord step o).ctx
  reduce : ∀ A list first acc x body c out o,
    Grows o (reduceAbs A m list first acc x body c out o).out ∧ Same A (reduceAbs A m list first acc x body c out o).ctx
  rounds : ∀ A acc x body c elem k a o, Grows o (reduceRounds A m acc x body c elem k a o).2
  lam2 : ∀ A acc x a elem body c o, Grows o (lam2 A m acc x a elem body c o).2

theorem sat_zero : SAt 0 where
  ev A _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  evI A _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  branch _ _ _ o := grows_fuel (Grows.refl o)
  andOr A _ _ _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  arms _ _ _ _ _ o := grows_fuel (Grows.refl o)
  args A _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  args5 A _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  inits A _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  list A _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  elems A _ _ _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  lam1 _ _ _ _ _ o := grows_fuel (Grows.refl o)
  cond _ _ _ _ _ o := grows_fuel (Grows.refl o)
  walk A _ _ _ _ _ _ _ _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  reduce A _ _ _ _ _ _ _ o := ⟨grows_fuel (Grows.refl o), Same.refl A⟩
  rounds _ _ _ _ _ _ _ _ o := grows_fuel (Grows.refl o)
  lam2 _ _ _ _ _ _ _ o := grows_fuel (Grows.refl o)

set_option hygiene false in
/-- Closes `Grows o X` and `Same A X` where `X` is what the analysis goes on with, step by step back
    to `o` and `A`, through the primitives and the functions at one fewer step (`hev`, …, in
    scope). -/
macro "gclose" : tactic => `(tactic| (repeat (first
  | exact Grows.refl _
  | exact Same.refl _
  | with_reducible apply grows_record
  | with_reducible apply grows_fuel
  | with_reducible apply grows_unknown
  | with_reducible apply grows_heldA
  | with_reducible apply grows_convA
  | with_reducible apply grows_calls
  | with_reducible apply grows_widen
  | with_reducible apply callAbs_grows
  | with_reducible apply ruleAbs_grows
  | with_reducible apply dateAbs_grows
  | with_reducible apply transferAbs_ok
  | with_reducible apply recordAbs_grows
  | with_reducible apply callAbs_same
  | with_reducible apply ruleAbs_same
  | with_reducible apply dateAbs_same
  | with_reducible apply recordAbs_same
  | with_reducible apply lamResult_grows
  | with_reducible apply same_assume
  | with_reducible apply same_bindNew
  | with_reducible apply same_apply
  | with_reducible apply grows_place
  | (show _ ∈ kindOrder; decide)
  | refine hev ?_ | refine hevS ?_ | refine hevI ?_ | refine hevIS ?_ | refine hbr ?_ | refine hao ?_ | refine haoS ?_
  | refine harms ?_ | refine hargs ?_ | refine hargsS ?_ | refine hargs5 ?_ | refine hargs5S ?_ | refine hinits ?_
  | refine hinitsS ?_ | refine hlist ?_ | refine hlistS ?_ | refine helems ?_ | refine helemsS ?_ | refine hlam1 ?_
  | refine hcond ?_ | refine hwalk ?_ | refine hwalkS ?_ | refine hred ?_ | refine hredS ?_ | refine hrounds ?_
  | refine hlam2 ?_)))

set_option maxHeartbeats 4000000 in
theorem sat_succ {m : Nat} (S : SAt m) : SAt (m + 1) := by
  have hev : ∀ {o₀ o A e}, Grows o₀ o → Grows o₀ (ev A m e o).out := fun h => h.trans (S.ev _ _ _).1
  have hevS : ∀ {A₀ A e o}, Same A₀ A → Same A₀ (ev A m e o).ctx := fun h => h.trans (S.ev _ _ _).2
  have hevI : ∀ {o₀ o A e}, Grows o₀ o → Grows o₀ (evI A m e o).out := fun h => h.trans (S.evI _ _ _).1
  have hevIS : ∀ {A₀ A e o}, Same A₀ A → Same A₀ (evI A m e o).ctx := fun h => h.trans (S.evI _ _ _).2
  have hbr : ∀ {o₀ o A b c}, Grows o₀ o → Grows o₀ (branch A m b c o).2 := fun h => h.trans (S.branch _ _ _ _)
  have hao : ∀ {o₀ o A isAnd a b}, Grows o₀ o → Grows o₀ (andOr A m isAnd a b o).out := fun h => h.trans (S.andOr _ _ _ _ _).1
  have haoS : ∀ {A₀ A isAnd a b o}, Same A₀ A → Same A₀ (andOr A m isAnd a b o).ctx := fun h => h.trans (S.andOr _ _ _ _ _).2
  have harms : ∀ {o₀ o A sy sv arms out}, Grows o₀ o → Grows o₀ (matchArms A m sy sv arms out o).2 :=
    fun h => h.trans (S.arms _ _ _ _ _ _)
  have hargs : ∀ {o₀ o A args}, Grows o₀ o → Grows o₀ (evArgs A m args o).2.1 := fun h => h.trans (S.args _ _ _).1
  have hargsS : ∀ {A₀ A args o}, Same A₀ A → Same A₀ (evArgs A m args o).2.2 := fun h => h.trans (S.args _ _ _).2
  have hargs5 : ∀ {o₀ o A args}, Grows o₀ o → Grows o₀ (evArgs5 A m args o).2.1 := fun h => h.trans (S.args5 _ _ _).1
  have hargs5S : ∀ {A₀ A args o}, Same A₀ A → Same A₀ (evArgs5 A m args o).2.2 := fun h => h.trans (S.args5 _ _ _).2
  have hinits : ∀ {o₀ o A inits}, Grows o₀ o → Grows o₀ (evInits A m inits o).2.1 := fun h => h.trans (S.inits _ _ _).1
  have hinitsS : ∀ {A₀ A inits o}, Same A₀ A → Same A₀ (evInits A m inits o).2.2 := fun h => h.trans (S.inits _ _ _).2
  have hlist : ∀ {o₀ o A args}, Grows o₀ o → Grows o₀ (evList A m args o).2.1 := fun h => h.trans (S.list _ _ _).1
  have hlistS : ∀ {A₀ A args o}, Same A₀ A → Same A₀ (evList A m args o).2.2 := fun h => h.trans (S.list _ _ _).2
  have helems : ∀ {o₀ o A elems elem k}, Grows o₀ o → Grows o₀ (listAbs A m elems elem k o).out :=
    fun h => h.trans (S.elems _ _ _ _ _).1
  have helemsS : ∀ {A₀ A elems elem k o}, Same A₀ A → Same A₀ (listAbs A m elems elem k o).ctx :=
    fun h => h.trans (S.elems _ _ _ _ _).2
  have hlam1 : ∀ {o₀ o A x elem body c}, Grows o₀ o → Grows o₀ (lam1 A m x elem body c o).2 :=
    fun h => h.trans (S.lam1 _ _ _ _ _ _)
  have hcond : ∀ {o₀ o A x elem body c}, Grows o₀ o → Grows o₀ (condition A m x elem body c o).2 :=
    fun h => h.trans (S.cond _ _ _ _ _ _)
  have hwalk : ∀ {o₀ o A id w list x body c ord step}, Grows o₀ o → Grows o₀ (walkAbs A m id w list x body c ord step o).out :=
    fun h => h.trans (S.walk _ _ _ _ _ _ _ _ _ _).1
  have hwalkS : ∀ {A₀ A id w list x body c ord step o}, Same A₀ A → Same A₀ (walkAbs A m id w list x body c ord step o).ctx :=
    fun h => h.trans (S.walk _ _ _ _ _ _ _ _ _ _).2
  have hred : ∀ {o₀ o A list first acc x body c out}, Grows o₀ o → Grows o₀ (reduceAbs A m list first acc x body c out o).out :=
    fun h => h.trans (S.reduce _ _ _ _ _ _ _ _ _).1
  have hredS : ∀ {A₀ A list first acc x body c out o}, Same A₀ A → Same A₀ (reduceAbs A m list first acc x body c out o).ctx :=
    fun h => h.trans (S.reduce _ _ _ _ _ _ _ _ _).2
  have hrounds : ∀ {o₀ o A acc x body c elem k a}, Grows o₀ o → Grows o₀ (reduceRounds A m acc x body c elem k a o).2 :=
    fun h => h.trans (S.rounds _ _ _ _ _ _ _ _ _)
  have hlam2 : ∀ {o₀ o A acc x a elem body c}, Grows o₀ o → Grows o₀ (lam2 A m acc x a elem body c o).2 :=
    fun h => h.trans (S.lam2 _ _ _ _ _ _ _ _)
  constructor
  · -- ev
    intro A e o
    cases e <;> simp only [ev]
    case letIn x v c body =>
      split <;> simp only [AR.of] <;> refine ⟨?_, ?_⟩ <;> gclose
    all_goals exact ⟨grows_record (S.evI _ _ _).1, (S.evI _ _ _).2⟩
  · -- evI
    intro A e o
    cases e <;> simp only [evI] <;> (repeat' split) <;> (try simp only [AR.of]) <;> refine ⟨?_, ?_⟩ <;> gclose
  · -- branch
    intro A b c o
    simp only [branch]
    gclose
  · -- andOr
    intro A isAnd a b o
    simp only [andOr]
    repeat' split
    all_goals (try simp only [])
    all_goals refine ⟨?_, ?_⟩ <;> gclose
  · -- matchArms
    intro A sy sv arms out o
    cases arms with
    | nil => simp only [matchArms]; gclose
    | cons arm rest =>
      obtain ⟨ps, body, c⟩ := arm
      simp only [matchArms]
      repeat' split
      all_goals gclose
  · -- evArgs
    intro A args o
    cases args with
    | nil => simp only [evArgs]; exact ⟨Grows.refl o, Same.refl A⟩
    | cons a rest =>
      obtain ⟨k, x, c⟩ := a
      simp only [evArgs]
      repeat' split
      all_goals refine ⟨?_, ?_⟩ <;> gclose
  · -- evArgs5
    intro A args o
    cases args with
    | nil => simp only [evArgs5]; exact ⟨Grows.refl o, Same.refl A⟩
    | cons a rest =>
      obtain ⟨k, x, c⟩ := a
      simp only [evArgs5]
      repeat' split
      all_goals refine ⟨?_, ?_⟩ <;> gclose
  · -- evInits
    intro A inits o
    cases inits with
    | nil => simp only [evInits]; exact ⟨Grows.refl o, Same.refl A⟩
    | cons a rest =>
      obtain ⟨k, x, c⟩ := a
      simp only [evInits]
      repeat' split
      all_goals refine ⟨?_, ?_⟩ <;> gclose
  · -- evList
    intro A args o
    cases args with
    | nil => simp only [evList]; exact ⟨Grows.refl o, Same.refl A⟩
    | cons x rest =>
      simp only [evList]
      repeat' split
      all_goals refine ⟨?_, ?_⟩ <;> gclose
  · -- listAbs
    intro A elems elem k o
    cases elems with
    | nil => simp only [listAbs, AR.of]; exact ⟨Grows.refl o, Same.refl A⟩
    | cons xc rest =>
      obtain ⟨x, c⟩ := xc
      simp only [listAbs]
      repeat' split
      all_goals (try simp only [AR.of])
      all_goals refine ⟨?_, ?_⟩ <;> gclose
  · -- lam1
    intro A x elem body c o
    simp only [lam1]
    gclose
  · -- condition
    intro A x elem body c o
    simp only [condition]
    gclose
  · -- walkAbs
    intro A id w list x body c ord step o
    simp only [walkAbs]
    repeat' split
    all_goals (try simp only [AR.of])
    all_goals refine ⟨?_, ?_⟩ <;> gclose
  · -- reduceAbs
    intro A list first acc x body c out o
    simp only [reduceAbs]
    repeat' split
    all_goals (try simp only [AR.of])
    all_goals refine ⟨?_, ?_⟩ <;> gclose
  · -- reduceRounds
    intro A acc x body c elem k a o
    cases k with
    | zero => simp only [reduceRounds]; gclose
    | succ k =>
      simp only [reduceRounds]
      repeat' split
      all_goals gclose
  · -- lam2
    intro A acc x a elem body c o
    simp only [lam2]
    gclose

theorem sat (m : Nat) : SAt m := by
  induction m with
  | zero => exact sat_zero
  | succ m ih => exact sat_succ ih

end KanameCore
