# Reviews keep to themselves: nothing goes between reviews and billing.
def average(stars: list[int]) -> float:
    return sum(stars) / len(stars) if stars else 0.0
