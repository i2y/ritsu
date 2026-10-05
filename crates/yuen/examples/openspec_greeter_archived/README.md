# openspec_greeter_archived

`openspec_greeter` after `openspec archive trim-names` (OpenSpec 1.14.0 wrote the specs and moved
the change under `openspec/changes/archive/`). The `.req` files, the claims, the server and
`reviewed/` are the same bytes as that example's.

It stops on purpose. The block of `Greeting by name` is now the one the change wrote, so the check
stops on its pin with E103 and shows what changed since the block that was looked at; and the spec
has a requirement no source pins, `Health check`, which is W102. After `yuen source pin`, the link
from the requirement of the spec and the three links below `greeting_by_name` are marked (E302)
until someone looks at them again; the other requirements of the spec mark nothing.

```console
$ ritsu yuen check examples/openspec_greeter_archived/greeter.req --root examples/openspec_greeter_archived
$ ritsu yuen check examples/openspec_greeter_archived/greeter.ja.req --root examples/openspec_greeter_archived
```
