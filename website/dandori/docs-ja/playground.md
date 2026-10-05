# ブラウザで試す

<p>このページは、ritsu のサイトの<a id="moved" href="../../ja/playground/#flow=tests/fixtures/hotel_naive.flow">ブラウザで試すページ</a>に移りました。</p>

<script>
  // Send the reader on to where this page went, ritsu's playground, which reads the links this page
  // gave: a link that opened something here (#flow=…&view=…) goes on with what it opened, and any
  // other opens what this page opened on, the first draft of the hotel booking (the link above).
  const to = new URL(document.getElementById("moved").href);
  if (location.hash.includes("=")) to.hash = location.hash;
  location.replace(to.href);
</script>
