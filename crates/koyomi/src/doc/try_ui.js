// The section of the page that asks: a date, the three shortcuts, the answer and the next closed days.
(function () {
  var section = document.getElementById("try");
  var D = JSON.parse(document.getElementById("try-data").textContent);
  var W = D.words;
  var cal = tryCalendar(D);
  var date = document.getElementById("try-date");
  var box = document.getElementById("try-answer");
  var after = document.getElementById("try-next");
  var buttons = section.querySelectorAll("button[data-go]");
  // How far ahead the page looks for the next closed days.
  var LOOK = 3700;
  function el(tag, text, cls) {
    var e = document.createElement(tag);
    if (text !== undefined) e.textContent = text;
    if (cls) e.className = cls;
    return e;
  }
  function closed(z) {
    var a = cal.answer(z);
    return a.known && !a.open ? a : null;
  }
  function nextClosed(z) {
    for (var x = z, end = Math.min(D.to, z + LOOK); x <= end; x++) if (closed(x)) return x;
    return null;
  }
  function today() {
    var n = new Date();
    return cal.dayOf(n.getFullYear(), n.getMonth() + 1, n.getDate());
  }
  function fill(text, values) {
    return text.replace(/\{(\w+)\}/g, function (_, k) {
      return values[k];
    });
  }
  // Why the day is what it is.
  function why(z, a) {
    if (!a.known) return [fill(W.why_unknown, { from: cal.iso(D.from), to: cal.iso(D.to) })];
    if (a.opened) return [fill(W.why_opened, { line: a.opened })];
    if (a.open) {
      var w = cal.weekday(z);
      if (w >= 5 && !D.weekly[w]) return [fill(W.why_weekend, { day: D.weekdays[w] }), "closed weekly sat, sun"];
      return [W.why_open];
    }
    return [a.reasons.map(function (r) {
      return r.kind === "weekly" ? fill(W.why_weekly, { day: r.text }) : r.kind === "table" ? fill(W.why_table, { table: r.table }) : r.kind === "every" ? W.why_every : W.why_day;
    }).filter(function (t, i, all) {
      return all.indexOf(t) === i;
    }).join(" ")];
  }
  function show(z) {
    box.textContent = "";
    if (z === null) {
      box.className = "ans";
      box.appendChild(el("p", W.choose, "head"));
      after.hidden = true;
      return;
    }
    var a = cal.answer(z);
    box.className = "ans " + (!a.known ? "unknown" : a.open ? "open" : "closed");
    box.appendChild(el("p", !a.known ? W.unknown : a.open ? W.open : W.closed, "head"));
    if (a.reasons.length) {
      box.appendChild(el("p", a.reasons.map(function (r) {
        return r.text;
      }).join(W.and), "name"));
    }
    var said = why(z, a);
    var p = el("p", said[0], "why");
    if (said[1]) p.appendChild(el("code", said[1]));
    box.appendChild(p);
    // the next closed days, from this one
    var rows = after.querySelector("tbody");
    rows.textContent = "";
    var x = a.known ? nextClosed(z) : null;
    for (var n = 0; x !== null && n < 5; n++) {
      var tr = el("tr");
      tr.appendChild(el("td", cal.iso(x), "d"));
      tr.appendChild(el("td", D.short[cal.weekday(x)], "w"));
      tr.appendChild(el("td", closed(x).reasons.map(function (r) {
        return r.text;
      }).join(W.and)));
      rows.appendChild(tr);
      x = nextClosed(x + 1);
    }
    after.hidden = !rows.children.length;
  }
  function ask(z, keep) {
    date.value = z === null ? "" : cal.iso(z);
    show(z);
    if (!keep) {
      try {
        var u = new URL(location.href);
        if (z === null) u.searchParams.delete("date");
        else u.searchParams.set("date", cal.iso(z));
        history.replaceState(null, "", u);
      } catch (e) {}
    }
  }
  date.addEventListener("change", function () {
    ask(cal.parse(date.value));
  });
  buttons.forEach(function (b) {
    b.addEventListener("click", function () {
      var now = cal.parse(date.value);
      if (b.dataset.go === "today") ask(today());
      if (b.dataset.go === "outside") ask(D.to + 1);
      if (b.dataset.go === "next") {
        var from = Math.max(now === null ? today() : now + 1, D.from);
        var next = from <= D.to ? nextClosed(from) : null;
        if (next === null) b.disabled = true;
        else ask(next);
      }
    });
  });
  section.hidden = false;
  var asked = null;
  try {
    asked = cal.parse(new URLSearchParams(location.search).get("date"));
  } catch (e) {}
  ask(asked === null ? today() : asked, true);
})();
