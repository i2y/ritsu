// Authorize は、action という名前の action を許すかを Cedar（このプロセスの中の
// cedar-go）に尋ねます。尋ねるのは、その action の関数です（view_order なら AuthorizeViewOrder）。
// action は Cedar での action の名前で、resources は、resource の ID を、その型の Cedar
// での名前ごとに持ちます。action がとらない型の ID は読みません。ゲートに無い名前（種類は
// action）、自分の入力を受け取る action（input。その action の関数で尋ねてください）、action の
// resource の ID が無いか、二つの型の ID があるとき（resource）は、*Error
// を持つ拒む答えを返します。
func Authorize(store Store, p Principal, action string, resources map[string]string, now time.Time) Answer {
	switch action {
	case "view_order":
		id, ok := resources["Order"]
		if !ok {
			return Answer{Policies: []string{}, Err: &Error{"resource", "resources に view_order の resource（Order）の ID がありません"}}
		}
		return AuthorizeViewOrder(store, p, ViewOrderInput{OrderId: id}, now)
	case "refund_order":
		return Answer{Policies: []string{}, Err: &Error{"input", "refund_order は自分の入力（amount）を受け取ります。AuthorizeRefundOrder で尋ねてください"}}
	case "export_refunds":
		id, ok := resources["RefundRecord"]
		if !ok {
			return Answer{Policies: []string{}, Err: &Error{"resource", "resources に export_refunds の resource（RefundRecord）の ID がありません"}}
		}
		return AuthorizeExportRefunds(store, p, ExportRefundsInput{Resource: id}, now)
	}
	return Answer{Policies: []string{}, Err: &Error{"action", fmt.Sprintf("ゲートに action %q はありません", action)}}
}

