// What the pages of `ritsu mod site` do. Each page reads and works without it, as far as a page can.
(function () {
  "use strict";
  var root = document.documentElement;
  var all = function (selector, from) {
    return Array.prototype.slice.call((from || document).querySelectorAll(selector));
  };

  // The theme: the reader's choice, kept in this browser when it lets a page keep anything.
  all(".rm-theme").forEach(function (b) {
    b.addEventListener("click", function () {
      var dark = root.getAttribute("data-theme") === "dark" || (!root.getAttribute("data-theme") && window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches);
      var next = dark ? "light" : "dark";
      root.setAttribute("data-theme", next);
      try {
        localStorage.setItem("ritsu-theme", next);
      } catch (e) {}
    });
  });

  // A menu whose options are the pages to go to, keeping the tab.
  all("select[data-go]").forEach(function (s) {
    s.addEventListener("change", function () {
      location.href = s.value + (root.getAttribute("data-tabs") ? location.hash : "");
    });
  });

  // The copy buttons: each copies the code just under its line.
  all("button.copy").forEach(function (b) {
    b.addEventListener("click", function () {
      var pre = b.parentNode.nextElementSibling;
      var said = function (text) {
        b.textContent = text;
        clearTimeout(b.timer);
        b.timer = setTimeout(function () {
          b.textContent = b.dataset.copy;
        }, 1600);
      };
      var done = navigator.clipboard && navigator.clipboard.writeText(pre.textContent);
      if (done && done.then) {
        done.then(function () {
          said(b.dataset.copied);
        }, function () {
          said(b.dataset.failed);
        });
      } else {
        said(b.dataset.failed);
      }
    });
  });

  // The tabs: the address's # picks one, and `#try:<file>` a file to try too.
  var tabs = (root.getAttribute("data-tabs") || "").split(" ").filter(Boolean);
  var tryFile = document.getElementById("try-file");
  var tryFrame = document.getElementById("try-frame");
  // A page that fills its window (a rule's) keeps the frame's height, and any other is as tall as it
  // is. A browser that does not let a page read a file beside it leaves the height as it is.
  var fit = function () {
    try {
      var b = tryFrame.contentDocument.body;
      tryFrame.style.height = b.classList.contains("boarded") ? "" : b.offsetHeight + "px";
    } catch (e) {}
  };
  var openTry = function () {
    if (!tryFrame || !tryFile || !tryFile.value) return;
    var want = tryFile.value + "?view=try";
    if (tryFrame.getAttribute("src") !== want) {
      tryFrame.hidden = false;
      tryFrame.setAttribute("src", want);
    }
  };
  if (tryFrame) {
    tryFrame.addEventListener("load", function () {
      fit();
      try {
        new ResizeObserver(fit).observe(tryFrame.contentDocument.body);
      } catch (e) {}
    });
    tryFile.addEventListener("change", function () {
      var o = tryFile.options[tryFile.selectedIndex];
      history.replaceState(null, "", "#try:" + o.dataset.rel);
      openTry();
    });
  }
  var show = function (fromClick) {
    if (!tabs.length) return;
    var hash = location.hash.slice(1);
    var parts = hash.split(":");
    var tab = tabs.indexOf(parts[0]) >= 0 ? parts[0] : tabs[0];
    root.setAttribute("data-tab", tab);
    all("a.tab").forEach(function (a) {
      if (a.dataset.tab === tab) {
        a.setAttribute("aria-current", "page");
        // a narrow window scrolls the tabs: this one stays in sight
        a.parentNode.scrollLeft = Math.max(0, a.offsetLeft - a.parentNode.offsetLeft - 24);
      } else {
        a.removeAttribute("aria-current");
      }
    });
    if (tab === "try" && tryFile) {
      if (parts[1]) {
        for (var i = 0; i < tryFile.options.length; i++) {
          if (tryFile.options[i].dataset.rel === parts.slice(1).join(":")) tryFile.selectedIndex = i;
        }
      }
      openTry();
    }
    if (fromClick || tabs.indexOf(parts[0]) >= 0) window.scrollTo(0, 0);
  };
  if (tabs.length) {
    all("a.tab, a.trylink").forEach(function (a) {
      a.addEventListener("click", function (e) {
        e.preventDefault();
        history.pushState(null, "", a.getAttribute("href"));
        show(true);
      });
    });
    window.addEventListener("hashchange", function () {
      show(false);
    });
    window.addEventListener("popstate", function () {
      show(false);
    });
    show(false);
  }

  // The search: on the list it narrows the rows as the reader types; on any other page the form
  // goes to the list, which reads it from the address.
  var rows = all(".row");
  if (rows.length) {
    var q = document.getElementById("rm-q");
    var empty = document.querySelector(".empty");
    var filters = all(".fil");
    var kind = "";
    var narrow = function () {
      var text = (q.value || "").trim().toLowerCase();
      var shown = 0;
      var counts = {};
      rows.forEach(function (r) {
        var hit = !text || r.dataset.text.indexOf(text) >= 0;
        var kinds = (r.dataset.kinds || "").split(" ").filter(Boolean);
        if (hit) {
          kinds.forEach(function (k) {
            counts[k] = (counts[k] || 0) + 1;
          });
          counts[""] = (counts[""] || 0) + 1;
        }
        var on = hit && (!kind || kinds.indexOf(kind) >= 0);
        r.hidden = !on;
        if (on) shown++;
        all(".item", r).forEach(function (i) {
          i.classList.toggle("hit", !!text && i.dataset.name.indexOf(text) >= 0);
        });
      });
      filters.forEach(function (f) {
        var c = f.querySelector(".count");
        if (c) c.textContent = counts[f.dataset.kind] || 0;
        var pressed = f.dataset.kind === kind;
        f.classList.toggle("on", pressed);
        f.setAttribute("aria-pressed", pressed ? "true" : "false");
      });
      if (empty) {
        empty.hidden = shown > 0;
        if (!shown) empty.querySelector(".emptyhead").textContent = empty.dataset.text.replace("%s", q.value.trim());
      }
    };
    filters.forEach(function (f) {
      f.addEventListener("click", function () {
        kind = f.dataset.kind;
        narrow();
      });
    });
    if (q) {
      var asked = new URLSearchParams(location.search).get("q");
      if (asked) q.value = asked;
      q.addEventListener("input", function () {
        try {
          var u = new URL(location.href);
          if (q.value) u.searchParams.set("q", q.value);
          else u.searchParams.delete("q");
          history.replaceState(null, "", u);
        } catch (e) {}
        narrow();
      });
      q.form.addEventListener("submit", function (e) {
        e.preventDefault();
      });
      narrow();
    }
  }

  // Opening a version in the playground: module.json read from beside the page, packed as the
  // playground packs the edits of its links (its JSON compressed as raw DEFLATE by the browser's
  // CompressionStream, in base64url with no padding), and the playground opened on it.
  var open = document.getElementById("open");
  if (open) {
    open.addEventListener("click", async function () {
      open.disabled = true;
      try {
        var files = await (await fetch(open.dataset.module)).json();
        var stream = new Blob([JSON.stringify(files)]).stream().pipeThrough(new CompressionStream("deflate-raw"));
        var bytes = new Uint8Array(await new Response(stream).arrayBuffer());
        var bin = "";
        for (var i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
        var packed = btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
        if (packed.length > Number(open.dataset.max)) {
          open.textContent = open.dataset.big;
          return;
        }
        location.assign(open.dataset.playground + "#project=empty&edits=" + packed);
      } catch (err) {
        open.textContent = open.dataset.failed;
      }
    });
  }

  // A file's code: a line's number marks the line, in the address, for a link to it. The number is
  // the stylesheet's, so a click on it is a click on the line, left of where the text starts.
  var code = document.querySelector("pre.lines");
  if (code) {
    var mark = function (line) {
      all(".l.on", code).forEach(function (l) {
        l.classList.remove("on");
      });
      if (line) line.classList.add("on");
    };
    if (/^#L[0-9]+$/.test(location.hash)) mark(document.getElementById(location.hash.slice(1)));
    code.addEventListener("click", function (e) {
      var line = e.target;
      if (!line.classList.contains("l")) return;
      if (e.clientX - code.getBoundingClientRect().left > parseFloat(getComputedStyle(line).paddingLeft)) return;
      history.replaceState(null, "", "#" + line.id);
      mark(line);
    });
  }

  // A file's page shown inside the tab of trying it: only its section to try, and, on a page that
  // lays itself out after this script (a rule's), the table beside it that lights up as it answers.
  if (root.classList.contains("rm-bare")) {
    var section = document.getElementById("try");
    if (section) {
      for (var el = section; el && el !== document.body; el = el.parentNode) {
        Array.prototype.forEach.call(el.parentNode.children, function (sibling) {
          if (sibling !== el && sibling.tagName !== "SCRIPT" && sibling.tagName !== "STYLE") sibling.hidden = true;
        });
      }
    }
    root.classList.add("rm-ready");
  }
})();
