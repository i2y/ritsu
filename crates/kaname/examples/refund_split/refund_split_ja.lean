import Kaname.refund_split_ja

namespace Proofs.refund_split_ja

open Kaname Kaname.refund_split_ja

/-- 配分は、その明細の終わりまでの返金から手前までの返金を引いたものなので、足すと途中が打ち消し
合い、全体の返金になる。 -/
theorem shares_add_up : Kaname.refund_split_ja.shares_add_up := by
  intro lines refund hlen _ hwithin _ _
  obtain ⟨S, hS⟩ : ∃ S, S = (lines.map fun l => l.left).sum := ⟨_, rfl⟩
  have hT : 0 < S := hS ▸ sum_pos hlen fun l hl => (hwithin l hl).1
  have key : (split lines refund).map (fun s => s.share)
      = (runningTotals lines (fun l => l.left)).map (fun r => allocate refund r.upto S - allocate refund r.before S) := by
    unfold split
    rw [List.map_map]
    apply List.map_congr_left
    intro r hr
    simp only [Function.comp, runningTotals_total _ _ r hr, hS]
  rw [key, runningTotals_telescope lines (fun l => l.left) (fun x => allocate refund x S), ← hS]
  rw [allocate_self refund hT, allocate_zero]
  omega

/-- 配分は、その明細の終わりまでの返金から手前までの返金を引いたもので、`allocate` は持ち分が
増えても減らない。 -/
theorem no_share_is_negative : Kaname.refund_split_ja.no_share_is_negative := by
  intro lines refund hlen _ hwithin hr0 _ s hs
  have hT : 0 < (lines.map fun l => l.left).sum := sum_pos hlen fun l hl => (hwithin l hl).1
  unfold split at hs
  obtain ⟨r, hr, rfl⟩ := List.mem_map.mp hs
  have hitem : r.item ∈ lines := by
    have := List.mem_map_of_mem (f := (·.item)) hr
    rwa [runningTotals_items] at this
  have hleft := (hwithin r.item hitem).1
  have hw : ∀ l ∈ lines, 0 ≤ l.left := fun l hl => by have := (hwithin l hl).1; omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hmono := allocate_mono (t := refund) hr0 (htot ▸ hT) (show r.before ≤ r.upto by omega)
  simp only
  omega

/-- 返金が全体を超えなければ、一つの明細が受け取る額は、全体のうちのその明細の幅を超えない。 -/
theorem no_share_is_more_than_its_left : Kaname.refund_split_ja.no_share_is_more_than_its_left := by
  intro lines refund hlen _ hwithin hr0 _ hle s hs
  have hT : 0 < (lines.map fun l => l.left).sum := sum_pos hlen fun l hl => (hwithin l hl).1
  unfold split at hs
  obtain ⟨r, hr, rfl⟩ := List.mem_map.mp hs
  have hitem : r.item ∈ lines := by
    have := List.mem_map_of_mem (f := (·.item)) hr
    rwa [runningTotals_items] at this
  have hleft := (hwithin r.item hitem).1
  have hw : ∀ l ∈ lines, 0 ≤ l.left := fun l hl => by have := (hwithin l hl).1; omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hsub := allocate_sub_le (t := refund) hr0 (htot ▸ hle) (htot ▸ hT) (show r.before ≤ r.upto by omega)
  simp only
  omega

end Proofs.refund_split_ja
