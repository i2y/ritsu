def authorize(store: Store, principal: Principal, action: str, resources: dict[str, str], now: datetime | None = None) -> Answer:
    """Asks Cedar (cedarpy, in this process) whether the action named action is allowed, through the action's own function (authorize_view_order for view_order). action is the action's name in Cedar, and resources holds the id of a resource by the name of its type in Cedar; the ids of types the action does not take are left alone. A name the gate has no action of (the kind action), an action that takes inputs of its own (input: ask it through its own function), and no id, or ids of two types, of the action's resource (resource) are a deny, with a SekishoError. A failure of the store is raised as it is."""
    if action == 'view_order':
        if 'Order' not in resources:
            return Answer(False, [], None, SekishoError('resource', 'resources holds no id of the resource of view_order (Order)'))
        return authorize_view_order(store, principal, {'orderId': resources['Order']}, now)
    if action == 'refund_order':
        return Answer(False, [], None, SekishoError('input', 'refund_order takes inputs of its own (amount): ask it through authorize_refund_order'))
    if action == 'export_refunds':
        if 'RefundRecord' not in resources:
            return Answer(False, [], None, SekishoError('resource', 'resources holds no id of the resource of export_refunds (RefundRecord)'))
        return authorize_export_refunds(store, principal, {'resource': resources['RefundRecord']}, now)
    return Answer(False, [], None, SekishoError('action', f'the gate has no action {json.dumps(action, ensure_ascii=False)}'))

