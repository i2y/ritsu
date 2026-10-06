// Who may refund an order, and how much.

@id("clerks_refund_within_their_limit")
permit (
  principal in Shop::Role::"clerk",
  action == Shop::Action::"refund_order",
  resource
)
when { context.amount <= principal.refund_limit };

@id("managers_refund_any_amount")
permit (
  principal in Shop::Role::"manager",
  action == Shop::Action::"refund_order",
  resource
);

@id("suspended_staff_do_nothing")
forbid (principal, action, resource)
when { principal has suspended && principal.suspended };
