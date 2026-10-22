# The host of catalog: it reads the tree of a catalog, and the generated code of the module walks it.
# Written beside the package `ritsu gen` writes (python/), and run there.
from generated.glue import catalog
from generated.units import USDc

garden = catalog.Category(
    name="Garden",
    items=[],
    children=[
        catalog.Category(
            name="Hand Tools",
            items=[catalog.Item(sku="sp-1", name="Spade", price=USDc(1999)), catalog.Item(sku="tr-2", name="Trowel", price=USDc(899))],
            children=[],
        ),
        catalog.Category(name="Seeds", items=[], children=[catalog.Category(name="Bulbs", items=[], children=[])]),
        catalog.Category(name="Hoses", items=[catalog.Item(sku="hr-1", name="Hose reel", price=USDc(4500))], children=[]),
    ],
)
for p in catalog.paths(c=garden):
    print(p)
kept = catalog.pruned(c=garden)
print("kept: " + ("nothing" if kept is None else ", ".join(catalog.names(c=kept))))
print("slugs: " + ", ".join(catalog.names(c=catalog.slugged(c=garden))))
c = catalog.cheapest(c=garden)
print("cheapest: " + ("none" if c is None else f"{c.name}, {c.price} cents"))
