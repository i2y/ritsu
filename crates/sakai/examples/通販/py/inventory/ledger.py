# A stand-in for the module `chobo build` writes from inventory/在庫の引当.book: the ledger of stock.
def hold(order: str, sku: str, count: int) -> bool:
    return count > 0
