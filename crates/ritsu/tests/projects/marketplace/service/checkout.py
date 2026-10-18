"""The marketplace's checkout, written by hand on the package ritsu gen writes from this project
(`ritsu gen --root . --out generated`, with generated/python on the path): the rule splits what
the buyer paid, and the book records the sale, in one call and with no workflow. What the rule
returns is the package's type of its unit, units.USDc, which is what the book's sale takes: were
the rule to count in whole dollars, mypy would stop here."""

from __future__ import annotations

from generated import units
from generated.books import marketplace
from generated.rules import sale_split


def checkout(book: marketplace.Book, order: str, shop: str, plan: sale_split.Plan, paid: units.USDc) -> marketplace.Result:
    """Record a sale of `paid` for `shop`: the marketplace's fee by the shop's plan, the rest to the shop."""
    split = sale_split.sale_split(plan, paid)
    return book.sale.do(order=order, shop=shop, to_shop=split.to_shop, fee=split.fee)
