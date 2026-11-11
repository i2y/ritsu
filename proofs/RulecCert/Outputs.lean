/-
  What an output answers (rulec's §7.2, §15.268, §15.269).

  Every numeric output rounds to a grid its line declares (E104), and the rule answers what decides
  the output put on that grid the way the output's mode rounds: its `result`, a `define` of its
  name, what the row of a table that fires writes into it, what a walk leaves behind, an open value.
  The certificate states the grid of each numeric output (`rounds`), its mode (`round_modes`, rulec's
  §15.303) and the range the other languages read the rule's answer in (`output_ranges`). So the
  claim is that the answer lies in that range, rounded by that mode — or under every rounding
  `RoundsWell` describes, where a certificate from before it stated the modes names none.

  The check is one comparison: the range holds what decides the output, as `interval` and `hullOf`
  work it out, with its two ends put on the grid as `interval` puts a rounding's (`Expr.roundTo`):
  rounded the way the mode rounds them, a mode never turning two values around, or out to the grid
  where no mode is named. No new arithmetic.

  Each theorem is of one output, the grid `g` it rounds to, the one `rounds` gives it, and the mode
  `mo`, the one `round_modes` names (`none` where it names none). A range for an output with no grid
  bounds an answer the certificate does not say how to round, so there is no claim to settle:
  `checkRounds` goes through every output `output_ranges` states a range for and refuses one that
  `rounds` gives no grid, or that `round_modes` gives a mode this checker does not know.
-/
import RulecCert.Values

namespace RulecCert

/-- The range `S` holds the span `J` put on the grid `g`, a grid above zero, by the mode `mo`: its
    two ends rounded the way the mode rounds them, or out to the grid where no mode is named. -/
def roundedWithin (mo : Option RoundMode) (g : Rat) (S J : Span2) : Bool :=
  decide (0 < g) && decide (S.1 ≤ roundLo mo g J.1) && decide (roundHi mo g J.2 ≤ S.2)

/-- **A value inside `J`, put on the grid by the mode, lies in a range that holds `J`'s two ends put
    on that grid by it**, and where no mode is named, whichever way the rounding goes. What a walk
    leaves behind and an open value are this with `J` the range their own checks hold them to
    (`extreme_mem_stated`, `foldAnswer_mem_stated`, `openValue_mem_stated`): the range then has to
    sit on the grid. -/
theorem rounded_mem_stated {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd) {mo : Option RoundMode}
    {g : Rat} {S J : Span2} (h : roundedWithin mo g S J = true) {v : Rat} (hv : inSpan J v) :
    inSpan S (rndBy mo rnd g v) := by
  simp only [roundedWithin, Bool.and_eq_true, decide_eq_true_eq] at h
  obtain ⟨⟨hg, h1⟩, h2⟩ := h
  obtain ⟨hr1, hr2⟩ := rndBy_between hrnd mo hg hv.1 hv.2
  exact ⟨Rat.le_trans h1 hr1, Rat.le_trans hr2 h2⟩

/-- **The claim of an output a `result` or a `define` decides**: wherever every name with a range is
    inside it and every `constraint` holds, the answer — the value of the expression `e` put on the
    grid `g` by the mode `mo`, or by any rounding where none is named — lies in the range `S` the
    certificate states for it. -/
def OutputHolds (ranges : String → Option Span2) (within : Expr → Expr → Bool) (mo : Option RoundMode)
    (g : Rat) (S : Span2) (e : Expr) : Prop :=
  ∀ (rnd : Rat → Rat → Rat) (env : String → Option UVal), RoundsWell rnd →
    (∀ n I w t, ranges n = some I → env n = some (w, t) → inSpan I w) →
    (∀ a b va vb ta tb, within a b = true →
      eval rnd env a = some (va, ta) → eval rnd env b = some (vb, tb) → va ≤ vb) →
    ∀ (v : Rat) (t : Ty), eval rnd env e = some (v, t) → inSpan S (rndBy mo rnd g v)

/-- **A range that holds the expression's interval put on the grid by the mode settles the claim.** -/
theorem output_mem_stated {ranges : String → Option Span2} {within : Expr → Expr → Bool} {e : Expr}
    {I : Span2} (hI : interval ranges within e = some I) {mo : Option RoundMode} {g : Rat} {S : Span2}
    (h : roundedWithin mo g S I = true) : OutputHolds ranges within mo g S e :=
  fun _ _ hrnd henv hwithin v t hv =>
    rounded_mem_stated hrnd h (eval_mem_interval hrnd henv hwithin e I v t hI hv)

/-- **The claim of an output a table decides**: whatever a row writes into it where the row fires,
    put on the grid `g` by the mode `mo`, lies in the range `S` the certificate states for it —
    "where it fires" being what the row's own cut says, as in `eval_mem_hull`. -/
def WrittenOutputHolds (ranges : String → Option Span2) (within : Expr → Expr → Bool)
    (mo : Option RoundMode) (g : Rat) (S : Span2) (ws : List Writes) : Prop :=
  ∀ (rnd : Rat → Rat → Rat) (env : String → Option UVal), RoundsWell rnd →
    (∀ n I w t, ranges n = some I → env n = some (w, t) → inSpan I w) →
    (∀ a b va vb ta tb, within a b = true →
      eval rnd env a = some (va, ta) → eval rnd env b = some (vb, tb) → va ≤ vb) →
    ∀ w ∈ ws, ∀ (v : Rat) (t : Ty), eval rnd env w.expr = some (v, t) →
      (∀ J, w.cut = some J → inIval J v) → inSpan S (rndBy mo rnd g v)

/-- **A range that holds the hull of what the rows write, put on the grid by the mode, settles it.** -/
theorem written_output_mem_stated {ranges : String → Option Span2} {within : Expr → Expr → Bool}
    {ws : List Writes} {H : Span2} (hH : hullOf ranges within ws = some H) {mo : Option RoundMode}
    {g : Rat} {S : Span2} (h : roundedWithin mo g S H = true) : WrittenOutputHolds ranges within mo g S ws :=
  fun _ _ hrnd henv hwithin w hw v t hv hcut =>
    rounded_mem_stated hrnd h (eval_mem_hull hrnd henv hwithin ws H hH w hw v t hv hcut)

end RulecCert
