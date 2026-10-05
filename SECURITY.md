# Security

## Reporting a vulnerability

Please report a vulnerability privately, through GitHub's private vulnerability reporting: the
**Report a vulnerability** button on the [Security tab](https://github.com/i2y/ritsu/security) of
this repository. Please do not open a public issue for it.

Say what it affects — the `ritsu` binary or one of the seven languages, the code a language
generates, a page a language draws, the playground on the site, or the action — which release you
use (its tag, such as `v0.23.0`), and, if you can, a file that shows it.

ritsu is kept by one person in their own time, so there is no fixed time for an answer. A report
is answered in its advisory, and once it is confirmed, the fix goes out in a release that the
advisory names.

## Supported versions

Only the latest release is supported: a fix goes into the next release, not into older ones.

## What the repository checks about itself

The audit workflow (`.github/workflows/audit.yml`) checks ritsu's own dependencies every day and
before every release: the crates in `Cargo.lock` with cargo-deny, and every lock file of the
repository with osv-scanner. The released binary carries the list of its crates, built with
cargo-auditable, so a scanner can read what is in it. [DESIGN.md](DESIGN.md) (3.6) says what is
checked and why.

---

## 脆弱性の知らせ方

脆弱性は、GitHub の非公開の報告で知らせてください。このリポジトリの [Security のタブ](https://github.com/i2y/ritsu/security) にある **Report a vulnerability** のボタンから送れます。公開の issue には書かないでください。

知らせるときは、次の三つを書いてください。

- 何にかかわるか：`ritsu` のバイナリか七つの言語のどれか、言語が生成するコード、言語が描くページ、サイトのブラウザで試すページ、action のどれか
- 使っているリリース（`v0.23.0` のようなタグ）
- できれば、それが起きるファイル

ritsu は一人が空いた時間に保守しているので、返事の期限は決めていません。返事はアドバイザリの中で書きます。確かめられたら直したリリースを出し、そのリリースをアドバイザリに書きます。

直すのは最新のリリースだけで、直したものは次のリリースに入ります。古いリリースには入れません。

リポジトリは、ritsu 自身の依存を毎日とリリースの前に確かめています（`.github/workflows/audit.yml`）。`Cargo.lock` のクレートは cargo-deny で、リポジトリのすべてのロックファイルは osv-scanner で確かめます。配るバイナリは cargo-auditable でビルドしてあり、含むクレートの一覧をスキャナーが読めます。何を確かめているかとその理由は、[DESIGN.md](DESIGN.md) の 3.6 にあります。
