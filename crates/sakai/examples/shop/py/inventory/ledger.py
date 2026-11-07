# A stand-in for the module `chobo gen` writes from inventory/inventory.book: the ledger of stock. A call answers
# ("done", None), ("done_before", None) or ("refused", the reason), as chobo's Result does.
_held = {}


def hold(line: str, sku: str, count: int) -> tuple[str, str | None]:
    if line in _held:
        return ("done_before", None) if _held[line] == (sku, count) else ("refused", "key_conflict")
    if count <= 0:
        return ("refused", "out_of_stock")
    _held[line] = (sku, count)
    return ("done", None)


def post(line: str) -> tuple[str, str | None]:
    return ("done", None) if line in _held else ("refused", "no_such_hold")
