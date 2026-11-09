import Kaname.order_flow_ja

namespace Proofs.order_flow_ja

open Kaname Kaname.order_flow_ja

/-- 状態ごとに場合を分ける。受け付ける状態は `when` の行と矛盾し、閉じた状態はどのイベントも
拒む。 -/
theorem a_closed_order_takes_no_event : Kaname.order_flow_ja.a_closed_order_takes_no_event := by
  intro order event _ _ closed
  cases order <;> simp_all [«open», next]

end Proofs.order_flow_ja
