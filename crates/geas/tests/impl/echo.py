# Prints its first argument on stdout and its second on stderr, each followed by a
# line break unless it is empty, and exits with its third: a process whose
# observation a test chooses.
import sys

out, err, code = sys.argv[1], sys.argv[2], int(sys.argv[3])
if out:
    print(out)
if err:
    print(err, file=sys.stderr)
sys.exit(code)
