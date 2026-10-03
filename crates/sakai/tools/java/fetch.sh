#!/bin/sh
# The jars the ArchUnit test of sakai runs with (PLAN 0.4), fetched from Maven Central into
# tools/java/lib. Each jar is held to the SHA-256 written here; one that differs is removed and
# the script stops. tools/java/lib is not in git.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
lib="$here/lib"
central=https://repo1.maven.org/maven2
mkdir -p "$lib"

digest() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# fetch <group, as a path> <artifact> <version> <sha256>
fetch() {
  jar="$2-$3.jar"
  if [ ! -f "$lib/$jar" ]; then
    curl -fsSL -o "$lib/$jar.part" "$central/$1/$2/$3/$jar"
    mv "$lib/$jar.part" "$lib/$jar"
  fi
  got=$(digest "$lib/$jar")
  if [ "$got" != "$4" ]; then
    echo "$jar: the SHA-256 is $got, not $4" >&2
    rm -f "$lib/$jar"
    exit 1
  fi
  echo "$jar"
}

fetch com/tngtech/archunit archunit 1.5.1 a4dbfc51c90005ad6ac9967672a7efb43aa6928287a57238b28e60e0a75c5c6f
fetch com/tngtech/archunit archunit-junit5-api 1.5.1 dc93df23e0113a82ab913a63135febe8b563a38d7e26aae12c3c7475893eeb26
fetch com/tngtech/archunit archunit-junit5-engine 1.5.1 a3a9db142ea31a7bfd124580839dd7ffc83b292f6b09e32d2025ac24be7bc04d
fetch com/tngtech/archunit archunit-junit5-engine-api 1.5.1 d8a4c1e51fecf593abcf1a52925a48525c9fa0e0d93d9018e588b1211341fdf3
fetch org/junit/platform junit-platform-console-standalone 6.1.3 e62b96ac475dbcde8599ea905d088f65d90778f86e259b856a49fa5c4ea256ec
fetch org/slf4j slf4j-api 2.0.17 7b751d952061954d5abfed7181c1f645d336091b679891591d63329c622eb832
