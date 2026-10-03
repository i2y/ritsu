# Prints the shop's sign.
import sys

name = sys.argv[1] if len(sys.argv) > 1 else "shop"
print(f"Welcome to the {name}")
print("Open every day")
