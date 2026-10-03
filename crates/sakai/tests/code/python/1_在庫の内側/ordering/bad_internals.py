# Ordering takes stock from inventory's ledger, the inside of inventory.
from inventory import ledger


def take(order: str, sku: str) -> bool:
    return ledger.hold(order, sku, 1)
