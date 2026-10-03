# geas's coverage hook for Python (DESIGN §7.2). While `geas map` runs, geas puts
# the directory holding this file first on PYTHONPATH, so every Python process a
# target starts imports it as its sitecustomize. It records with sys.monitoring
# the lines that run in the files under the root, writes them into GEAS_COVER_OUT
# when the process exits or gets SIGTERM, and then runs the sitecustomize this
# Python would have run without it.
import os
import sys

_GEAS_EXCLUDED = {".geas", ".git", "node_modules", "site-packages", "__pycache__", "target"}


def _geas_start():
    out = os.environ.get("GEAS_COVER_OUT")
    root = os.environ.get("GEAS_COVER_ROOT")
    if not out or not root or not hasattr(sys, "monitoring"):
        return  # Python before 3.12 has no sys.monitoring: no record (W060)
    import atexit
    import json
    import signal

    mon = sys.monitoring
    tool = mon.COVERAGE_ID
    try:
        mon.use_tool_id(tool, "geas")
    except ValueError:
        return  # another coverage tool holds the slot
    root = os.path.realpath(root)
    here = os.path.dirname(os.path.realpath(__file__))
    known = {}  # a code object's file name -> its real path under the root, or None

    def path_of(name):
        if name in known:
            return known[name]
        p = None
        if name.endswith(".py"):
            try:
                real = os.path.realpath(name)
            except (OSError, ValueError):
                real = ""
            if real.startswith(root + os.sep) and not real.startswith(here + os.sep):
                if not _GEAS_EXCLUDED.intersection(real[len(root) + 1:].split(os.sep)):
                    p = real
        known[name] = p
        return p

    ran = {}  # real path -> the lines that ran
    modules = {}  # real path -> the module's code objects

    def on_line(code, line):
        p = path_of(code.co_filename)
        if p is not None:
            ran.setdefault(p, set()).add(line)
        return mon.DISABLE  # once a line has run, it costs nothing more

    def on_start(code, offset):
        if code.co_name == "<module>":
            p = path_of(code.co_filename)
            if p is not None:
                modules.setdefault(p, []).append(code)
        return mon.DISABLE

    mon.register_callback(tool, mon.events.LINE, on_line)
    mon.register_callback(tool, mon.events.PY_START, on_start)
    mon.set_events(tool, mon.events.LINE | mon.events.PY_START)

    def code_lines(code, acc):
        for _, _, line in code.co_lines():
            if line:  # None, or 0 for the frame's start
                acc.add(line)
        for c in code.co_consts:
            if hasattr(c, "co_lines"):
                code_lines(c, acc)

    written = []

    def write():
        if written:
            return
        written.append(True)
        files = {}
        for p in set(ran) | set(modules):
            code = set()
            for c in modules.get(p, ()):
                code_lines(c, code)
            hit = ran.get(p, set())
            files[p] = {"code": sorted(code | hit), "ran": sorted(hit)}
        try:
            os.makedirs(out, exist_ok=True)
            with open(os.path.join(out, "python-%d.json" % os.getpid()), "w") as f:
                json.dump({"files": files}, f)
        except OSError:
            pass

    atexit.register(write)

    def on_term(signum, frame):
        write()
        os._exit(128 + signum)

    try:
        signal.signal(signal.SIGTERM, on_term)
    except ValueError:
        pass  # only the main thread may set a handler


def _geas_chain():
    import importlib.machinery
    import importlib.util

    here = os.path.dirname(os.path.realpath(__file__))
    rest = [p for p in sys.path if os.path.realpath(p or ".") != here]
    spec = importlib.machinery.PathFinder.find_spec("sitecustomize", rest)
    if spec is None or spec.loader is None:
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules["sitecustomize"] = module
    spec.loader.exec_module(module)


_geas_start()
_geas_chain()
