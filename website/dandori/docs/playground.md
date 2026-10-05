# Try it in the browser

<p>This page has moved to <a id="moved" href="../playground/#flow=tests/fixtures/hotel_naive.flow">the playground on ritsu's site</a>.</p>

<script>
  // Send the reader on to where this page went, ritsu's playground, which reads the links this page
  // gave: a link that opened something here (#flow=…&view=…) goes on with what it opened, and any
  // other opens what this page opened on, the first draft of the hotel booking (the link above).
  const to = new URL(document.getElementById("moved").href);
  if (location.hash.includes("=")) to.hash = location.hash;
  location.replace(to.href);
</script>
