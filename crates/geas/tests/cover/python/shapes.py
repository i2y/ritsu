# Areas of a few shapes, from the command line.
import sys


def area(kind, size):
    if kind == "square":
        return size * size
    if kind == "circle":
        return 3 * size * size
    raise ValueError(kind)


class Unused:
    def method(self):
        return 1


print(area(sys.argv[1], int(sys.argv[2])))
