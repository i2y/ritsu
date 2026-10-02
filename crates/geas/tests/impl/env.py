# Prints, for each name it is given, `NAME=value` or `NAME is not set`, one line
# each in the order given. A name written `?NAME` prints only whether it is set,
# since its value belongs to the machine (PATH).
import os
import sys

for name in sys.argv[1:]:
    if name.startswith("?"):
        name = name[1:]
        print(f"{name} is {'set' if name in os.environ else 'not set'}")
    elif name in os.environ:
        print(f"{name}={os.environ[name]}")
    else:
        print(f"{name} is not set")
