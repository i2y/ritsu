# The host of handles: it takes what people typed, and the generated code of the module says the
# handle kept for each, or why there is none. Written beside the package `ritsu gen` writes
# (python/), and run there.
from generated.glue import handles, kaname

for typed in [" Grace Hopper", "9", "Root", "ünïcode"]:
    r = handles.handle(raw=typed)
    print(f'"{typed}" -> {r.value}' if isinstance(r, kaname.Ok) else f'"{typed}" -> refused: {"; ".join(r.error)}')
for typed in ["Ada Lovelace#42", "ada#0"]:
    t = handles.tag(text=typed)
    print(f'"{typed}" -> {t.value.handle} #{t.value.number}' if isinstance(t, kaname.Ok) else f'"{typed}" -> refused: {"; ".join(t.error)}')
