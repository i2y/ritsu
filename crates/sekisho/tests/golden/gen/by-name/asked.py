def authorize(store: Store, principal: Principal, action: str, resources: dict[str, str], now: datetime | None = None) -> Answer:
    """Asks Cedar (cedarpy, in this process) whether the action named action is allowed, through the action's own function (authorize_refund for refund). action is the action's name in Cedar, and resources holds the id of a resource by the name of its type in Cedar; the ids of types the action does not take are left alone. A name the gate has no action of (the kind action), an action that takes inputs of its own (input: ask it through its own function), and no id, or ids of two types, of the action's resource (resource) are a deny, with a SekishoError. A failure of the store is raised as it is."""
    if action == 'refund':
        return Answer(False, [], None, SekishoError('input', 'refund takes inputs of its own (amount, kind, on, note): ask it through authorize_refund'))
    if action == 'review':
        return Answer(False, [], None, SekishoError('input', 'review takes inputs of its own (cost, memo): ask it through authorize_review'))
    if action == 'escalate':
        return Answer(False, [], None, SekishoError('input', 'escalate takes inputs of its own (cost, memo): ask it through authorize_escalate'))
    if action == 'ping':
        t = _given(resources, ('Order', 'Batch'), 'resources holds no id of the resource of ping (Order, Batch)', 'resources holds ids of more than one type of the resource of ping (Order, Batch): give the one it is asked of')
        if isinstance(t, SekishoError):
            return Answer(False, [], None, t)
        if t == 'Order':
            return authorize_ping(store, principal, {'id': resources[t], 'resource_type': 'Order'}, now)
        return authorize_ping(store, principal, {'id': resources[t], 'resource_type': 'Batch'}, now)
    return Answer(False, [], None, SekishoError('action', f'the gate has no action {json.dumps(action, ensure_ascii=False)}'))

def _given(resources: dict[str, str], types: tuple[str, ...], none: str, two: str) -> str | SekishoError:
    """The one type of types, the types of the resource of an action that takes several, that resources holds an id of. None, or more than one, is a SekishoError of the kind resource, saying none or two."""
    found = [x for x in types if x in resources]
    if len(found) == 1:
        return found[0]
    return SekishoError('resource', two if found else none)

