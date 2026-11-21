// Authorize asks Cedar (cedar-go, in this process) whether the action named action is allowed,
// through the action's own function (AuthorizeViewOrder for view_order). action is the action's
// name in Cedar, and resources holds the id of a resource by the name of its type in Cedar; the
// ids of types the action does not take are left alone. A name the gate has no action of (the kind
// action), an action that takes inputs of its own (input: ask it through its own function), and no
// id, or ids of two types, of the action's resource (resource) are a deny, with an *Error.
func Authorize(store Store, p Principal, action string, resources map[string]string, now time.Time) Answer {
	switch action {
	case "view_order":
		id, ok := resources["Order"]
		if !ok {
			return Answer{Policies: []string{}, Err: &Error{"resource", "resources holds no id of the resource of view_order (Order)"}}
		}
		return AuthorizeViewOrder(store, p, ViewOrderInput{OrderId: id}, now)
	case "refund_order":
		return Answer{Policies: []string{}, Err: &Error{"input", "refund_order takes inputs of its own (amount): ask it through AuthorizeRefundOrder"}}
	case "export_refunds":
		id, ok := resources["RefundRecord"]
		if !ok {
			return Answer{Policies: []string{}, Err: &Error{"resource", "resources holds no id of the resource of export_refunds (RefundRecord)"}}
		}
		return AuthorizeExportRefunds(store, p, ExportRefundsInput{Resource: id}, now)
	}
	return Answer{Policies: []string{}, Err: &Error{"action", fmt.Sprintf("the gate has no action %q", action)}}
}

