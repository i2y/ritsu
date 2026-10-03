#!/bin/sh
# Prints the Homebrew formula for one release, from the SHA256SUMS published with it
# (DESIGN §15.158). The formula installs the release archives themselves, so what
# `brew install i2y/tap/rulec` puts on the path is the binary the release page
# offers, held to the same sums. release.yml runs this once the release is up, has
# brew audit, install and test what it printed on macOS and on Linux, and only then
# pushes it to the tap, i2y/homebrew-tap.
#
#   sh packaging/homebrew.sh v0.9.0 SHA256SUMS > Formula/rulec.rb
set -eu

tag=$1
sums=$2
base="https://github.com/i2y/rulec/releases/download/$tag"

sum() {
  s=$(awk -v n="rulec-$tag-$1.tar.gz" '$2 == n { print $1 }' "$sums")
  if [ -z "$s" ]; then
    echo "$sums has no line for rulec-$tag-$1.tar.gz" >&2
    exit 1
  fi
  echo "$s"
}

mac_arm=$(sum aarch64-apple-darwin)
mac_intel=$(sum x86_64-apple-darwin)
linux_arm=$(sum aarch64-unknown-linux-musl)
linux_intel=$(sum x86_64-unknown-linux-musl)

cat <<EOF
# Written by packaging/homebrew.sh in i2y/rulec for $tag; the next release replaces it.
class Rulec < Formula
  desc "Little language for business rules that proves each rule before it compiles"
  homepage "https://i2y.github.io/rulec/"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "$base/rulec-$tag-aarch64-apple-darwin.tar.gz"
      sha256 "$mac_arm"
    end
    on_intel do
      url "$base/rulec-$tag-x86_64-apple-darwin.tar.gz"
      sha256 "$mac_intel"
    end
  end

  on_linux do
    on_arm do
      url "$base/rulec-$tag-aarch64-unknown-linux-musl.tar.gz"
      sha256 "$linux_arm"
    end
    on_intel do
      url "$base/rulec-$tag-x86_64-unknown-linux-musl.tar.gz"
      sha256 "$linux_intel"
    end
  end

  def install
    bin.install "rulec"
  end

  test do
    assert_equal "rulec #{version}", shell_output("#{bin}/rulec --version").strip

    (testpath/"fee.rule").write <<~RULE
      rule fee v1
      description "Two zones, one fee each"

      enum zone = domestic | overseas

      inputs
        dest : zone

      outputs
        fee : money[USD, incl_tax]  round up(1USD)

      table fee
      policy unique
      | dest     | -> fee : money[USD, incl_tax] |
      | domestic | 6USD                          |
      | overseas | 16USD                         |
    RULE
    assert_match "ok fee.rule", shell_output("#{bin}/rulec check fee.rule 2>&1")

    # Without the overseas row the rule has a gap, and check has to say which input falls in it.
    (testpath/"gap.rule").write (testpath/"fee.rule").read.sub(/^\\| overseas .*\\n/, "")
    assert_match "dest = overseas", shell_output("#{bin}/rulec check gap.rule 2>&1", 1)
  end
end
EOF
