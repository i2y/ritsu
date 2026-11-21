def authorize(store: Store, principal: Principal, action: str, resources: dict[str, str], now: datetime | None = None) -> Answer:
    """action という名前の action を許すかを Cedar（このプロセスの中の cedarpy）に尋ねます。尋ねるのは、その action の関数です（view_order なら authorize_view_order）。action は Cedar での action の名前で、resources は、resource の ID を、その型の Cedar での名前ごとに持ちます。action がとらない型の ID は読みません。ゲートに無い名前（種類は action）、自分の入力を受け取る action（input。その action の関数で尋ねてください）、action の resource の ID が無いか、二つの型の ID があるとき（resource）は、SekishoError を持つ拒む答えを返します。Store の失敗は、そのまま投げます。"""
    if action == 'view_order':
        if 'Order' not in resources:
            return Answer(False, [], None, SekishoError('resource', 'resources に view_order の resource（Order）の ID がありません'))
        return authorize_view_order(store, principal, {'orderId': resources['Order']}, now)
    if action == 'refund_order':
        return Answer(False, [], None, SekishoError('input', 'refund_order は自分の入力（amount）を受け取ります。authorize_refund_order で尋ねてください'))
    if action == 'export_refunds':
        if 'RefundRecord' not in resources:
            return Answer(False, [], None, SekishoError('resource', 'resources に export_refunds の resource（RefundRecord）の ID がありません'))
        return authorize_export_refunds(store, principal, {'resource': resources['RefundRecord']}, now)
    return Answer(False, [], None, SekishoError('action', f'ゲートに action {json.dumps(action, ensure_ascii=False)} はありません'))

