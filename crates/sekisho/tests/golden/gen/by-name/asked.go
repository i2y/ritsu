// Authorize asks Cedar (cedar-go, in this process) whether the action named action is allowed,
// through the action's own function (AuthorizeRefund for refund). action is the action's name in
// Cedar, and resources holds the id of a resource by the name of its type in Cedar; the ids of
// types the action does not take are left alone. A name the gate has no action of (the kind
// action), an action that takes inputs of its own (input: ask it through its own function), and no
// id, or ids of two types, of the action's resource (resource) are a deny, with an *Error.
func Authorize(store Store, p Principal, action string, resources map[string]string, now time.Time) Answer {
	switch action {
	case "refund":
		return Answer{Policies: []string{}, Err: &Error{"input", "refund takes inputs of its own (amount, kind, on, note): ask it through AuthorizeRefund"}}
	case "review":
		return Answer{Policies: []string{}, Err: &Error{"input", "review takes inputs of its own (cost, memo): ask it through AuthorizeReview"}}
	case "escalate":
		return Answer{Policies: []string{}, Err: &Error{"input", "escalate takes inputs of its own (cost, memo): ask it through AuthorizeEscalate"}}
	case "ping":
		t, err := given(resources, []string{"Order", "Batch"}, "resources holds no id of the resource of ping (Order, Batch)", "resources holds ids of more than one type of the resource of ping (Order, Batch): give the one it is asked of")
		if err != nil {
			return Answer{Policies: []string{}, Err: err}
		}
		return AuthorizePing(store, p, PingInput{Id: resources[t], ResourceType: t}, now)
	}
	return Answer{Policies: []string{}, Err: &Error{"action", fmt.Sprintf("the gate has no action %q", action)}}
}

// given is the one type of types, the types of the resource of an action that takes several, that
// resources holds an id of. None, or more than one, is an *Error of the kind resource, saying none
// or two.
func given(resources map[string]string, types []string, none, two string) (string, *Error) {
	found := ""
	for _, t := range types {
		if _, ok := resources[t]; ok {
			if found != "" {
				return "", &Error{"resource", two}
			}
			found = t
		}
	}
	if found == "" {
		return "", &Error{"resource", none}
	}
	return found, nil
}

