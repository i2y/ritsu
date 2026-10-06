// A policy set written by hand for a test: what sekisho generates of examples/refunds/refunds.gate,
// copied as a person who keeps Cedar by hand would keep it.
@id("refunds/staff_view_orders")
@doc("The staff look at any order")
permit (
  principal,
  action == Shop::Action::"view_order",
  resource is Shop::Order
)
when { principal in Shop::Role::"clerk" || principal in Shop::Role::"auditor" };

@id("refunds/customers_view_their_orders")
@doc("A customer looks at their own orders")
permit (
  principal is Shop::Customer,
  action == Shop::Action::"view_order",
  resource is Shop::Order
)
when { resource.customer == principal };

@id("refunds/clerks_refund_within_their_limit")
@doc("A clerk refunds up to the clerk's own limit, while the refund period lasts")
permit (
  principal in Shop::Role::"clerk",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { context has refund_band && context.refund_band == "within_limit" }
when { context.in_period };

@id("refunds/managers_refund_in_period")
@doc("A manager refunds any amount while the refund period lasts")
permit (
  principal in Shop::Role::"manager",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { context.in_period };

@id("refunds/managers_refund_late_on_business_days")
@doc("After the refund period, a manager may still refund, on a business day in England and Wales")
permit (
  principal in Shop::Role::"manager",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
unless { context.in_period }
when { context.business_day };

@id("refunds/returns_refunds_returned_orders")
@doc("The returns workflow refunds a returned order, up to 50 pounds")
permit (
  principal == Shop::Workflow::"returns",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { resource.status == "returned" }
when { context.amount <= 50 };

@id("refunds/auditors_export_refunds")
@doc("An auditor exports the record of refunds")
permit (
  principal in Shop::Role::"auditor",
  action == Shop::Action::"export_refunds",
  resource is Shop::RefundRecord
);

@id("refunds/no_second_refund")
@doc("An order already refunded is not refunded again")
forbid (
  principal,
  action == Shop::Action::"refund_order",
  resource is Shop::Order
)
when { resource.status == "refunded" };

@id("refunds/auditors_do_not_refund")
@doc("Whoever audits the refunds does not refund")
forbid (
  principal in Shop::Role::"auditor",
  action == Shop::Action::"refund_order",
  resource is Shop::Order
);

@id("refunds/suspended_staff_do_nothing")
@doc("A suspended member of the staff does nothing")
forbid (
  principal is Shop::User,
  action in
    [Shop::Action::"view_order",
     Shop::Action::"refund_order",
     Shop::Action::"export_refunds"],
  resource
)
when { principal.suspended };
