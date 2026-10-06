#!/bin/sh
# Prints the Homebrew formula for one release, from the SHA256SUMS published with it (DESIGN
# 13.2). The formula installs the release archives themselves, so what `brew install
# i2y/tap/ritsu` puts on the path is the binary the release page offers, held to the same sums,
# with the eight links beside it; the keg holds the two licenses and THIRD_PARTY_NOTICES.
# release.yml runs this once the release is up, has brew audit, install and test what it printed
# on macOS and on Linux, and only then pushes it to the tap, i2y/homebrew-tap.
#
#   sh packaging/homebrew.sh v0.23.0 SHA256SUMS > Formula/ritsu.rb
set -eu

tag=$1
sums=$2
base="https://github.com/i2y/ritsu/releases/download/$tag"

sum() {
  s=$(awk -v n="ritsu-$tag-$1.tar.gz" '$2 == n { print $1 }' "$sums")
  if [ -z "$s" ]; then
    echo "$sums has no line for ritsu-$tag-$1.tar.gz" >&2
    exit 1
  fi
  echo "$s"
}

mac_arm=$(sum aarch64-apple-darwin)
mac_intel=$(sum x86_64-apple-darwin)
linux_arm=$(sum aarch64-unknown-linux-musl)
linux_intel=$(sum x86_64-unknown-linux-musl)

cat <<EOT
# Written by packaging/homebrew.sh in i2y/ritsu for $tag; the next release replaces it.
class Ritsu < Formula
  desc "Eight small languages, one toolchain: what one checks, the next can build on"
  homepage "https://github.com/i2y/ritsu"
  # ritsu's own code with the data it holds (crates/ritsu/Cargo.toml); brew's audit asks for a
  # nested license on lines of its own
  license all_of: [
    { any_of: ["MIT", "Apache-2.0"] },
    "Unicode-3.0",
    "BSD-3-Clause",
  ]

  on_macos do
    on_arm do
      url "$base/ritsu-$tag-aarch64-apple-darwin.tar.gz"
      sha256 "$mac_arm"
    end
    on_intel do
      url "$base/ritsu-$tag-x86_64-apple-darwin.tar.gz"
      sha256 "$mac_intel"
    end
  end

  on_linux do
    on_arm do
      url "$base/ritsu-$tag-aarch64-unknown-linux-musl.tar.gz"
      sha256 "$linux_arm"
    end
    on_intel do
      url "$base/ritsu-$tag-x86_64-unknown-linux-musl.tar.gz"
      sha256 "$linux_intel"
    end
  end

  def install
    bin.install "ritsu"
    # one link to it for each language: called by that name, ritsu is that language's command
    %w[rulec dandori koyomi chobo geas yuen sakai sekisho].each do |language|
      bin.install_symlink "ritsu" => language
    end
    # brew puts LICENSE-MIT and LICENSE-APACHE in the keg by their names; the notices of what the
    # binary holds from others go beside them
    prefix.install "THIRD_PARTY_NOTICES"
  end

  test do
    %w[LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES].each do |file|
      assert_path_exists prefix/file
    end
    assert_equal "ritsu #{version}", shell_output("#{bin}/ritsu --version").strip
    %w[rulec dandori koyomi chobo geas yuen sakai sekisho].each do |language|
      assert_equal "#{language} #{version}", shell_output("#{bin}/#{language} --version").strip
    end

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
    # the link and the same words after ritsu are one command
    assert_match "ok fee.rule", shell_output("#{bin}/ritsu rulec check fee.rule 2>&1")

    # Without the overseas row the rule has a gap, and check has to say which input falls in it.
    (testpath/"gap.rule").write (testpath/"fee.rule").read.sub(/^\\| overseas .*\\n/, "")
    assert_match "dest = overseas", shell_output("#{bin}/rulec check gap.rule 2>&1", 1)
  end
end
EOT
