# A stand-in for the module `chobo gen` writes from inventory/inventory.book: the ledger of stock.
def hold(order: str, sku: str, count: int) -> bool:
    return count > 0
