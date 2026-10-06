// だれが注文を返金できるか、いくらまでか。

@id("係は自分の上限まで返金できる")
permit (
  principal in Shop::Role::"clerk",
  action == Shop::Action::"返金",
  resource
)
when { context.amount <= principal.refund_limit };

@id("店長はいくらでも返金できる")
permit (
  principal in Shop::Role::"manager",
  action == Shop::Action::"返金",
  resource
);

@id("停止中の人は何もできない")
forbid (principal, action, resource)
when { principal has suspended && principal.suspended };
