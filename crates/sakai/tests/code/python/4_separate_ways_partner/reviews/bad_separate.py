# Reviews call into billing, though the two go separate ways.
from billing import invoice


def billed() -> bool:
    return invoice.due is not None
