import sys

a, op, b = sys.argv[1], sys.argv[2], sys.argv[3]
x, y = int(a), int(b)
if op == "+":
    print(x + y)
elif op == "*":
    print(x * y)
elif op == "/":
    if y == 0:
        print("cannot divide: division by zero", file=sys.stderr)
        sys.exit(1)
    print(x // y)
elif op == "**":
    print(x ** y)
else:
    print(f"unknown op: {op}", file=sys.stderr)
    sys.exit(2)
