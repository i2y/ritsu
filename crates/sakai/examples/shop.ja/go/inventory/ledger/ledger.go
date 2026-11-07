// A stand-in for what `chobo gen` writes from inventory/在庫の引当.book: the ledger of stock. A call answers
// "done", "done_before" or "refused" with its reason, as chobo's Result does.
package ledger

type held struct {
	sku   string
	count int
}

var holds = map[string]held{}

func Hold(line, sku string, count int) (result, reason string) {
	if before, ok := holds[line]; ok {
		if before == (held{sku, count}) {
			return "done_before", ""
		}
		return "refused", "key_conflict"
	}
	if count <= 0 {
		return "refused", "out_of_stock"
	}
	holds[line] = held{sku, count}
	return "done", ""
}

func Post(line string) (result, reason string) {
	if _, ok := holds[line]; ok {
		return "done", ""
	}
	return "refused", "no_such_hold"
}
