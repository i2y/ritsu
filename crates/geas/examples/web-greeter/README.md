# web-greeter

A page and the service behind it, in Python's standard library ([server.py](server.py),
[index.html](index.html)). The page asks for a name, greets it by the time of day, lists the names
it has greeted, which it keeps in the service, and draws a fortune at random.
[web-greeter.geas](web-greeter.geas) holds six claims: three on the page as a person sees it in a
browser, one on the service's API alone, and two that use both, one driving the page and then
asking the API what the page did, the other posting a name to the API and then opening the page.

```console
$ geas check examples/web-greeter/web-greeter.geas
```

It needs Chrome or Chromium (`GEAS_CHROME`, the macOS application, or `google-chrome`, `chromium`
or `chromium-browser` on PATH) and python3. geas opens the page in a headless Chrome and reads the
screen as a screen reader is told it, so the claims name a `heading`, a `textbox "Your name"`, a
`button "Greet"` and whether it is `disabled`, never a selector.

What to look for in the claims file:

- **The four pins.** The page writes today's date in the locale's words, greets by the hour, and
  draws a fortune with `Math.random`. `locale`, `tz`, `clock` and `seed` make those the same on
  every machine and in every run, so a claim can say `"Today is Saturday, August 29, 2026."` and
  `"Good morning, Ada!"`, and drift on the unchanged page is quiet.
- **The page and the API in one claim.** "the service keeps the names the page greets" types a
  name, presses Enter, and then reads `/api/greetings` from the same service.
- **Matchers.** `with value ""`, `enabled` and `disabled`, `in list "Greeted"`, `exactly 0
  listitem`, `matching "Your fortune: .+"`, `header "content-type"`, `body json ".names[1]" does
  not exist`, `is between 400 and 499`.

[tests/changes/web-greeter/after/index.html](../../tests/changes/web-greeter/after/index.html) is the
page after an agent restyled the Greet button as a `<div>` with a click handler. Copied over
`index.html`, it fails two claims: the screen shows the text "Greet" where the button was, and the
click is refused (E035), since a `<div>` is not a button to a screen reader. The top-level
[README](../../README.md#screens) shows that run.
