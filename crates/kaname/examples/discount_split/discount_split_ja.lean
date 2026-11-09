import Kaname.discount_split_ja

namespace Proofs.discount_split_ja

open Kaname Kaname.discount_split_ja

/-- 配分は途中の合計一つから一つずつ作り、途中の合計は明細を順に保つ（`runningTotals_items`）。
規則の答えが決めるのは額だけで、項目には触れない。 -/
theorem every_line_keeps_its_label : Kaname.discount_split_ja.every_line_keeps_its_label := by
  intro inner _ lines discount _ _ _ _ _
  conv => rhs; rw [← runningTotals_items lines (fun (l : Line) => l.amount)]
  simp [split, List.map_map, Function.comp_def]

end Proofs.discount_split_ja
