# Try it in the browser

<p>This page has moved to <a id="moved" href="../playground/#project=rulec/gap">the playground on ritsu's site</a>.</p>

<script>
  // Send the reader on to where this page went, ritsu's playground: a link that opened something
  // here goes on with what it opened, and any other opens what this page opened on, the table with
  // a row missing (the link above).
  const to = new URL(document.getElementById("moved").href);
  if (location.hash.includes("=")) to.hash = location.hash;
  location.replace(to.href);
</script>
