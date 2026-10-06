# ritsu 実装計画

DESIGN.md を仕様として、ritsu を五つの段階（B〜F）で作る。どの段階も、ここに書いた順に進め、各段階の最後にある完了の条件が全部成り立ったら終わりにする。DESIGN.md と違うことをしたくなったら、先に DESIGN.md に決定と理由と捨てたものを書き、報告で言う。

この計画は段階 A（設計）で書いた。コードは書いていない。DESIGN.md の 1 章の数と 12.5 の試しは、A の段階に作業場所で測ったもので、試しに使ったコピーは残していない。

| 段階 | すること | 主に読む DESIGN の章 |
|---|---|---|
| B | `~/ritsu` を作り、七つの履歴を取り込み、中身を直さずに全部のテストを通す | 2、12 |
| C | 重なっているものを土台へ移す。koyomi・chobo・geas・yuen・sakai から先に、rulec と dandori は合うところだけ | 4、9.2、10 |
| D | dandori と rulec のつなぎを型付きの呼び出しに替え、yuen と sakai の一式の読み込みを型付きで作り、単位の型を一つにする | 3、5、6、7.3、7.10〜7.12 |
| E | 言語をまたぐ検査、`ritsu check`、一つの生成パッケージ | 6、7、8、9.3 |
| F | yuen と sakai の段階 D、ritsu の README・DESIGN・スキル、LSP（今回は作らない）、ブラウザのページ、Lean の層、リリースの準備 | 8、11、13 |

## 0. 全部の段階に共通の決まり

### 0.1 守ること

- 作者の決まり（段階ごとの指示書が挙げるもの）を先に読み、従う。日本語（DESIGN.md、PLAN.md、`--lang ja` の診断、README.ja.md、報告）は、英語の概念語を漢字に直訳しない。カタカナ英語が普通の語はカタカナで書く（ツール、バージョン、イベント、リトライ、タイムアウトなど）。
- git のコミットと push をしない。例外は B の取り込みのマージ（B.2）で、それも指示する側が許したときだけ行う。
- 元のリポジトリ（`~/rulec`、`~/dandori`、`~/koyomi`、`~/chobo`、`~/geas`、`~/yurai`（yuen の前の名前のまま）、`~/sakai`）に書かない、そこでビルドしない、git の状態を変えない。履歴や古いバージョンが要るときは、作業場所に `git clone --no-local` でコピーして使う。ほかの木（`~/pixie` など）も同じ。
- Rust は手元の stable 1.94.1 で通すこと。言語のクレートの edition は変えない（dandori は 2021、ほかは 2024）。新しいクレートは 2024。
- 依存は DESIGN 3.1 の表のとおり。外のクレートは serde_json だけで、土台の層と rulec と geas は外のクレートに依存しない。
- 各言語のコマンドの振る舞い（コマンド、フラグ、終了コード、診断のコード、`--format json` と `api` の形）を変えるときは、その言語の DESIGN.md に理由を書き、報告に並べる（DESIGN の P6）。
- テストは `cargo test`。報告の前に `-- --nocapture` で SKIP の行を読み、数と理由を報告に書く。この機械にツールがそろっていれば SKIP は 0 で、0 にならないものは段階ごとに許したものだけ。
- golden の取り直しは `<名前>_BLESS=1`（C から `RITSU_BLESS=1` も）。取り直したら差分を読んでから報告する。
- サーバー、クラスタ、コンテナ、ブラウザを立てたまま終わらない（Temporal の dev server、kind の上のワークフロー、LocalStack、PostgreSQL、TigerBeetle、Chrome）。一時ファイルは段階の作業場所に置き、終わったら消す。
- 成果物（文書、golden、生成物）に、手元の絶対パス、ユーザー名、マシン名を入れない。
- 文書に載せる出力と数は、実際に走らせたものを貼る。

### 0.2 この機械で気をつけること（macOS arm64）

ツールの場所は、テストが環境変数で受け取る。この機械での値は、段階ごとの指示書にある。

- ディスクの空きは 48 GB ほど。ワークスペースの `target/` は大きくなる（rulec の元の木の `target/` は 26 GB あった）。要らなくなった `target/` は消し、Go は `-trimpath` を付け、ビルドキャッシュは作業場所に置く。
- Python の venv は `uv venv --python 3.13 <場所>`（Homebrew の 3.14 の venv には pip が入らない）。
- macOS には `timeout` が無い。子プロセスの時間切れは、テストの Rust の側で決めて kill する。
- PostgreSQL は常駐のものを使わず、`initdb` と `pg_ctl` で使い捨てのクラスタを立てる。ソケットのパスは 103 バイトまで。
- Chrome はヘッドレスで書き出したあとも終わらないことがある。時間を区切って kill し、`--user-data-dir` に一時ディレクトリを渡して、終わったら消す。
- Java（OpenJDK 27）は PATH に無い。
- dandori の重いテスト（Temporal、Argo、LocalStack）は、ほかのクレートのテストと同時に走らせない。落ちたら、まず `kubectl --context kind-dandori -n argo get workflows` を見て、残っていれば消す。`TYPESAFE_API_KEY` が環境にあると、dandori のテストが本物の TypeSafe に送る。送らないなら空にして走らせる。

### 0.3 段階をまたぐ約束

- クレートの名前と置き場所は DESIGN 2.2 のとおり。言語のクレートのパッケージの名前とバイナリの名前は変えない。
- 依存の決まりは DESIGN 3.1。C の段階からは `cargo xtask deps` が確かめる。
- `naming.tsv`（36 行。D.6 で 42 行になった）は、C から `crates/ritsu-base/tests/fixtures/naming.tsv` にあり、yuen と sakai のコピーは消す。表を直すときは、この一つを直す。
- バージョンは F まで各クレートのいまのまま（DESIGN 13.1）。
- 段階の報告は、指示書の「報告の形」の六項目で返す。

## 1. ディレクトリ

```
ritsu/
  Cargo.toml  Cargo.lock  .gitignore  LICENSE-MIT  LICENSE-APACHE  DESIGN.md  PLAN.md      （B）
  tools/import.sh                                                                     （B）
  crates/rulec/ dandori/ koyomi/ chobo/ geas/ yuen/ sakai/                             （B。元の木のまま）
  crates/ritsu-base/ ritsu-testkit/ ritsu-proto/ ritsu-emit/  crates/xtask/             （C）
  .github/workflows/  ci/skips/                                                       （C）
  crates/ritsu-units/ ritsu-ports/                                                    （D）
  crates/ritsu-project/ ritsu-cross/ ritsu/   ritsu.ctx                                 （E）
  crates/ritsu-wasm/  crates/ritsu-model/  proofs/  skills/ritsu/  website/  packaging/  action.yml  README.md  README.ja.md   （F）
```

## 2. 段階 B：取り込み

`~/ritsu` を作り、七つのリポジトリを履歴ごと `crates/<名前>/` に取り込み、ワークスペースとして全部のクレートをビルドし、全部のテストを通す。この段階では、取り込んだクレートの中身を一字も直さない。直すのは、根に新しく置くファイル（`Cargo.toml`、`Cargo.lock`、`.gitignore`、ライセンス、DESIGN.md、PLAN.md、`tools/import.sh`）だけである。

この段階では、yuen は取り込んだときの名前 yurai で `crates/yurai/` にあった（C の最初に改めた。DESIGN 2.2）。この章では、いまの名前 yuen で書く。B のコミットの題と、元のリポジトリ（`~/yurai`）は前の名前のままである。

### B.1 始める前に

- DESIGN の 2 章と 12 章を読む。
- 七つの元のリポジトリについて、作業ツリーに変更が無いことを、状態を変えない形で確かめる（`GIT_OPTIONAL_LOCKS=0 git -C ~/<名前> status --porcelain` が空）。それぞれの main の先頭のコミット（ハッシュ、題、日時）を報告に書く。A の段階のあとにコミットが増えていれば、それも報告に書く。
- `git filter-repo` と、Rust 1.94.1 と、ディスクの空きを確かめる。
- yuen の二つのテスト（B.7）をどう扱うかを、指示する側の答えで知ってから始める。

### B.2 履歴を取り込む

`tools/import.sh` を書く。七つのリポジトリについて、DESIGN 12.2 の手順を順に行う（rulec、dandori、koyomi、chobo、geas、yuen、sakai の順）。

1. `git clone --no-local ~/<名前> <作業場所>/import/<名前>`
2. `git -C <作業場所>/import/<名前> filter-repo --to-subdirectory-filter crates/<名前> --tag-rename '':'<名前>/'`
3. 取り込む先のリポジトリで `git remote add <名前> <作業場所>/import/<名前>`、`git fetch <名前> --tags`、`git merge --allow-unrelated-histories -m "chore: bring <名前> into the workspace with its history" <名前>/main`、`git remote remove <名前>`

取り込む先のコミットは、次の三つの段になった。A の段階では、根の `Cargo.toml`、`Cargo.lock`、`tools/import.sh` をマージのあとの別のコミットにする案だったが、根のファイルは全部、最初のコミットに入れた。`tools/import.sh` は取り込む先のリポジトリに置くスクリプトで、作業ツリーに変更があると止まる作りなので、どのみちマージより前にコミットしておくことになる。

1. 最初のコミット：`LICENSE-MIT`、`LICENSE-APACHE`（dandori のものをコピーした。著作権者は同じ）、`DESIGN.md`、`PLAN.md`（A の段階の二つ）、`.gitignore`（`/target`）、ワークスペースの根の `Cargo.toml`、`Cargo.lock`、`tools/import.sh`（B.3）。題は `build: start ritsu, one workspace for seven small languages`。
2. 七つのマージ。題は `chore: bring <名前> into the workspace with its history`。
3. yuen の二つのテストの直し（B.7 の (i) と (ii) の間をとったもの）：`tests/cli.rs` の `the_json_of_check` と、DESIGN.md の `$ yuen check tests/mutants/E302_条が変わった` とその直し方の三行に `--root .` を足した。題は `test(yurai): pass --root . where the root was taken to be the crate`。回り道の表示は C.0 に残した。

まず作業場所のコピー（`<作業場所>/ritsu-try`）で、全部を通して走らせる。そこで次を確かめる。

- コミットの数が、七つの元のコミットの数（355、52、2、1、6、1、1 で 418。B.1 で増えていれば足す）に、マージの七つと、最初と最後のコミットの二つを足した数（427）になる（実際に 427 になった）。
- タグが 29（`rulec/v0.1.0`〜`rulec/v0.22.1` の 28 と `dandori/v0.1.0`）で、根の名前空間に `v0.1.0` などが無い。
- 各クレートの、元の HEAD で追っているファイルと、取り込んだ HEAD の `crates/<名前>/` の下のファイルが、同じ並びで同じ blob のハッシュを持つ（`git -C ~/<名前> ls-files -s` と `git ls-tree -r HEAD crates/<名前>`）。
- 各クレートで三つのファイルについて、`git log --format=%h -- crates/<名前>/<パス> | wc -l` が、元のリポジトリの `git log --format=%h -- <パス> | wc -l` と同じ（`--follow` なしで）。
- 作者と日時が元のまま（`git log --format='%an %ae %ad'` の並びが元と同じ）。

`~/ritsu` に対しては、作業場所のコピーで B.3〜B.6 を済ませてから、同じスクリプトを走らせる（コミットは実際の時刻で行い、日時を書き換えない）。

### B.3 ワークスペースの根

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
license = "MIT OR Apache-2.0"
repository = "https://github.com/i2y/ritsu"

[profile.release]
strip = true

[profile.test.package.koyomi]
opt-level = 2
```

- 五つのクレートの `[profile.release] strip = true` と、koyomi の `[profile.test] opt-level = 2` を根に置いたもの（ワークスペースの中のクレートの `[profile.*]` は読まれない）。`[profile.test.package.koyomi]` が koyomi のテストに効くことを、ビルドの出力で確かめる。効かなければ、`[profile.test] opt-level = 2` を全体に置くかを指示する側に聞く。
- クレートの中の `[profile.*]` は残る（中身を直さないため）。cargo が出す「無視する」の警告は、報告に並べる。
- `Cargo.lock` は通信せずに作る（`cargo generate-lockfile --offline`）。五つの `Cargo.lock` の依存は同じバージョンなので、取ってくるものは無い。クレートの中の `Cargo.lock` は使われないが、消さない（C で消す）。
- `[workspace.package]` は、この段階ではどのクレートも参照しない（中身を直さないため）。

### B.4 ビルド

- `cargo build --workspace --locked --offline` がエラー無く通る。警告は数えて報告に書く（元の木で出ていた警告と、プロファイルの警告を分けて）。
- 七つのバイナリ（`target/debug/rulec` など）ができる。

### B.5 外のツールを入れ直す

gitignore したもの（各クレートの `tools/` の `node_modules` と venv、Java の jar、TigerBeetle、`go-arch-lint`、ReqIF のスキーマ、rulec の `website/` のコピー、dandori の `website/docs-ja/` のコピー、rulec の `proofs/.lake`）は、取り込んでも来ない。各クレートの README、PLAN、`tools/` の README と `requirements.txt` の頭に書いてある手順で、`crates/<名前>/` の下に入れ直す。元のリポジトリに入っているものを指して使わない（元の木の venv を走らせると `__pycache__` を書くことがある）。

| クレート | 入れるもの |
|---|---|
| rulec | `website/rulec/sync.sh`（CI と同じく、テストの前に）、`proofs/` で `lake build`、Connect・mypy・NumPy・ruff の入った venv（rulec の CI の一覧）、JDK、使い捨ての PostgreSQL（`PGHOST`・`PGPORT`・`PGDATABASE` で渡す）、buf、node、ruby（rbs と steep）、php、go、swiftc。何を入れるかの正は根の `.github/workflows/tools.yml` の rulec の組 |
| dandori | `npm install --prefix` で `tools`、`tools/temporal`、`tools/durable`、`tools/agents`、`tools/wire`、`tools/mermaid`。venv は `tools/temporal-python`、`tools/pydantic-graph`、`tools/agents`、`tools/wire`、`tools/connect`（作り方は各 `requirements.txt` の頭）。`tools/temporal-go` で `go mod download`。kind のクラスタ（`tools/argo/setup.sh`。この機械にあれば使う）と argo CLI、LocalStack 4.14.0 のイメージ、Chrome。rulec は 0.22.0（B.7） |
| koyomi | `npm ci --prefix tools`（tsc）、`tools/.venv`（mypy）、PostgreSQL のバイナリ（`KOYOMI_PG_BIN`、`KOYOMI_PG_SOCKET_DIR`）、go、rustc、Chrome |
| chobo | `npm ci --prefix tools/runner` と `tools/runner/.venv`、`npm ci --prefix tools/mermaid`、`tools/tigerbeetle/fetch.sh`、PostgreSQL のバイナリ（`CHOBO_PG_BIN`、`CHOBO_PG_SOCKET_DIR`）、go、`TMPDIR` を作業場所に |
| geas | `GEAS_PIXIE_GREETER`（pixie の木ではビルドしない。場所は指示書で渡す）、Chrome、LLVM のツール（`GEAS_LLVM_BIN`）、go、node、python3 |
| yuen | `tools/.venv`（`tools/requirements.txt`、`YUEN_PYTHON`）、`tools/reqif/fetch.sh`（`YUEN_REQIF_XSD`）、xmllint |
| sakai | `tools/.venv`（import-linter）、`npm ci --prefix tools`（dependency-cruiser）、`tools/java/fetch.sh`、`tools/cml/fetch.sh`、`tools/go/install.sh`、`SAKAI_JAVA` と `SAKAI_JAVAC`、buf。`SAKAI_RULEC`・`SAKAI_KOYOMI`・`SAKAI_CHOBO`・`SAKAI_DANDORI` には、B.4 で作ったワークスペースのバイナリを渡す |

### B.6 テストを回す

- 一つのクレートずつ、そのクレートのディレクトリで回す：`cd crates/<名前> && cargo test --no-fail-fast -- --nocapture`。rulec の `.cargo/config.toml`（`RULEC_LANG=ja` を強いる）は、そのディレクトリで走らせたときだけ読まれる（DESIGN 12.4）。dandori は、ほかと同時に回さない。
- テストの並びを元と比べる：各元のリポジトリを作業場所にコピーし（`git clone --no-local`）、`cargo test --target-dir <作業場所>/target-base -- --list` で、走らせずにテストの名前の一覧を取る。取り込んだクレートの `cargo test -- --list` と、同じでなければならない。
- 落ちたテストがあれば、作業場所の元のコピーで同じテストを走らせ、元でも落ちるか（揺れか、元からか）、取り込んだせいかを分ける。dandori の Argo と Temporal の揺れは、DESIGN 10.6 のとおり一度だけ回し直してよく、回し直したことを報告に書く。
- 各クレートの通った数、SKIP の行と理由、かかった時間を報告に書く。

### B.7 分かっている食い違い

A の段階に測ったもの（DESIGN 12.4、12.5）。

- **yuen の二つのテスト**：`tests/cli.rs` の `the_json_of_check` と、`tests/design.rs` の `every_command_in_design_prints_what_design_shows` は、git のルートが `~/ritsu` に移るだけで落ちる。扱いは二つのどちらかで、B を始める前に指示する側が選ぶ。
  - (i) B の中で、テストと文書だけを最小に直す：`tests/cli.rs` の `the_json_of_check` の呼び出しに `--root .` を足し、DESIGN.md の `$ yuen check tests/mutants/E302_条が変わった`、`$ yuen check tests/fixtures/period`、`$ yuen check tests/fixtures/period --lang ja` に `--root .` を足し、E302 の直し方の三行（`yuen review tests/mutants/E302_条が変わった --root . --at …`）も合わせる。出力は元と同じになる（A の段階で確かめた）。ツールのコードは直さない。
  - (ii) B では直さず、二つを「ルートが移ったために落ちる」と報告し、C.0 で直す（表示のパスの回り道も一緒に）。
  勧めは (ii) で、B の「中身を直さない」を字のとおりに保ち、落ちる理由が取り込みにあることを報告で示す。
- **sakai の `tests/cli.rs` の `check_exit_codes_and_formats`**：通るが、`--root` なしの形を確かめる部分を、クレートのディレクトリに `.git` が無いので黙って飛ばす。報告に書き、C.0 で直す。
- **dandori が使う rulec**：dandori の golden は rulec 0.22.0 で取ってある。ワークスペースの rulec（0.22.1）ではなく、0.22.0 を `DANDORI_RULEC` で渡す。作り方は、作業場所に rulec を `git clone --no-local` でコピーして `v0.22.0` を `cargo build --release --target-dir <作業場所>/…` で作るか、GitHub のリリースのバイナリを取る。
- **rulec と dandori のサイトのコピー**：rulec は `website/sync.sh`、dandori は `website/build.sh` がコピーするページを、テストの前に作る（それぞれの CI と同じ）。

### B.8 B の完了の条件

1. `~/ritsu` に、七つの履歴が `crates/<名前>/` の下に、元の作者と日時のまま入っている（B.2 の確かめが全部通る）。タグは 29 で、どれもツールの名前で始まる。
2. 各クレートの、HEAD で追っているファイルが、元の HEAD と同じ blob のハッシュを持つ（取り込んだクレートの中身を一字も直していない）。B.7 で (i) を選んだときは、直した二つのファイル（yuen の `tests/cli.rs` と `DESIGN.md`）だけが違い、その差分を報告に貼る。
3. `cargo build --workspace --locked --offline` が通る。
4. 各クレートの `cargo test -- --list` が、元のものと同じ。
5. 各クレートのテストが、そのディレクトリで全部通る。SKIP は 0。ただし、次は許す：`TYPESAFE_API_KEY` を空にしたときの Jev の SKIP、Ollama が無いときの SKIP（どちらも dandori）。B.7 で (ii) を選んだときの yuen の二つは、落ちたまま報告する。
6. 立てたものが残っていない（`ps` で、Temporal の dev server、テストが立てたヘッドレスの Chrome（`--user-data-dir` が一時ディレクトリのもの）、PostgreSQL、TigerBeetle が無い。kind の上にワークフローが無い。LocalStack のコンテナが無い）。作業場所のコピー（`import/`、`ritsu-try/`、`target-base/` など）を消した。
7. 報告に、元のリポジトリの先頭のコミット、取り込みの確かめの結果、クレートごとの通った数と SKIP と時間、警告の数を書く。

## 3. 段階 C：土台

1.2 の重なりを土台の層へ移す。順は、koyomi・chobo・geas・yuen・sakai から先に、rulec と dandori は合うところだけにする。どのクレートも、移す前と後で、テストの結果と golden が同じであること（変えると決めたものを除く）を確かめながら進める。

### C.0 B から持ち越したもの

C の最初に、yurai を yuen に改めた（DESIGN 2.2。`git mv crates/yurai crates/yuen`、パッケージとバイナリ、`YURAI_` の環境変数、文書と診断と golden、名指しのツールの語。sakai の名指しと `naming.tsv` の行も）。golden は、名前を戻せば元と一字も同じになることを確かめた。書き出しの識別子だけはハッシュなので値が変わり、元の識別子と一対一に対応すること（150 の識別子）を確かめた。名前を含む yuen の例のファイル（`payment` のコードと約款）はハッシュで固定してあるので、改めたあとのハッシュで固定と記録を書き直した。

C.0 でしたこと：

- yuen の表示のパスの回り道を直した（`Project::shown` が、走らせたディレクトリからいちばん短い相対で書く）。ルートが走らせたディレクトリより上にあるときのテストを足した（`tests/cli.rs` の `a_root_above_where_yuen_runs`。直す前の形では落ちることも確かめた）。この計画は「DESIGN.md の例のコマンドに `--root` を渡す形にする」と書いていたが、そうしなかった。表示を直せばテキストの出力はルートの場所によらなくなるので、B で `--root .` を足した E302 の例は、元の形（`--root` なし）に戻した。この例は、ルートがクレートの二つ上にあるワークスペースの中で DESIGN のテストに走らされるので、直したことの確かめにもなる。JSON（`root` とルートからのパスを書く）を見るテストだけが `--root` を渡す。
- sakai の `tests/cli.rs` の `check_exit_codes_and_formats` は、一時ディレクトリに `.git` を作り、その二つ下から `--root` なしで走らせる形にした（`api` の `map.file` で、どこがルートになったかを確かめる）。`.git` が無いときは、渡したファイルのディレクトリがルートになることも確かめる。
- クレートの中の `Cargo.lock`（七つ）と `[profile.*]`（五つ）を消した。koyomi の DESIGN.md の、テストのビルドを `opt-level = 2` にしたという項に、いまは根の `[profile.test.package.koyomi]` にあると書き足した。
- rulec のテストは、rulec を走らせるところで自分で `RULEC_LANG=ja` を渡す（`Command::new(env!("CARGO_BIN_EXE_rulec"))` のあと。`--lang` か `RULEC_LANG` を自分で渡すところは、そのまま）。rulec を走らせるスクリプト（`tests/make-mutants.sh`、`website/` の確かめ）にも渡す。プロセスの中で日本語の文を読むテスト（`apply`、`coverage`、`defset`、`m0`、`sources` の全部と、`golden` の描くところ）は、はじめに `rulec::i18n::set(Lang::Ja)` を呼ぶ。`src/sources.rs` の単体テスト一つは、どちらの言語でも通る形にした（`revision_words` の文の頭）。`.cargo/config.toml` を消し、それに触れていたテストの注と、rulec の `ci.yml` の注を直した。根から `cargo test --workspace` で回り、環境に `RULEC_LANG=en` があっても通る。
- 各クレートの `Cargo.toml` の `license` と `repository` は、まだワークスペースのものにしていない（F で決める）。yuen の `repository` は、改名で機械的に `https://github.com/i2y/yuen` にした。このリポジトリは作らない（配るのは ritsu だけ）ので、F で全部のクレートの `repository` を決めるときに一緒に直す。（F.7 で、言語の七つのクレートを `repository.workspace = true` にした。rulec の `homepage` はサイトを切り替えるまで残した。DESIGN 13.2）

### C.1 `ritsu-base`

DESIGN 4 章のモジュールを作る。どれも std だけで書く。C の最初の部分で作った（形と決めたことは DESIGN 4.12）。koyomi・yuen・sakai の `explain`（テキストと Markdown）、koyomi の `--help`、yuen の `review --help` の出力を `tests/golden/compat/` に保存し、土台で組み直したものと一字も違わないことをテストで確かめた。`naming.tsv` は `crates/ritsu-base/tests/fixtures/naming.tsv` にコピーし、yuen と sakai のコピーと同じバイト列であることもテストで確かめる（C.7 と C.8 で二つのコピーを消すまで）。

| モジュール | 元にするもの | テスト |
|---|---|---|
| `text`（`Lang`、`Text`、`tr!`、幅と詰め） | koyomi・yuen・sakai の `src/i18n.rs` | 幅（W と F を 2）、`Lang` の選び方（`--lang`、`<名前>_LANG`、`RITSU_LANG`、英語） |
| `diag`（共通の部分と、言語ごとの部分のトレイト、テキストと JSON） | koyomi・chobo・yuen・sakai の `src/diag.rs` | 英語と日本語の golden、JSON のキーの順、`root` |
| `ledger`（`Entry`、`find`、書き出し、再現を走らせるテストの共通部分） | koyomi・yuen・sakai の `src/codes.rs` の後ろ半分 | Markdown のアンカー、関連するコードのリンク |
| `cli`（`Flag`、`Cmd`、`--help`、読み取り） | koyomi・yuen・sakai の `src/cli.rs` | 知らないフラグ、閉じた集合の外の値、値の無いフラグ、二度目のフラグがどれも exit 2 |
| `sha256` | koyomi の `src/sha256.rs` | FIPS 180-4 の既知の値（空、`abc`、長い入力） |
| `naming`、`paths` | yuen の `src/names.rs`、sakai の `src/naming.rs` と `src/paths.rs` | `naming.tsv` の 36 行が全部、yuen と sakai のいまの表と同じ結果。ルートの決め方。表示のパスがいちばん短い相対になる |
| `sources`（引用から要素の名前、コピーの場所と本文、固定、固定の行の書き換え、curl、e-Gov、eCFR、base64） | rulec の `src/sources.rs` の 20〜313 行と 1047〜2142 行、koyomi の `src/fetch.rs` と `src/sources.rs`、yuen の `src/fetch.rs`・`src/copies.rs`・`src/base64.rs` | 三つのリポジトリのテストにある、要素の名前と本文の例を全部。通信は小さな HTTP サーバーで（yuen の `tests/fetch.rs` の形）。本物の e-Gov と eCFR には、`RITSU_TEST_LEVEL=platforms` のときだけ問い合わせる |
| `docpage`（HTML の枠と CSS、Markdown の頭） | koyomi の `src/doc/html.rs`、dandori と chobo と rulec の CSS | 明るい配色と暗い配色、外の URL を読まない |
| `json`（値の型、読み書き、キーの順） | rulec の `src/json.rs` | 読んで書いて同じになる、整数が正確 |

### C.2 `ritsu-testkit`

DESIGN 10.8 のものを作る：自分を消す一時ディレクトリ（終わったプロセスの分も消す）、ツールを探す（`RITSU_<ツール>`、いまの `<クレート>_<ツール>`、既定の場所、PATH）、時間を区切って走らせる、golden と取り直し（`RITSU_BLESS` と `<名前>_BLESS`）、SKIP（`SKIP: <クレート>: <理由>` と `RITSU_SKIP_LOG`）、段（`RITSU_TEST_LEVEL` と `need(Tool::…)`）、使い捨ての PostgreSQL、TigerBeetle、Chrome、Mermaid。元は五つの `tests/common` と、dandori・koyomi・chobo の `tests/doc.rs` の Chrome と Mermaid のところ。C の最初の部分で作った（DESIGN 10.9）。段の型は `Need`（`need(Need::Postgres)`）にした。テストの中の小さな HTTP サーバー（yuen の `tests/fetch.rs` の形）を足し、`ritsu-base` の出典のテストが使う。

### C.3 `xtask`

- `cargo xtask test [--level fast|tools|platforms] [--changed <リビジョン>] [-p <クレート>]`：段とクレートを選んで回し、最後に SKIP の表を出す。`ci/skips/<段>.txt` に無い SKIP があれば exit 1。
- `cargo xtask deps`：`cargo metadata` を読み、DESIGN 3.1 の表と突き合わせる。破っている依存を名指して exit 1。
- `.cargo/config.toml`（根）に `[alias] xtask = "run --package xtask --"`。
- C の最初の部分で作った（DESIGN 10.9）。SKIP の記録に理由の種類（`level` か `missing`）を足し、`ci/skips/<段>.txt` と突き合わせるのは `missing` だけにした。`ci/skips/` のファイルは C.12 で書く（無ければ何も許さない）。`--changed` は、変わったファイルのクレートと、それに依存するクレートを回す。文書だけの変更を見分けることはまだしない。

### C.4〜C.8 五つを土台へ

どれも、そのクレートの `sha256`、二つの言語の文、診断の共通の部分、台帳の枠、CLI の表、テストの共通部分を、土台のものに替える。言語ごとに替えるものと、気をつけることは次のとおり。

| | 替えるもの | 気をつけること |
|---|---|---|
| C.4 koyomi | 上の全部、出典の法令の部分、doc のページの枠、生成物の予約語（C.10） | `tr!` と `Text` は koyomi の形が元なので、ほとんど名前を替えるだけ。祝日の表（`src/holidays.rs`、`src/sjis.rs`）は koyomi に残す |
| C.5 chobo | 上の全部、`src/ids.rs` の SHA-256、doc の枠、生成物の予約語（C.10） | 診断の JSON のキー（`column` → `col`、`title` → `message`。`excerpt`・`operations`・`hint` は chobo の部分に）。変える理由を chobo の DESIGN.md に書く |
| C.6 geas | 上の全部、`src/json.rs`、ルートの決め方（`src/tree.rs`） | 外のクレートに依存しないまま。`t(en, ja)` を `tr!("日本語", "English")` に替える（文は一字も変えない。機械的に替えたあと、英語と日本語の golden が同じことで確かめる）。SHA-1（`src/hash.rs`）は geas に残す。DESIGN.md は英語のまま |
| C.7 yuen | 上の全部、`src/names.rs`（→ `naming`）、`src/sources.rs` と `src/fetch.rs` と `src/copies.rs` と `src/base64.rs`（→ `sources`） | 診断の JSON はもうルートからの相対。`root` を外側に足す |
| C.8 sakai | 上の全部、`src/naming.rs` と `src/paths.rs`、`src/proto.rs`（→ `ritsu-proto`、C.9） | 診断の JSON の `file` を、走らせたディレクトリからルートからの相対に替え、`root` を足す（DESIGN 6.2 の 9）。変える理由を sakai の DESIGN.md に書く |

C.4〜C.8 でしたこと（C の二つ目の部分）：

- 五つとも、土台の `sha256`、`text`（`tr!`・`Text`・`Lang`）、`diag`（`Diag<X: Extra>`）、`ledger`、`cli` と、テストの共通部分（`ritsu-testkit` の一時ディレクトリ、ツールの探し方、時間を区切った実行、golden、SKIP、段、PostgreSQL、TigerBeetle、Chrome、Mermaid、小さな HTTP サーバー）に替えた。言語ごとに残したものと、出力が変わったところの理由は、その言語の DESIGN.md に書いた（koyomi 11.1、chobo 8.1、geas 13.1、yuen 16.1、sakai 2.4・5.1・12.1）。
- 一つ替え終えるたびに、その言語のテストを全部回し、コマンドの出力を、例と fixture の全部について替える前と比べた（koyomi 352 回と生成したファイル 80 個、chobo 273 回とファイル 72 個、yuen と sakai は全部のコマンド、geas は `--help` と `explain`）。違ったのは、下の表の決めて変えたところだけだった。

| | 言語に残したもの | 出力が変わったところ | 取り直した golden | テスト |
|---|---|---|---|---|
| C.4 koyomi | 計算の段（`Example`）、祝日の表、コピーのうち本則の条だけを引くこと、`text_diff`、ページの中身と koyomi だけの色、台帳とコマンドの表の中身 | `--help` と `--lang` の説明（`RITSU_LANG`）、HTML の共通の役割の色の値、E001 の JSON の `line` と `col` が null、curl の `--compressed` と e-Gov の取り直し、コピーの本文の表の読み方、固定の行の書き換えの `\"` | `docs/images/doc-{top,months}.{en,ja}.png`、`docs/reference.md` とスキルのコピー | 99（C.10 で 100） |
| C.5 chobo | 操作とヒント（`Ops`）、`--help` の組み立て、台帳と `explain` の形、Markdown の頭、ページの中身 | `check --format json` の診断のキーと外側の `v`、`--lang` の説明、`doc` の `generator` と配色の変数の名前 | `tests/doc/*.html`（14）、`docs/formats.md` とスキルのコピー、`skills/chobo/SKILL.md` | 63 |
| C.6 geas | コマンドの表（DESIGN 4.4 が外している）、台帳と `explain` の形、`src/json.rs`、SHA-1、歩くときに飛ばす名前 | E081 の JSON の `line` と `col` が null、診断の JSON の `\b` と `\f`、`RITSU_LANG`、SKIP の行、一時ディレクトリの場所 | `tests/golden/en/errors/E081-missing.json` | 236 |
| C.7 yuen | `.req` の字句と構文、`Trail`、名指しの診断のコードと文（E011〜E013）、ReqIF と PROV の書き出し | 書き出しの識別子、`RITSU_LANG`、表示のパス、歩くときに飛ばす名前、curl と e-Gov の取り直し、JSON が読めないときの理由の文 | `tests/golden/export/{ecfr,payment,period}.{reqif,provn,prov.json}`（9） | 94（`#[test]` は 95 から 94。base64 の RFC 4648 の例のテストを ritsu-base に移した） |
| C.8 sakai | `.ctx` の字句と構文、`Refs`、名指しの診断の文、地図の検査、`build`、`export cml`、`api` | `--format json` の `file` をルートからの相対にし、`root` を足した、`RITSU_LANG`、SKIP の行 | なし | 94（SHA-256 の一つと文の二つのテストは ritsu-base にある。C.9 のあと 90） |

- 土台に足したもの（すでに替えた言語のテストも回し直した）：`ledger` に `Entry::beside` と `Entry::later`（koyomi と yuen の、隣に置くファイルと、まだ無い再現）。`cli` に、手で書いた使い方の行（`Cmd.usage`。chobo）、何が誤りかの種類（`Misuse` と `Table::read`。chobo が自分の文で言うため）、`Reading` の二つ（`no_inline_values`、`single_dash_args`。chobo の読み方）、`lang_flag` の説明に `RITSU_LANG`（全部の言語。7.3 のとおり）。`diag` に `fixed_line`（koyomi と sakai のテスト）、`Extra::fix_key`（geas の JSON は `fix` のキーを持たない）、ファイルの無い診断は場所を書かず、JSON の `file` を null にすること（geas のコマンドラインの診断）。`ritsu-testkit` に `TempDir::exists`（chobo）。
- yuen の書き出しの識別子（C.7）：SHA-256 を一度だけかける形に直した（yuen の DESIGN 12 章に書いたとおり。実装はダイジェストにもう一度かけていた）。頭の `yuen/1` は変えていない。公開する前で、yurai から yuen への改名で識別子はすでに全部変わっていたので、直すのはいまがいちばん安い。golden（三つの例の三つずつ）を取り直し、識別子の値のほかは一字も変わらないこと、古い識別子と新しい識別子が一対一に対応すること（150 個）を差分で確かめた。DESIGN 12 章と 13 章の例も取り直した。
- 決めたこと（★は作者に確かめたい）：
  - ★ chobo の `check --format json` の外側の `v` を 1 から 2 にした。診断のキーを替えたので、読む側が形の違いを見分けられるように。
  - ★ geas の `src/json.rs` は残した。geas は試すプログラムが出す数（小数と指数）を記録にそのまま書き、`ritsu-base` の `json` は指数を読まないので、替えると記録が読めなくなる。
  - geas のコマンドの表は替えていない（DESIGN 4.4 が土台の表から外している）。chobo と geas の台帳と `explain` は、形がほかの言語と違うので、それぞれのまま残した（chobo は土台の `Severity` だけを使う）。
  - koyomi と chobo の doc の印（stamp）と chobo の Markdown の頭のコメントは、いまの文のまま残した。土台の `stamp` と `markdown_head` は、まだどの言語も使っていない。
  - 行の無い診断の JSON に `0` を書いていたのは、geas のほかに koyomi の E001（UTF-8 でないファイル）と yuen もそうだった。三つとも null にした（DESIGN 4.12 を直した）。
  - yuen の表示のパスは、最初に渡したパスが絶対パスなら絶対パスで書く（土台の `Shown` の形）。歩くときに飛ばす名前は、どの言語も `site-packages` と `__pycache__` を含む土台の組になった。
  - テストの SKIP の行は `SKIP: <クレート>: <理由>` で、外のツールが要るテストは `need(Need::…)` で段を見る。言語の文を読むテストは `RITSU_LANG` を消してから走らせる。yuen の本物の e-Gov と eCFR への問い合わせは、`YUEN_NET=1` のほかに `RITSU_TEST_LEVEL=platforms` でも走る。

### C.9 `ritsu-proto`

sakai の `src/proto.rs`（型の名前の解決まで持つ）を元に、rulec の `src/proto.rs` の Protovalidate の読み方と `buf.yaml`・`buf.lock` の読み方、dandori の `src/proto.rs` の `json_name` とオプションの読み方を足す（DESIGN 4.10）。テストは、三つのリポジトリの `.proto` の fixture を全部読み、三つの読み手がいま出しているものを、新しい読み手が全部出すこと。この段階で替えるのは sakai だけ。rulec と dandori は D.10 で替える。

C.9 でしたこと：

- `crates/ritsu-proto`（std と `ritsu-base` だけ）を作った。sakai の `src/proto.rs` を元に、どの要素にもオプションを書いたまま持たせ（`Opt`：名前、値の文、protobuf のテキスト形式として読んだ値 `Value`）、次を足した。dandori の読み方：オプションを拡張の名前で引く木にすること（`value::tree`。`(a).b.c = v` は `{"a": {"b": {"c": v}}}`、同じフィールドを二度書けば並び）、import の先を何段でも読むこと（`load_from`。dandori の決まりで探し、`dandori/v1/options.proto` のように中身を渡されたファイルはその文を読む）。rulec の読み方：Protovalidate の規則（`validate`：フィールドの整数の範囲、`required`、`ignore`、並びの数、文字列の値と長さ、CEL を文字列のまま、読まなかった規則の名前。メッセージの CEL と `oneof` と `disabled`）、`buf.yaml` の `deps` と `buf.lock` の固定（`buf`）。宣言した `oneof` そのもの（名前と中のフィールドとオプション）、`syntax` を書いた行、読んだ順（`Protos::order`）も持つ。読めないときは、何が悪いか（`Problem`）を返し、文は読んだプログラムの名前を渡して作る（`ReadError::message("sakai")`。知らない `syntax` の文だけがプログラムの名前を言う）。
- テスト（`tests/readers.rs`）：三つのリポジトリの `.proto` の全部（rulec 4、dandori 20、sakai 39）と、三つの読み手が自分のテストで使っていた例（ritsu-proto の `tests/fixtures/` にコピーした 22 のファイルと、rulec のテストの `buf.yaml` と `buf.lock` の五つ）を、三つの読み手の言葉に直して比べる。rulec の読み手とは rulec の型のまま等しいこと（`package`、`imports`、`enums`、`messages`、`buf_deps`、`buf_lock`）、dandori の読み手とは `load` が出すもの（全部のメッセージと列挙、型の解決、JSON の名前、`presence`、規則の木、サービスとメソッドのオプション、読めなかった import、型が無いときの誤り）が一行ずつ等しいことを確かめる。sakai の読み手とは、替える前に一時的に sakai を dev-dependency にして、ファイルごとの読み取りと、まとめて読んだときの import の行き先と型の解決が一字も違わないことを確かめ、それを `tests/golden/sakai.txt` に残した（いまはその golden と比べる）。rulec と dandori の分も golden に残す（D.10 で古い読み手を消したあとに比べるため）。
- 食い違いは二つだけだった。どちらも新しい読み手の方が多く読む。rulec の `package` は行の頭にしか見つけないので、`syntax = "proto3"; package a.v1;` のように一行に書いたファイルでは何も返さない（テストは、rulec が何かを返したときだけ比べる）。三つのファイル（sakai の E106 の変異、dandori のテストの `.proto` でないファイル、proto2 の `group`）は、新しい読み手が読めないと言い、rulec の読み手は読めたところまでを返す（テストは、この三つが読めないことを確かめる）。
- sakai を替えた：`src/proto.rs` は ritsu-proto の型と関数をそのまま公開するだけになり、名指しの作り方（`Naming`）と、何も設定していないことを言う列挙の値の決め方（`value_prefix`、`is_unset`）だけが sakai に残った。読み手の単体のテスト四つは ritsu-proto に移した。sakai の出力は、全部のコマンド（205 回）で替える前と一字も違わない。
- 決めたこと：言語が読んだものから作るもの（rulec の列挙の値の別名と `upper_snake`、sakai の `value_prefix`、dandori が proto2 と edition を受け付けないこと、dandori の型の解決が読んだ全部のファイルから引くこと）は、言語に残した。ritsu-proto の型の解決は sakai の形（import したファイルと `import public` の先だけを見る）で、dandori の例では結果が同じだった。


### C.10 `ritsu-emit`

生成先の言語ごとの予約語（rulec の `src/backend.rs` の `words`、koyomi の `src/reserved.rs`、dandori の `src/temporal_py.rs` と `src/temporal_go.rs`、chobo の `src/client/`）、識別子の作り方、リテラル、生成物の頭（DESIGN 9.2）。koyomi と chobo を替え、生成物が一バイトも変わらないことを、各クレートの突き合わせのテスト（koyomi の五つの出力先、chobo の七つの組み合わせ）と golden で確かめる。rulec と dandori の表は、ritsu-emit にコピーして突き合わせるテストだけを置き、二つのコードは C.11 で替える。

C.10 でしたこと：

- `crates/ritsu-emit`（std と `ritsu-base` だけ）を作った。`words`：標準が並べる語を、標準ごとに一つずつ（TypeScript は ECMAScript 2025 の予約語、strict mode の予約語、strict mode で名前にできない `arguments` と `eval` と大域の値の `undefined`・`NaN`・`Infinity`、Python 3.14.6 の `keyword.kwlist` と `softkwlist`、Go 1.25 のキーワードと事前宣言の識別子、Rust 1.94 のキーワード、PostgreSQL 18.0 の `kwlist.h` と PL/pgSQL の予約語）。生成器が名前を照らし合わせる語は、いくつかの表をまとめた `Words` で表し、各言語の `NAMES` が koyomi の表と同じ語になる。`copies`：rulec の 12 の出力先ごとの三つの表と、dandori の四つの表をコピーした。標準の表と同じものはそれを指し、違うところだけを自分の表に持つ。`ident`（`pascal`、`go_package`、`go_exported`、`is_ascii_ident`、`aside`、`unique`）、`lit`（JSON の文字列、Python の `'…'`、Go の `"…"`、SQL の `'…'` と `"…"`）、`header`（`Code generated … DO NOT EDIT.` の一行と、それを書くコメント）。
- koyomi を替えた：`src/reserved.rs` は ritsu-emit の表を出力先の名前と組にするだけになり、`pascal` と `go_package`、五つの生成器の文字列のリテラル、生成物の頭の一行とコメントが ritsu-emit のものになった。生成物が使う名前（`GENERATED`、`MODULES`）は koyomi に残した。
- chobo を替えた：Python と Go のキーワード、Go の外に見せる名前、ASCII の識別子の見分け、名前を `_2` で分けること、TypeScript・Python・Go・SQL のリテラル、Go のファイルの頭の一行が ritsu-emit のものになった。生成物が自分で使う名前（`self`、`_str` など）と、TypeScript と Python のファイルの頭の文（`Written by …`）は chobo に残した。
- 確かめたこと：例の全部の `.cal` を五つの出力先に生成したファイル（80 個）と、三つの帳簿を七つの組み合わせに生成したファイル（72 個）が、替える前と一バイトも違わない。koyomi の五つの出力先の突き合わせ、chobo の生成したクライアントを本物の PostgreSQL と TigerBeetle で走らせるテスト、golden は SKIP なしで通る。koyomi に、ritsu-emit の JSON の文字列が serde_json の書くものと同じことを確かめるテストを一つ足した。rulec の表は rulec の `backend::ALL` と、dandori の表は dandori のソースの定数と（公開していないので文字で読む）、語の組として等しいことをテストで確かめる。


### C.11 rulec と dandori（合うところだけ）

- rulec：`src/sha256.rs`、`src/json.rs`（土台の `json` の元なので、置き場所が移るだけ）、`src/sources.rs` の共通の部分（20〜313 行と 1047〜2142 行。コピーと表の突き合わせ、534〜1009 行は rulec に残す）、テストの SKIP の書き方（`注意:` → `SKIP: rulec: …`）。診断（`src/diag.rs`）、CLI の表（`src/main.rs`）、`tr!` は残す（DESIGN 4.1、4.2、4.4）。
- dandori：診断の `(en, ja)` の組を `tr!` の順に（機械的に）、CLI を土台の表に移し `--version` とコマンドごとの `--help` を足す、テストの一時ディレクトリと Chrome と golden を `ritsu-testkit` に。rulec とのつなぎ（`src/rulec.rs`、`src/sources.rs`）は D まで触らない。
- どちらも、コーパスとテストの全部、rulec の証明書（`rulec certificate` の出力）、golden が一字も変わらない。

C.11 でしたこと（C の最後の部分）：

- rulec：`src/sha256.rs` を消し、`rulec::sha256` は土台の `sha256` を指す。`src/json.rs` は土台の `json` の上の薄い層になった（456 → 204 行）。残したのは、読めないときの rulec の文（土台が返す種類から選ぶ。位置の数も前と同じ）、値の種類の名前、オブジェクトをキーの順に並べて読み書きすること、`--format json` を組み立てる `Obj` である。`src/sources.rs` は、引用から要素の名前を作ること、コピーの場所と本文、固定の行の書き換え、curl と e-Gov と eCFR と GitHub への問い合わせ、base64 を土台のものにした（2,142 → 1,640 行）。コピーと表の突き合わせと、`source fetch | pin | outdated` を rulec の文で言う部分は残した。予約語の表（`src/backend.rs` の `words`）は `ritsu-emit` の `copies` から読む（998 → 812 行）。テストの SKIP の行は、29 のファイルで `SKIP: rulec: <理由>` になった。外のツールが要るテストは `ready(Need::…, 見つかるか, 理由)` で先に段を見る。理由の文は前のまま（日本語）。診断、コマンドの表、`tr!` は残した。rulec の DESIGN §15.162 に書いた。
- dandori：診断の文と注、そこに至る実行の一歩、文の切れ端の `(en, ja)` の組を、スクリプトで機械的に `tr!("日本語", "English")` に替えた（613 か所。片方が文字列でも `format!` でもない 9 か所は `Text::new(ja, en)`）。診断の型（`Diag`、`Step`）は、文を土台の `Text` で持つ。CLI は `src/cli.rs` の表（土台の `cli`）に移し、`--version` とコマンドごとの `--help` を足した。テストの一時ディレクトリ、Chrome、golden、SKIP、段は `ritsu-testkit` のものにした。予約語の表（Python、Go、TypeScript）は `copies` から読む。`src/rulec.rs` と `src/sources.rs` は触っていない。dandori の DESIGN 0.3 に書き、README、サイトのコマンドのページ（英語と日本語）、スキルのコピー（`skills/sync.sh`）を直した。
- 確かめたこと：rulec は、コーパスの全部の規則の `check`（英語・日本語・JSON）、`certificate`、`api`、`schema`、`graph`、`vectors`、`coverage`、`adapter`、`fmt --check`、`doc`（Markdown と HTML、英語と日本語）、`gen` の全部の出力先と、変異の全部の `check`、壊れた JSON Lines を渡した `fixtures lint` を、替える前と後のバイナリで出して比べ、1,227 回とも一字も違わなかった。dandori は、例とテストのフローの全部の `check`（英語・日本語・JSON）、例の `build`（七つのプラットフォーム）、`scenarios`、`doc`、`run` の 695 回を比べ、違ったのは CLI を表に移して決めて変えた 7 回（`--help` と引数の誤り）だけだった。`source outdated` は、本物の e-Gov と eCFR に問い合わせて、前と後のバイナリで比べた。テストと golden と証明書は一つも変わっていない（取り直した golden は無い）。
- 漢数字と `第0条`（7.3）：土台の `sources` に替えて、rulec は条の番号を百と千の位まで読み、`第0条` を通さなくなった。これで変わったテストや出力は、コーパスにもテストにも証明書にも無かった。どの規則も踏まない変わったこととして、rulec の DESIGN §15.162 に並べた（ほかに、JSON として読まないもの三つ、文字列の中の `\b` と `\f`、`file://` の URL を一度だけ試すこと、e-Gov の base64 を標準の形で読むこと、問い合わせが失敗したときの文）。
- 土台に足したものと消したもの（すでに替えた五つの言語のテストも回し直した）：`json` の読み手が止まった理由の種類（`Problem`。rulec が自分の文を選ぶため）。`ritsu-emit` の `copies::rulec` を出力先ごとの定数と `BACKENDS` にしたこと（rulec の `backend.rs` が並べるため）。`ritsu-testkit` の `ready` と `Need::Rulec`（rulec と dandori）。`docpage` の `stamp` と `markdown_head` は、rulec と dandori の doc が出力を変えずに使えなかったので消した（DESIGN 4.8）。
- `ritsu-emit` のテストから、rulec と dandori のいまの表と比べる部分と rulec への dev-dependency を消し、`tests/golden/copies.txt`（表ごとの語の並び）と比べるテストだけを残した（DESIGN 3.3）。
- 予約語の表の違いで、dandori の生成物が rulec の生成物の名前を参照するところに食い違いが出るかを調べた。表の違いからは出なかったが、dandori が自分で書く名前と規則の別名がぶつかるところが見つかった。直していない（7.5）。

### C.12 CI のワークフロー

`.github/workflows/` に DESIGN 10.5 のジョブ（`fast`、`tools`、`proofs`、`kani`、`platforms`）を書く。rulec の `ci.yml` を元にし、koyomi・chobo・geas・yuen・sakai のツールを足す。`ci/skips/fast.txt`、`tools.txt`、`platforms.txt` に、それぞれの段で許す SKIP を書く（`tools` は空の予定だったが、CI で用意できない geas の pixie の四つを許した。下の「PLAN と違えたところ」）。リモートが無いので走らせられない。ワークフローの YAML が読めることと、ジョブが呼ぶコマンド（`cargo xtask test --level …` など）を手元で走らせて通ることを確かめる。

C.12 でしたこと：

- 根の `.github/workflows/` に、ジョブ一つにファイル一つで `fast.yml`、`tools.yml`（三つの組の matrix）、`proofs.yml`、`kani.yml`、`platforms.yml` を書き、`ci/skips/` に三つの一覧を書いた。ジョブごとに決めたことと、クレートの中に残っている `.github/` の扱いは DESIGN 10.5 にある。
- 五つとも YAML として読め、actionlint 1.7.12 が何も言わず、46 の `run` の中身が `bash -n` を通る（protoc を足したあとにもう一度確かめた）。
- `fast`：作業ツリーと同じ中身を、まっさらに取り出した木に置き（gitignore したものは無い）、PATH には cargo と git と curl とシステムのコマンドだけを残して（ほかのツールは、呼ぶと記録を残して失敗するものに替えて）、ジョブのとおりに走らせた。コピーを作るのに 2 秒、`cargo build --workspace --locked` に 12 秒、`cargo xtask deps` は通り、`cargo xtask test --level fast` は 1,489 件が通った（ignored 1。SKIP は段で外したもの 303 で、許していないものは 0。ツールの呼び出しは 0）。テストのビルドを含めて 3 分 30 秒、ビルド済みなら 2 分 15 秒だった。
- 最初に `fast` を回したとき、段を聞かずにツールを呼ぶテストが geas に五か所、sakai に一か所あり、許していない SKIP が 20 出た（geas が Python・Node・Go・rustc・pixie の greeter、sakai が rulec・koyomi・chobo・dandori のバイナリ）。段を聞くようにし、`ritsu-testkit` に `Need::Suite`（sakai）と `Need::Pixie`（geas）を足した（DESIGN 10.9）。curl は、koyomi と yuen と土台の `sources` のテストが、自分の中に立てたサーバーに問い合わせるのに使っていた。これは fast の段で使ってよいものとして DESIGN 10.2 に書き足した。
- `tools`：この機械で、三つの組の `cargo xtask test --level tools` を順に走らせた。rulec は 727 件が通り（ignored 1。SKIP は段で外した Kani と Buf Schema Registry の二つ）、399 秒。dandori は 81 件が通り（SKIP は段で外した platforms の段の十）、92 秒。残りの組は 681 件が通り（SKIP 0。この機械には pixie の greeter がある）、252 秒。rulec の組の後ろの三つも通った：`rulec test --require-all` は 36 の側が全部合い、`skipped` は無かった。証明書の再検査は 50 本、`fmt --check` と `check` は通った。
- `proofs`：`lake build`、50 本の証明書を `rulec-recheck` で、`cargo test --release -p rulec --test lean`（12 件）が通った。
- `kani`：`cargo xtask test --level platforms -p rulec -- フラグを付ければ証明が走る` が通り（SKIP 0）、コーパスの 50 本の規則の 118 のハーネスを Kani 0.68.0 で証明した（二つで 3 分 29 秒）。
- `platforms`：この機械の kind のクラスタ（`tools/argo/setup.sh` で作ってあったもの）、argo CLI v4.1.4、LocalStack 4.14.0 のイメージで、dandori の platforms の段の十を `cargo xtask test --level platforms -p dandori -- --exact <名前>` で一つずつ回し、全部が一度で通った（SKIP は Jev の一つ。回し直しは無し。kind の上にワークフローは残っていない）。外のサーバーに問い合わせる四つも通った（SKIP 0）。数と時間は C.13 にある。
- ジョブのファイルを書いたあとで、dandori のテストが buf と protoc も使うことに気づき、`tools` の dandori の組に二つを足した（protoc は 35.1 のリリースの zip。手元の確かめは Homebrew の buf と protoc 35.1 で走らせたもの）。
- PLAN と違えたところ：★ `ci/skips/tools.txt` は空ではなく、geas の pixie の四つを許す。pixie は ritsu の外でビルドするもので、CI でその greeter を作る手段がまだ無いからである。10.5 の表に無かった外のサーバーに問い合わせるテスト四つ（土台・koyomi・yuen の本物の e-Gov と eCFR、rulec の Buf Schema Registry）は `platforms` に置いた。ほかにどのジョブも回さないからである。

### C.13 C の完了の条件

1. 根から `cargo test --workspace --no-fail-fast -- --nocapture` が全部通る。この機械では SKIP は 0（B.8 で許したものを除く）。`RITSU_TEST_LEVEL=fast` で回した時間を報告に書く。
2. `cargo xtask deps` が通る。`cargo tree -p ritsu-base`、`-p rulec`、`-p geas` に外のクレートが無い。
3. 重なりが消えている。`grep -rn 0x428a2f98 crates/*/src` が `ritsu-base` にしか当たらない。名指し、`.proto` の読み手（sakai が使うもの）、Chrome を探すコード、一時ディレクトリのコードが、それぞれ一つ。DESIGN 1.2 の表を、移したあとの行数で書き直す（見込みの 6 千行も実際の数に）。
4. golden が、変えると決めたもの（doc の CSS、chobo と sakai の診断の JSON のキー）のほかは一字も変わらない。変えたものは、各クレートの DESIGN.md に理由がある。
5. yuen の二つのテスト（C.0）が通り、表示のパスの回り道のテストがある。
6. 報告に、クレートごとの行数（前と後）、テストの数と時間、変えた golden の一覧を書く。

C.13 で確かめたこと（2026-10-03、この機械で）：

1. 根から `cargo xtask test -- --skip localstack --skip temporal --skip pydantic_graph --skip durable --skip argo --skip ollama`（`cargo test --workspace --no-fail-fast -- --nocapture` に、dandori の重い十二を外す `--skip` と、SKIP の表を足したもの）が、10 分 34 秒で 1,477 件通った（ignored 1、rulec の一つ）。SKIP は dandori の Jev の一つだけ（`TYPESAFE_API_KEY` が空。B.8 で許したもの）。外した十二は、7.2 のとおり一つずつ、`cargo xtask test --level platforms -p dandori -- --exact <名前>` で回し、全部が一度で通った（durable 17 秒、pydantic-graph 4 秒、記録した履歴の再生 17 秒、LocalStack 72 秒、Temporal の TypeScript 75 秒・Python 67 秒・Go 71 秒・言語をまたぐ 111 秒・Worker Versioning 20 秒・子のフロー 13 秒、Argo 115 秒、Ollama 28 秒。回し直しは無し）。合わせて 1,489 件で、クレートごとには rulec 727（ignored 1）、dandori 81、koyomi 100、chobo 63、geas 236、yuen 94、sakai 90、ritsu-base 58、ritsu-testkit 15、ritsu-proto 14、ritsu-emit 5、xtask 6。コンパイラの警告は rulec のテスト関数の名前の七つだけ（元のリポジトリと同じ）。`RITSU_TEST_LEVEL=fast` は、まっさらに取り出した木で 2 分 15 秒だった（テストのビルドを含めて 3 分 30 秒。C.12）。
2. `cargo xtask deps` が通る（12 のクレート）。`cargo tree -p ritsu-base`、`-p rulec`、`-p geas` に外のクレートは無い（ritsu-base は何にも依存せず、rulec は ritsu-base と ritsu-emit、geas は ritsu-base。dev-dependency は三つとも ritsu-testkit だけ）。
3. `grep -rn 0x428a2f98 crates/*/src` は `crates/ritsu-base/src/sha256.rs` にしか当たらない。名指しは `ritsu-base` の `naming`、sakai が使う `.proto` の読み手は `ritsu-proto`、テストが Chrome を探すコードは `ritsu-testkit` の `chrome::find`、自分を消す一時ディレクトリは `ritsu-testkit` の `TempDir` の、それぞれ一つになった。geas の `src/cdp.rs` が Chrome を探すのは geas のコマンドそのものの機能で、テストの重なりではない。rulec のテストは一時ディレクトリの場所を自分で作るが、自分を消す仕組みは持たない（C.11 の外。7.5）。DESIGN 1.2 の表を移したあとの行数で書き直した（土台の三つは 6,087 行で、見込みの 6 千行に近い）。DESIGN 4.9 の geas の `src/json.rs` の一文も直した。
4. C の最後の部分で取り直した golden は無い。足したのは `crates/ritsu-emit/tests/golden/copies.txt` だけである。C の全体で取り直した golden は C.4〜C.8 の表のとおりで、どれも決めて変えたもの（doc の CSS と配色、chobo の診断の JSON のキー、geas の E081 の `line` と `col`、yuen の書き出しの識別子）である。理由は各クレートの DESIGN.md にある。
5. yuen の二つのテスト（`tests/cli.rs` の `the_json_of_check` と、`tests/design.rs` の `every_command_in_design_prints_what_design_shows`）は通り、表示のパスの回り道のテスト（`tests/cli.rs` の `a_root_above_where_yuen_runs`）もある。
6. 報告に書いた。

## 4. 段階 D：型付きの境目

境目を JSON から型付きの呼び出しに替える。D の終わりには、ritsu のツールどうしが子プロセスで呼び合うところが無くなる。

### D.1 `ritsu-units`

DESIGN 5 章。rulec の `src/types.rs` の `money_unit`・`unit_info`・`unit_offset`・`CURRENCIES` と `src/num.rs` の有理数を移し、`Unit`（次元、単位、税込か税抜か、刻み）を足す。テストは、表のすべての綴りの係数、`JPY` と `円`、`USD` と `USDc` の換算、℉ の一次式、順序だけの次元、整数にならない換算をエラーにすること。rulec をこれに替え、コーパスの golden と、すべての規則の `rulec certificate` と `rulec api` の出力が一字も変わらないことを確かめる。

D.1 でしたこと（2026-10-04）：

- `crates/ritsu-units`（`ritsu-base` だけに依存）を作った。rulec の `src/num.rs` の有理数 `Rat`（`Hash` を足した）と、`src/types.rs` の表（`CURRENCIES`、`money_unit`、`unit_info`、`unit_offset`）を中身を変えずに移し、単位の型 `Unit`（次元、書いたとおりの綴り、税込か税抜か、刻み）と `Dim`、`Tax`、`Problem` を足した。形は DESIGN 5.1 の「D.1 で作った形」に書いた。テストは `tests/units.rs` の 9 本（表のすべての綴りの次元と係数、`JPY` と `円`、`USD` と `USDc` の換算と、二つの通貨は換算しないこと、整数にならない換算をエラーにすること、℉ の一次式、順序だけの次元、綴りを読んで書き戻すと同じになること、単位でない綴りの理由、有理数が正確で溢れを言うこと）。
- rulec を替えた。`num::Rat` は `ritsu_units::Rat` を指し、`unit_info` と `unit_offset` は表を引くだけになった。丸めの仕方（`RoundMode` と `round_to`）は rulec の意味なので rulec に残し、`round_to` は rulec が `Rat` に足すトレイト `RoundTo` のメソッドにした（使う六つのファイルが `use` する）。rulec の DESIGN §15.164 に書いた。
- 確かめたこと：コーパスの 50 本、変異の 109 本、ほかの 16 本の規則について、`check`（英語、日本語、JSON）、`certificate`、`api`、`schema`、`graph` を、コーパスの 50 本についてはさらに `fmt --check`、`vectors`、`coverage`、`doc`（Markdown と HTML、二つの言語、顧客向け）、`gen`、`adapter` を、替える前と後のバイナリで出し、1,782 回とも一字も違わなかった（以下、この 1,782 回を「rulec の出力の突き合わせ」と呼ぶ）。rulec の `tests/units.rs` に二本足した（単位を挙げる文が表のすべての綴りを挙げること、コーパスのどの数の型も表で書けること。206 の数）。
- 決めたこと（★）：rulec の `Ty` は、中に `Unit` を持たず、書いたとおりの綴りを持ち続ける。`Ty::unit(刻み)` で単位の型にする。DESIGN 5.2 の「D.1 で変えたこと」に理由を書いた。税の語を `incl_tax` と `excl_tax` のほかに書いた型（`money[円, foo]`）を rulec が黙って通すことは、変えなかった（7.6）。
- D の二つ目の部分で、作者が決めたとおり、税の語を誤った型を rulec が E103 でエラーにするようにした（rulec の §15.169。型を書く六つの場所のどれでも、お金の型の二つ目の語が `incl_tax` でも `excl_tax` でもなければ、その型に印を付けてエラーにする）。新しいコードを作らずに E103 にしたのは、E103 が型の書き方の誤り（読めない刻み、率の入力の刻みが無いこと）も受け持つからである。台帳の E103 の文と `docs/codes.md`・`docs/codes.ja.md` を直した。`tests/units.rs` に一本足した。rulec の出力（1,725 回）と golden は変わらない。

### D.2 `ritsu-ports`

DESIGN 3.2 の型とトレイト（`RuleFacts`、`DateFacts`、`BookFacts`、`Claims`、`Items`、`References`、`Answer`）。出す側が自分で実装する：rulec（`Rules`、`Items`、`References`）、koyomi（`Dates`、`Items`、`References`）、chobo（`Books`、`Items`）、geas（`Claims`、`Items`）、dandori（`Items`、`References`）、yuen と sakai（自分の名指しの `Items` と `References`）。

D.2 でしたこと（2026-10-04）：

- `crates/ritsu-ports`（`ritsu-base` と `ritsu-units` だけに依存）を作った。答えの形（`Answer`、`Found`、答えられないときにその言語が言う `Said`）、`Rules` と `RuleFacts`、`Dates` と `DateFacts`、`Books` と `BookFacts` と `Ledger`、`Claims`、`Items` と `Item`、`References` と `Reference` である。スケッチからの違い（問いの相手をファイルのパスにしたこと、診断の型の代わりに `Said`、`Rules::doc` を足したこと）は DESIGN 3.2 に書いた。テストは `tests/ports.rs` の 2 本（名指しの最後の組が中のものの種類と名前になること、`Said` が二つの言語の文を持つこと）。どの口もトレイトオブジェクトで持てることは、同じファイルがコンパイルで確かめる。
- 出す側の実装は、各言語の `src/ports.rs` の `Engine` にした。rulec（`Rules`、`Items`、`References`）、koyomi（`Dates`、`Items`、`References`）、chobo（`Books`、`Items`）、geas（`Claims`、`Items`）、yuen と sakai（`Items`、`References`）。geas は口に答えるために、ライブラリ（`src/lib.rs`）とコマンド（`src/main.rs`）に分けた。どの問いに答え、どの問いがまだ「決められない」と理由を言うかは、DESIGN 3.2 の表に書いた。dandori の `Items` と `References` は D.6 に残した。
- 口の事実を作るために、言語の中で JSON を書くところを、型を返す関数とそれを JSON にする部分に分けた。rulec は `codegen` の `enum_list`（`api` の列挙もここから書く）、`calls`、`connect_facts`、`cert::machine_facts`、`doc::render_named`（ページの頭に書くファイルの名前を、読む場所と別に渡す）。chobo は `api` の仮押さえの状態の表（`hold_table`。`api` の JSON もここから書く）。どれも出力は変わらない。
- `RuleFacts` は、dandori の `RuleInfo` の項目を全部作れる（規則の名前と版と SHA-256、入力と出力の名前と型、列挙、ステートマシン、前提、歩く並び。`api` の JSON から読んでいるところは、TypeScript・Python・Go の呼び方と Connect の形として型にした）。率の刻みは、説明の文からではなく rulec の型と入力の宣言から取る。
- テスト（どれも `tests/ports.rs`）：rulec 6 本（コーパスの 50 本の全部で、口の事実が `api`・`certificate`・`schema` の JSON と同じこと、50 本の 4,894 のベクタの全部で口の評価が `rulec vectors` と同じこと、口の描くページが `rulec doc` と一字も同じこと、前提の問い、検査を通らない規則の診断、ファイルが持つものと名指すもの）、koyomi 4 本（事実が `koyomi api` と同じこと、DESIGN 7.5 の例の日付の値 23 個と日数 18〜51、評価がベクタと同じこと、中のものと参照）、chobo 3 本（事実が `chobo api` と同じこと、帳簿が参照インタプリタと同じにすべてのシナリオを走ること、中のものの定義の文のハッシュが yuen の試作の計算と同じこと）、geas 3 本、yuen 2 本（要件の定義の文が、記録の確かめたハッシュと同じこと）、sakai 2 本（`means` の名指しが `sakai api` と同じこと）。
- 確かめたこと：七つの言語のコマンドの出力が、替える前と一字も違わなかった（rulec の出力の突き合わせの 1,782 回、dandori 701 回、koyomi 352 回、chobo 273 回、geas 9 回、yuen 527 回、sakai 205 回）。`cargo xtask deps` は 14 のクレートで通る。
- 決めたこと：問いの相手はファイルのパスにし、答えられないときは言語の診断の型ではなく `Said` を返し、`Rules` に `doc` を足した（理由は DESIGN 3.2）。geas は、口をライブラリの側で実装するために、ライブラリとコマンドに分けた（コマンドの振る舞いは変えていない）。rulec の `Engine` は、読んだ規則をパスと中身の SHA-256 で覚えておく（コーパスの全部のベクタを口から評価するテストが、60 秒ほどから 9 秒ほどになった）。

### D.3 dandori と rulec のつなぎ

1. 突き合わせ：rulec のコーパスの全部の規則と dandori の例の全部の規則について、`Rules::facts` で得た事実と、いまの dandori の `src/rulec.rs` が `rulec schema`・`certificate`・`api` の JSON から組み立てる `RuleInfo` が同じであることを確かめるテストを書き、通す。食い違いがあれば、どちらが正しいかを調べて報告する（DESIGN 1.4 の率の刻みのように、JSON の側が説明の文から読んでいるところは、型の側が正しい）。
2. 通ったら、dandori の `src/rulec.rs` の JSON の読み手と、`src/sources.rs` の rulec を子プロセスで呼ぶところを消し、`Rules` の口を使う。ブラウザで試すページの記録した rulec の出力（`Bundle`）も、口に替える。
3. dandori の CLI を、引数と口と出力先を受け取る関数（`dandori::cli::run`）にする。dandori のテストのうち規則を読むものは、`[dev-dependencies]` の rulec をつないでこの関数を呼ぶ形にする（DESIGN 3.3）。`DANDORI_RULEC` を消し、dandori の README と DESIGN.md を直す。
4. `rulec doc` を埋め込むところも口から。dandori の golden に入っていた `rulec 0.22.0` は、ワークスペースの rulec のバージョンになるので、一度取り直して差分を読む。

D.3 でしたこと（2026-10-04、D の二つ目の部分）：

- 突き合わせ（手順 1）：rulec のコーパスの 50 本と dandori の 14 本（例の 11 本と `tests/fixtures/rules` の 3 本）の規則で、`Rules::facts` から作った `RuleInfo` と、前の `src/rulec.rs` が `rulec schema`・`certificate`・`api` の JSON（rulec のライブラリで書いたもの）から組み立てた `RuleInfo` を、dandori が読む 1,939 の項目（名前とバージョンと SHA-256、入力と出力の名前と別名と型、列挙、ステートマシンのすべての項目、前提、TypeScript・Python・Go の呼び方、`connect` で呼ぶサービスの形）で比べた。検査を通らない規則は無かった。違ったのは 7 か所で、どれも JSON の側の読み違えだった。規則がたどる並び（`elements`）を入力の一つに数えて文字列と読んでいたこと（5 本。dandori はどちらでも E005 でエラーにする）と、無いことがある列挙の入力（`T?`）を文字列と読んでいたこと（2 本）である。率の刻み（11 列）は説明の文から読んだものと型の刻みが同じで、ステートマシン（5 本）は、どれも状態の軸を持っていた。比べたテストは、JSON の読み手を消すときに一緒に消した（報告に、そのコピーと出力を添えた）。
- 状態の軸（7.6 の申し送り）：dandori のステートマシンも軸を `Option` で持ち、None（表が状態を読まない）なら、どの状態からも同じ行が当てはまると読む。前の dandori は null を 0 と読み、最初の軸を状態の軸と取り違えて、どの呼び出しも当てはまらないとしていた。単体テストを一本足した。
- 無いことがある入力と出力（`T?`）：口は型を `Opt` で渡す。dandori は規則に `none` を渡さず、生成するコードも規則のコードとのあいだで null をやりとりしない（`Window | null` や Go の `*Window` を型の名前として読み込んで壊れる）ので、そういう規則を `use rule` が E005 でエラーにするようにした（★。dandori の DESIGN 1.13、7 章に残したこと）。例とテストの規則には、そういう規則は無い。
- JSON の読み手と子プロセス（手順 2）：dandori の `src/rulec.rs` は、口の事実を `RuleInfo` にするだけになった（678 行から 554 行。`connect_shape` も型から読む。`connect.enums` を言わない rulec のための読み方は消した）。`src/sources.rs` は、ファイルと規則の口を返す trait（`Sources::rules`）になり、子プロセスで rulec を呼ぶところを消した。ディスクから読む `Disk` は渡された口を使い、規則を読まない口 `NoRules` は `ritsu dandori …` で走らせるよう言う。生成器（`asl.rs`、`temporal.rs`、`temporal_py.rs`、`temporal_go.rs`）は、生成されたコードの呼び方を `rulec api` の JSON ではなく型（`ritsu_ports::Call`）から読む。dandori は `ritsu-ports` と `ritsu-units` に依存し、rulec は `[dev-dependencies]` にだけ持つ。
- CLI（手順 3）：コマンドの本体を `dandori::cli::run(引数, 口, 標準出力, 標準エラー)` に移し、`src/main.rs` は `NoRules` を渡して呼ぶだけにした。テストは rulec の口（`rulec::ports::Engine`）を渡して呼ぶか、ライブラリを `dandori::sources::with_rules` の中で呼ぶ。テストが要る `rulec gen` の出力は、rulec の `gen` の本体をライブラリに移した `rulec::codegen::generate` で作る（rulec の §15.170。出力は変わらない）。`DANDORI_RULEC`、ritsu-testkit の `Need::Rulec`、CI（`tools.yml` と `platforms.yml`）で rulec 0.22.0 を取ってくる段を消した。規則を読むだけのテスト（`examples_pass_check` など）は `fast` の段でも走る。`Need::Rulec` だけで段を見ていたテストは、使うツールの `need` で段を見る。`tests/fixtures/unnamed_enums`（0.21.2 の出力を記録したもの）と、それを読むテストを消した。dandori のテストに `tests/cli.rs`（三本）を足した。規則を使わないフローでクレートのバイナリと口を渡したコマンドが同じものを出し、同じものを書くこと、規則を使うフローでバイナリが E005 で `ritsu dandori` を案内すること、`cli::run` が出力を渡された先に書くことを見る。
- golden（手順 4）：`tests/doc` の golden 34 本とサイトの例のページ 10 枚（44 ファイル、60 行、78 か所）を取り直した。どれも `rulec 0.22.0` が `rulec 0.22.1` になっただけで、取り直す前のものの版の文字列を置き換えると一字も違わない。
- バイナリ（DESIGN 2.3）と、D と E のあいだ：dandori のクレートのバイナリは、規則を使わないフローならいまと同じに動き、`use rule` のところで E005 が `ritsu dandori …` で走らせるよう言う。そのままでは、規則を使う例を手で走らせる手段（dandori の README とサイトの入れ方の手順）が E まで無いので、入口の最小の形（`crates/ritsu` の `ritsu dandori`）を先に作った（★。DESIGN 8.6）。ブラウザで試すページは困らない（下）。
- ブラウザで試すページ：記録（`presets.json`）を口の答え（規則の事実と、`rulec doc` のページ）にし、記録から答える口 `Recorded` で読む。事実は dandori の形の JSON（`src/record.rs`）で持つ。`presets.json` は 1,442,820 バイトから 1,141,648 バイトになった。`website/tools/make_wasm.sh` をワークスペースの `target/` を見るように直し、`dandori.wasm` を作り直した（モジュールが 432 の問いにライブラリと同じに答えることを、テストで確かめた）。
- D.5 の残り：dandori が規則のページを描かせるとき、口の `Rules::doc` にページの言語を渡す（rulec がそのスレッドの言語で描く）。
- 確かめたこと：例とテストのフローの全部の `check`（英語・日本語・JSON）、例と `tests/flows` の `build`（七つのプラットフォーム）・`scenarios`・`doc`（Markdown と HTML、二つの言語）・`run` の 969 回を、替える前（HEAD の dandori に HEAD の rulec 0.22.1 を `DANDORI_RULEC` で渡したもの）と `ritsu dandori` で比べ、一字も違わなかった。rulec の出力の 1,725 回も、`gen` を移す前と一字も違わない。`cargo xtask deps` は 15 のクレートで通る。
- 決めたこと：★ 入口の最小の形を D で作った（DESIGN 8.6）。★ `T?` を持つ規則を E005 でエラーにする。★ 突き合わせのテストは JSON の読み手と一緒に消した（JSON の読み手を残す理由が無いため。コピーは報告に添えた）。状態の軸の None を、表が状態を読まないステートマシンとして読む。

D の二つ目の部分で、あわせてしたこと（作者が D で直すと決めたもの。7.5 の申し送り）：dandori の名前のぶつかり。

- 規則の別名（rulec の生成したコードの関数、モジュール、Go のパッケージ）と、受け取る列挙の別名が、dandori が規則のまわりに書くコードの名前とぶつかる規則を、`check` が E006 でエラーにする（dandori の DESIGN 1.15）。どの名前をエラーにするかは、生成するコードを読んで、四つのファイル（`rules.ts`、`rules.py`、Lambda の関数、`rules.go`）ごとに、そのファイルが宣言するか外から読む名前として決め、生成器の表（`AROUND_RULES`）に置いた。7.5 で見つかった五つ（`activity`、`rules`、`rule_<規則>`、`handler`・`event`・`context`）のほか、`args`、`out`、TypeScript の `Boolean`・`String`・`BigInt`・`Number`、Python の `Any`・`bool`・`str`・`int`・`dict`、Go の `context`・`ctx`・`args`・`int64`・`any`・`string`・`error`・`nil`・`true` と列挙を受け取る規則の `ok` である。7.5 の「Go ではぶつからない」は、dandori の名前（`dd` が付く）については正しかったが、規則のパッケージの名前は、`rules.go` が読み込む `context` と、関数の引数と、組み込みの名前に当たりうる。別名が同じ二つの規則（`rulec gen` の書くファイルが重なる）と、タスクと規則のアクティビティ（`rule_<規則>`）が同じ名前になるものもエラーにする。コードは新しく作らず、名前のぶつかりの E006 に入れた（★）。
- 読んでいて見つけたもの：二つの規則のあいだで名前が重なると、生成物が壊れていた。`money[JPY, incl_tax]` を受け取る二つの規則を呼ぶフローでは `rules.ts` が同じ型の名前を二度読み込んでコンパイルが通らず、同じ名前の列挙を受け取る二つの規則では `rules.py` が後のクラスで前の規則を呼んでいた。エラーにすると、ふつうのフローまでエラーにすることになるので、生成するコードの側で、重なる名前だけを別の名前で読み込むようにした（★。単位の型は一度だけ読み込む）。重なりの無いフローの生成物は変わらない。
- エラーにする範囲の外：rulec の生成したコードと出力先の言語そのもの（予約語、組み込みの名前、標準ライブラリ）のぶつかりは、rulec の W121（警告）が言うので、dandori はエラーにしない（dandori の DESIGN 1.15）。
- `dandori explain` と台帳：E006 の文を `explain` で読めるように、dandori に診断の台帳（`src/codes.rs`、30 のコードの全部）と `explain` を足した（★。台帳が無かったので、一つのコードだけを載せる形にはしなかった）。どのコードにも最小の再現があり、`tests/codes.rs` が、どの再現も自分のコードを出すこと（E040 と E050 は `build` で）を確かめる。
- テスト：`tests/names.rs`（六本）が、四つの表のどの名前にも E006 が出ること、列挙の別名、Go の `ok`、`rule_<規則>`、別名の同じ二つの規則、タスクと規則のアクティビティ、Connect で呼ぶ規則はエラーにしないこと、表の名前がどれもそのファイルに書かれることを確かめる。エラーにしない名前で全部の出力先の生成物が通ることは、`tests/flows/names.flow`（規則二つを `tests/flows/rules/` に足した）を、ほかのテストのフローと同じに全部のプラットフォームで走らせて確かめる。rulec の例を規則のまわりのコードに答えさせる二本（`rule_glue_answers_the_rulec_vectors`、`python_rules_answer_the_rulec_vectors`）は、例のフローだけでなく `tests/flows` のフローの規則も見るようにした。
- 見つけた rulec のこと：order_state.rule の TypeScript（rulec の生成したもの）は `tsc --strict`（TypeScript 7）を通らない（列挙の値の絞り込み）。どのフローも order_state を関数として呼んでいなかったので、これまで表に出なかった。`names.flow` では別の規則にした（7.7）。D の最後の部分で rulec の生成器を直し（rulec の §15.171）、`names.flow` は order_state を関数として呼ぶ形にした（D.11）。

### D.4 dandori の単位

DESIGN 5.3。`Ty::Num(Unit)`、`UNIT_KINDS` を `ritsu-units` から、`rate_unit` と `rate_per` を単位の型の刻みに、範囲の端に単位を付けて書けるようにする。DESIGN 1.4 の `hold.flow`（`money[JPY, incl_tax]` を `money[円, incl_tax]` に渡す）が通るテストと、`mass[kg]` を `mass[g]` に渡すと E003 になるテストを足す。生成物はどのプラットフォームでも変わらない（突き合わせのテストで確かめる）。

D.4 でしたこと（2026-10-04、D の二つ目の部分）：

- dandori の `Ty::Num` は `ritsu_units::Unit` を持つ。二つの数の型が同じかは `Unit::same` で決める（`Ty` の等しさを手で書いた。`contract.rs` の突き合わせも同じ）。型の綴りは `Unit::parse` で読み、`Display` で書いたとおりに出すので、診断と生成物の型の名前は変わらない。`src/syntax.rs` の `UNIT_KINDS` は消した（次元の語は ritsu-units が知っている）。表に無い単位（`mass[foo]`）、税区分の誤り（`money[円, foo]`）、次元の誤り（`length[kg]`）は E002 で、ritsu-units が言う理由を注にする。前は、種類の語だけを確かめていた。
- `src/model.rs` の `rate_unit` と `rate_per`（刻みを文字列で作って読む）は、単位の型の刻みを読む `rate_per(&Unit)` 一つにした。規則の型も、口が渡す単位の型のまま持つ（D.3 では綴りの文字列にしていた）。
- 範囲の端に単位を付けて書ける（`range >=1kg <=40kg`、`<=100万円`、率の `<=50%`）。字句の段で、`>=` と `<=` のあとの、単位の付いた数を一つの語として読み（`万` と `億` も rulec と同じに掛ける）、lower が型の単位で数えた整数にする（ritsu-units の `Unit::convert`。率は百分率を刻みで割る）。整数にならない端、型の次元に無い単位、`int` に付けた単位は E003。前は、単位を付けた端を E001 でエラーにしていた。温度の単位（`℃`、`℉`）を、型の `[` のすぐあとと範囲の端の単位としてだけ読むようにした（前は `temperature[℃]` が E001 だった。名前の一部にはならず、ほかのところではこれまでどおり E001）。
- テスト：dandori の `tests/units.rs` に四本足した。DESIGN 1.4 の流れ（規則の `money[JPY, incl_tax]` をタスクの `money[円, incl_tax]` に渡す）が検査を通ること、`mass[kg]` を `mass[g]` に渡すと E003（税込を税抜に渡すのも E003）、単位を付けた端が型の単位で数えられること（`>=1kg` は 1000、`<=50%` は 500 刻み、`>=100銭` は 1、`>=41℉` は 5）、表に無い単位が理由つきの E002 になること。`tests/fixtures/range_syntax.flow` を、単位を付けた端を試すように書き直し、golden を取り直した（前は E001 の一行、いまは E003 の三つ）。
- 確かめたこと：例とテストのフローの出力の 969 回は、書き直した `range_syntax.flow` の `check` の三つのほかは、D.3 のあとと一字も違わない。どのプラットフォームの生成物も変わらない。

### D.5 rulec の言語をスレッドごとに

DESIGN 4.1。`i18n::with(lang, || …)` を足し、dandori と yuen が rulec を呼ぶところで使う。テストで、英語と日本語の `rulec doc` を同時に（別のスレッドで）描いて、どちらも正しい言語になること。

D.5 でしたこと（2026-10-04）：

- rulec の `src/i18n.rs` に `with` を足した（スレッドごとの言語。抜ければパニックでも前の言語に戻る。スレッドの言語はプロセスの言語に勝つ）。CLI はこれまでどおり `set` でプロセスの言語を決める。2,800 か所の `tr!` は書き換えていない。rulec の DESIGN §15.165 に書いた。
- テスト：rulec の `tests/lang.rs` に一本足した。`送料.rule` のページを、Markdown と HTML、英語と日本語の四つのスレッドで同時に四回ずつ描き、どれも `rulec doc --lang …` の出力と同じこと、抜けたスレッドの言語が戻ることを見る。スレッドの言語を読まないようにすると落ちる。rulec の出力の突き合わせも一字も違わなかった。
- dandori と yuen が rulec を呼ぶところで使うのは、まだできない（どちらもまだ rulec を同じプロセスで呼ばない。D.3 と D.7）。いまは rulec の口の `Rules::doc` が `with` を使い、dandori と yuen はこの口から呼ぶ。rulec に `RITSU_LANG` を読ませることは、CLI の振る舞いを変えないためにしなかった（7.6）。
- D の二つ目の部分で、作者が決めたとおり、rulec も `--lang`、`RULEC_LANG`、`RITSU_LANG`、英語の順に言語を選ぶようにした（rulec の `src/i18n.rs` の `from_env` が ritsu-base の `Lang::pick` で読む。rulec の §15.168）。`RITSU_LANG` を置いた環境でだけ振る舞いが変わる。`--help` の `--lang` の説明と最後の行、rulec のサイトの入れ方のページの優先順位の文に `RITSU_LANG` を足した。`tests/lang.rs` に一本足し（`ritsu_lang_comes_after_rulec_lang`）、そのファイルのテストは `RULEC_LANG` と `RITSU_LANG` の両方を外して rulec を走らせる。`RULEC_LANG` と `RITSU_LANG` を外した環境での rulec の出力（1,725 回）は、替える前と一字も違わなかった。

### D.6 dandori の種類の語

DESIGN 6.3。dandori の `Items`（`task`、`case`、`record` と `field`、`enum` と `value`、`input`、`output`）と `References`。`naming.tsv` の `dandori "order.flow" task reserve` の行を JSON の行に替え、dandori の入れ子の行を足す。

D.6 でしたこと（2026-10-04）：

- ritsu-base の名指しに、dandori の種類（`task`、`case`、`record` と下の `field`、`enum` と下の `value`、`input`、`output`）を足した。`naming.tsv` は、dandori の行を JSON にし、入れ子の行を二つ（`record 予約 field 泊数`、`enum Outcome value awaiting_review`）、入れ子の誤りの行を三つ（親のすぐあとでない `value`、`task` の下の組、`record` の下の `field` でない組）足した。種類の無いツールに種類を書く誤りは、`dandori has no kinds yet` の行が受け持っていたので、`file "src/app.py" task main` の行に替えた。表は 42 行（名指し 24、誤り 18）になった（DESIGN 6.3）。
- dandori の `src/ports.rs` の `Engine` が `Items` と `References` に答える。どちらも `.flow` を構文まで読むだけで、規則は読まない（規則を読めないフローにも、タスクと案件はある）。定義の文は、タスク・案件・レコードなら宣言の塊の行、列挙・フィールド・入力・出力ならその行、列挙の値ならその名前で、どの行もコメントと前後の空白を除き、文字列の外の続いた空白を一つにし、字下げは深さごとに空白二つに直す（レコードのコロンをそろえ直しても、字下げの幅を変えても、定義は変わらない）。参照は、`use rule`（下に書いた呼び方を `use rule … lambda, local` のように添える）、`use proto`・`use openapi`・`use smithy`（OpenAPI と Smithy の記述は `file`）、`implements`（サービス）、`connect` のタスク（サービスとメソッド）、子の `flow` である。サービスは package から見た名前（最後の部分）で名指す。`tests/ports.rs` が、例とテストのフローの全部で、中のものの行が収まり、名指しが読み直せ、参照の先のファイルがあることを確かめる。dandori のコマンドの出力は変わらない。
- 決めたこと（★）：型の中の参照（`<API>.<名前>`、`<規則>.<列挙>`、`follows <規則>.<ステートマシン>`）と、OpenAPI の操作を呼ぶ `http` は、参照に入れなかった。DESIGN 6.4 の一覧に無く、どれも、もう参照に出る `use rule` か `use proto|openapi` の行のファイルの中を指すからである。sakai が要素まで要ると分かったら（D.8）、足す。
- 名指しの表が変わったので、yuen と sakai の名指しのテストを直した（表の行数、理由の表）。dandori に種類を書く誤りを試していた yuen の変異 `E012_dandoriの種類` は、dandori に無い種類（`table`）を書く形にし、golden を取り直した。yuen の台帳の E012 の例と文、sakai の台帳の E011 の文から「dandori の種類」を外した。sakai では種類の語が `.ctx` の予約語でもあるので、`task`、`case`、`record` を名前にできなくなった（sakai の DESIGN 12.1）。yuen と sakai は、dandori の中のものを読むのはまだで（D.7 と D.8）、種類つきの名指しを、ファイルで名指したときと同じに扱う。

### D.7 yuen の一式の読み込み

yuen の PLAN の C.1〜C.9 を、子プロセスと JSON ではなく口で作るように書き直してから作る（書き直した計画を yuen の PLAN.md に書く）。

- 端は DESIGN 6.4 の定義の文。rulec の表、koyomi の日付と条件、geas の主張、dandori のタスクと案件が、一つずつ端になる。yuen の PLAN の C.13 に並べたハッシュは、端の中身が変わるので取り直し、yuen の DESIGN.md の 3.2 と 19 章を直す。
- 借りた出典と E107 は土台の `sources` で。
- `affected` は geas の `Claims` の口で記録を読む。
- `.proto` は `ritsu-proto` で。
- E203（ツールがファイルを読めない）と E204（ツールの JSON が知らない形）は、出す側の検査のエラーを名指す形に意味を替えるか、退かせる（DESIGN 7.10）。
- テストの例と確かめた記録（`.req` のハッシュ）を取り直す。yuen の DESIGN の「形の案」のうち、8 章と 11 章を実物にする。

D.7 でしたこと（2026-10-04、D の最後の部分）：

- yuen の PLAN の C.1〜C.9 を、口で作る計画に書き直してから作った（書き直す前の計画の要点は、各項の「前の計画」に残した）。yuen は読む言語を `yuen::suite::Suite`（言語ごとの `Items` と `Sources`、それに `Rules`、`Dates`、`Claims`）として渡され、コマンドを関数（`yuen::run::run`）にした。`ritsu yuen`（DESIGN 8.6）がすべての言語をつなぎ、yuen のクレートのバイナリは何もつながず、ほかの言語のものを名指すプロジェクトには `ritsu yuen` に同じコマンドを続けた形を言って exit 2 で終わる。
- 口を二つ足した（DESIGN 3.2）。`Sources`（rulec と koyomi が答える。ファイルが保存して固定している出典。検査を通らないファイルには検査の診断を返す）と、`Claims::affected`（geas が答える。geas の `affected` と同じ関数）。
- 端は、言語が `Items` で渡す定義の文になった（DESIGN 6.4、yuen の DESIGN 3.2）。定義の文が空なら端にせず E203。端のハッシュは取り直した（ファイルの端と chobo の端は A の段階の値のまま。koyomi の日付と条件、rulec の表と節、geas の主張、proto の要素、dandori のタスク、sakai の語の値は、yuen の PLAN の C.13 と DESIGN 19 章）。DESIGN 4.3 の例は、印が五本から三本になった（日付の端が、ファイル全体からその日付の定義の文になった）。
- 借りた出典と E107 は `Sources` で読む。`source outdated` は借りた出典も問い（取り直すのは借りた先の言語だと言う）、条が変われば、その条を固定している成果物も言う。`trace`（リンクごとの `pins`）、`api`（成果物の `pins`）、PROV（`yuen:pins`）も、成果物のファイルが固定している条を出す。
- `affected` を作った（yuen の DESIGN 8 章を実物にした）。統一形式の差分の読み手は、geas の `src/diff.rs` の読む部分を ritsu-base の `udiff` に移して、geas と yuen で一つにした（DESIGN 4.14。geas の振る舞いは変わらない）。`yuen api` を実物にした（11 章）。
- 台帳：E203 の意味を替え、E204 と W201 を退かせた。退いたコードを書く形（`Repro::Retired`）を ritsu-base の台帳に足した（DESIGN 4.3）。E106、E107、E202、E203、E205 に再現と変異を足した。
- テストの材料を七つ足した（`tests/fixtures/` の rulec、koyomi、chobo、geas、proto、dandori、sakai）。確かめた記録は `ritsu yuen review` で書いた。geas の記録は `geas map` で一度だけ取ってテストの材料に置き、テストは python3 を走らせない。yuen のテストは六つの言語のクレートを dev-dependency に持ち、同じプロセスの中でつなぐ（DESIGN 3.3）。新しいテストは `tests/suite.rs`（19 本）と、`crates/ritsu/tests/yuen.rs`（3 本）。
- 決めたこと（★）：
  - `Sources` を `Rules` と `Dates` の事実に入れず、別の口にした（DESIGN 3.2 の段落）。
  - yuen のクレートのバイナリがほかの言語を読めずに止まるときの終了コードは 2 で、言う文は `ritsu yuen` に同じコマンドを続けた形にした（ツールが見つからないときと同じく、走らせる場所の問題として）。
  - `affected` で、どの主張も走らせない行でも、そのファイルをリンクが名指していれば要件に届くとした。届かない変更（exit 1）は、どのリンクも名指さないファイルの行と、範囲の中でどの要件にも辿れないファイルだけにした（yuen の DESIGN 8 章）。
  - ReqIF には、成果物のファイルが固定している条を書かない（PROV だけ。yuen の DESIGN 12 章）。PROV では、固定をファイルの entity から引く（リンクがファイルを名指していなければ足す）。
  - `trace --format json` の、リンクごとの `pins` は、規則とカレンダーでなくても空の並びで出す（キーの形を一つにするため。前の golden も取り直した）。
  - 名前の変わった成果物の候補が一つで、確かめたときの中身があれば、差分を見せる（yuen の DESIGN 4.5）。
- 確かめたこと：yuen のテストは全部通り、SKIP は 0。替える前の yuen のバイナリと替えたあとの `ritsu yuen` で、テストの材料と変異の全部のコマンドの出力（1,026 回）を比べた。違った 258 回は、どれも決めて変えたものだった。ほかの言語のものを名指すプロジェクトを読むようになったこと（新しいテストの材料と変異の 173 回）、`--help` と引数の無い `yuen`（`affected` が入った。exit 2 の説明）と `explain --all`（台帳の変わり）の 13 回、`trace --format json` の `pins` の 72 回である（yuen の DESIGN 16.1）。rulec、koyomi、geas は口を足しただけで、出力は変わらない（geas と koyomi のテストの全部が通る）。

### D.8 sakai の一式の読み込み

sakai の PLAN の C.1〜C.5 を、口で作るように書き直してから作る。

- rulec の列挙と `shape` は `Rules` の口で、koyomi のカレンダーへの参照は `References` で、chobo の勘定と振替は `Books` で（doc のため）。
- dandori の参照を `References` で読み、N101 をやめ、DESIGN 7.10 の四つの検査を足す（コードは sakai の台帳に）。子の `.flow` の扱いを sakai の DESIGN.md に決める。
- E104（ツールが無い）と E105（ツールの api が失敗した）は、意味を替えるか退かせる。
- 例の `check` の要約に、rulec 2、koyomi 1 と、dandori の参照の数が出る（sakai の PLAN の C.15 の表の値を、dandori の分を足して直す）。

D.8 でしたこと（2026-10-04、D の最後の部分）：

- sakai の PLAN の C.1〜C.5 を、口で作る計画に書き直してから作った（子プロセス、ツールの探し方、版の確かめ、時間の上限は作らない）。sakai は読む言語を `sakai::suite::Suite`（rulec の `Rules`、rulec・koyomi・dandori の `References`、chobo の `Books`）として渡され、コマンドを関数（`sakai::run::run`）にした。`ritsu sakai`（DESIGN 8.6）がすべての言語をつなぎ、sakai のクレートのバイナリは何もつながない。
- 境界を越える参照に、rulec の `import proto`・`shape`・`apply`、koyomi の `use calendar`、dandori の `use rule`・`use proto`・`connect`・`flow` を足した。dandori の `implements` は、自分の公表された言語の公開ホストサービスであることを確かめる。例の `check` の要約は「9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)」になった（sakai の PLAN の C.15 の表を直した）。
- 台帳：E104 は「地図が含む成果物の言語がつながっていない」、E105 は「成果物が、その言語の検査を通らないか、読めない」に意味を替え、N101 を退かせた（台帳に残す）。DESIGN 7.10 の四つの検査を E202（同梱、`apply`）、E207、E208、E209 として足した。ほかの言語の成果物を含む再現は、`sakai explain` が `ritsu sakai check .` で走らせると言う（ritsu-base の台帳で、`ritsu` で始まるコマンドをそのまま書くようにした。DESIGN 4.3）。
- テスト：sakai のテストは rulec、koyomi、chobo、dandori を dev-dependency に持ち、同じプロセスでつなぐ。一式のバイナリを走らせるところ（`SAKAI_RITSU` と、例のコピーを確かめる `what_was_copied_passes_the_suite`）を口に替え、ritsu-testkit の `Need::Suite` と CI（`tools.yml`）の `SAKAI_*` を消した。例を土台にした変異を九つ足した（sakai の PLAN の 5.4）。
- 決めたこと（★）：
  - E104 は言語ごとに一度、その言語の最初の成果物を持つ `owns` の行で言う診断にした（exit 1）。yuen のクレートのバイナリは exit 2 で止まるが、sakai は地図の検査の診断として言う（sakai の診断の台帳にあったコードの意味を替える形で、検査の段と一緒に出せるため）。
  - 子の `.flow` を境界の向こうから走らせてよいのは、パートナーシップ、共有カーネル、子が相手の公開ホストサービスを `implements` で実装しているときだけ（E209。sakai の DESIGN 4.7）。
  - 規則の `apply` も境界を越える参照にした（規則そのものを使うので、共有カーネルの中でなければ E202）。`use openapi`・`use smithy`、JSON Schema、出典のコピーは、それを読む成果物の一部として数えない。
  - koyomi と dandori は、構文を読めるファイルに参照を答える。rulec は、検査を通る規則にだけ事実を答え、sakai は事実を答えた規則にだけ参照を問う。
  - 規則の `means` の先に無い要素は E007（sakai の PLAN は E408 と書いていた。E408 は公表された言語に無い要素のコード）。
  - 片側だけの共有カーネルで、境界を越えて読むカレンダーは E201 になる（sakai の PLAN は E202 と書いていた。関係の向きが逆なので E201 が先に決まる）。
  - api の `not_checked` は、いつも空のまま残した。`crossings[].via` は参照の種類ごとの語にした。
- 確かめたこと：sakai のテストは全部通り、SKIP は 0。例と fixture の `.ctx` に対する 238 回の出力を、替える前の sakai のバイナリと `ritsu sakai` で比べ、違った 13 回は、どれも決めて変えたもの（例の `check` の 6 回と `api`、`explain --all` の 4 回、`check --help` の 2 回）だった。sakai のクレートのバイナリでは、ほかに例の `build` の 4 回と `export cml` が E104 で止まる（18 回）。B と C の変異の golden は一字も変わらない。

### D.9 chobo の単位

DESIGN 5.4。chobo の単位を単位の型に載せ、お金の単位に税込と税抜の区別（`unit 円 incl_tax`）を書けるようにする。区別の無い chobo の単位は、区別の無い額だけを受け取る。chobo の README（英語と日本語）、DESIGN.md、スキル、`explain` の台帳に、新しい書き方を載せる。

D.9 でしたこと（2026-10-04、D の最後の部分）：

- chobo の単位の行は `unit <名前> [scale <桁>] [incl_tax | excl_tax]` になった。税の語は最後に一つだけ書く（`scale` より前に書けば E001）。chobo の `model::unit_type` が単位を ritsu の単位の型にする（DESIGN 5.4 の三つの決まり。通貨の名前で `scale 0` はその通貨、円と JPY のほかの通貨で `scale 2` は `<コード>c`、足し引きのできる次元の単位の綴りで `scale 0` はその単位、ほかは `Dim::Count(名前)`）。お金でない単位に税の語を書けば、新しいコード E014。chobo は ritsu-units に依存するようになった（言語のクレートが土台に依存するのは 3.1 のとおり）。
- 口の `BookUnit` に、ritsu の単位の型 `unit` を足し、`BookFacts::unit(名前)` で引けるようにした。`chobo api` は税の語を書いた単位にだけ `tax` を出す。`chobo doc` は勘定の単位を `円 incl_tax` のように書く。税の語は帳簿の中の意味も生成するコードも変えない（TigerBeetle の ledger の番号は単位の名前と `scale` から作り、税の語は入らない）。
- 「区別の無い chobo の単位は、区別の無い額だけを受け取る」は、ritsu-units の `Unit::same` が税の区別まで比べることで成り立つ（`unit 円` の型は `money[円]` で、`money[円, incl_tax]` とも `money[円, excl_tax]` とも同じでない）。受け取るところで確かめる検査は E の X4（DESIGN 7.6）で、D では単位の型を渡すところまでにした。
- 文書：chobo の DESIGN（1.2 に税込と税抜と ritsu の単位の型の表、1.6 のキーワード、3.1 の E014、5 章の api の `tax`、8.1 に D.9 の段落）、README.md（「Money with tax or without」の節、いまの状態、コードの数 29）、README.ja.md（「税込と税抜」の節、いまの状態、コードの数）、`docs/reference.md`（単位の節とキーワードの表）、`docs/formats.md`、`docs/codes.md` と `docs/codes.ja.md`（`explain` の出力そのもの）、スキル（`SKILL.md` の単位の行と、`sync.sh` でコピーする三つ）。
- テスト：chobo の `tests/units.rs`（四本。単位の型、税の区別が同じかどうか、E014 と税の語の書き方の誤り、口の事実と api の `tax`）、`tests/fixtures/税区分.book`（E014 の golden、英語と日本語）、`tests/ids.rs`（税の語で ledger の番号が変わらないこと）。
- 確かめたこと：例とテストの帳簿の全部のコマンドの出力（277 回）と、例を七つの組み合わせに生成したファイル（72 個）を、替える前と後のバイナリで比べた。違ったのは、E014 が増えた `--help` の三つ（`check --help` の英語と日本語、知らないフラグに添える `--help`）と `explain --all` の四つ、新しいテストの帳簿 `税区分.book` の四つ（替える前は E001）だけで、生成したファイルは 72 個とも一バイトも変わらない。yuen の端になる chobo の定義の文（DESIGN 6.4）も、税の語を書かない帳簿では変わらない（`返金` は 1,206 バイトで `84e9ce254075c697` のまま）。

### D.10 rulec と dandori を `ritsu-proto` に

二つの `src/proto.rs` を `ritsu-proto` に替える。rulec の契約の検査（コーパスと変異）、dandori の `connect`・`implements`・proto から作る型（`tests/protos.rs` と例）の結果が、替える前と同じであること。

D.10 でしたこと（2026-10-04）：

- rulec と dandori の `src/proto.rs` を `ritsu-proto` で読む形にした（rulec 1,358 行から 569 行、dandori 1,145 行から 551 行）。それぞれに残したものは DESIGN 4.13 の「D.10 で rulec と dandori を替えた形」に書いた。dandori は import をたどる部分を残し（ディスクからも、ブラウザで試すページが持つファイルからも探すため）、型の名前は見えるファイルだけから引き、proto2 と editions を受け付けず、読めなかった import があれば引けない名前を書いたまま持つ。
- 決めたこと（★）：rulec は、`.proto` として読めない契約を途中まで読まずに、E013 で場所を言う。rulec の DESIGN §15.166 に理由を書いた。前の読み手は、最後の `}` が欠けた契約を通し、値の行の `=` が抜けた契約に E032 を出していた。
- `ritsu-proto` の文：何が要るかの語を二つの言語の文にし、日本語の文に英語の語が混ざらないようにした（「a name が要る」が「名前が要る」）。dandori の読めないファイルの文の形も変わった（dandori の DESIGN 0.3）。
- `ritsu-proto` の `tests/readers.rs` から、rulec と dandori の古い読み手と生のまま比べる部分を消し、rulec と dandori への dev-dependency を消した（DESIGN 3.3）。消す前に、替えたあとの二つの読み方とも生のまま比べ、golden の全部と同じことを確かめた。golden は `tests/golden/sakai.txt` の一行（上の文の直し）だけが変わった。
- 確かめたこと：rulec の出力の突き合わせ（コーパスと変異の契約の突き合わせを含む）と、dandori の 701 回の出力（93 の `.flow` の `check` を英語・日本語・JSON で、例の七つの出力先への `build` とシナリオと `doc`、`tests/flows` の Temporal への `build`）が、替える前と一字も違わなかった。rulec の読み手の単体テスト 14 本と dandori の 9 本は、そのまま通る。rulec のテストを二本足した（`tests/proto.rs` の「読めない契約は途中まで読まずに場所を言う」と `tests/projection.rs` の「読めない proto の契約は読めないと言う」）。dandori の `tests/protos.rs` と、`.proto` を読む例（fulfillment）と Connect で規則を呼ぶ例（order）を走らせる重いテスト九つも通った（7.6）。

### D.11 D の完了の条件

1. 根から `cargo test --workspace --no-fail-fast -- --nocapture` が全部通り、この機械で SKIP は 0（B.8 で許したものを除く）。
2. ritsu のツールどうしが子プロセスで呼び合うところが無い（`Command::new` で `rulec`・`koyomi`・`chobo`・`geas`・`dandori` を呼ぶコードが、テストのランナーのほかに無い）。
3. D.3 の突き合わせが、JSON の読み手を消す前に全部の規則で通った（報告に件数）。
4. DESIGN 1.4 の二つの例が直っている（`hold.flow` が通る。dandori が率の刻みを文から読まない）。
5. yuen の C.1〜C.9 と sakai の C.1〜C.5 が、書き直した計画の完了の条件を満たす。
6. 報告に、変えた golden の一覧と、yuen の取り直したハッシュの一覧を書く。

D.11 でしたこと（2026-10-04、D の最後の部分）：

1. 根から `cargo test --workspace --no-fail-fast -- --nocapture`（dandori の重い十二を `--skip` で外したもの）が、10 分 54 秒で 1,576 件通った（ignored 1、rulec の一つ）。SKIP は 0 で、dandori の Jev も、作者の環境の TypeSafe の鍵で TypeSafe に送って回した。外した十二は一つずつ `cargo test -p dandori -- --exact <名前>` で回し、全部が一度で通った（durable 16 秒、pydantic-graph 4 秒、記録した履歴の再生 16 秒、LocalStack 69 秒、Temporal の TypeScript 74 秒・Python 63 秒・Go 72 秒・言語をまたぐ 106 秒・Worker Versioning 17 秒・子のフロー 13 秒、Argo 126 秒、Ollama 25 秒。回し直しは無し）。合わせて 1,588 件で、クレートごとには rulec 744（ignored 1）、geas 231、yuen 118、koyomi 104、dandori 102（重い十二を含む）、sakai 95、chobo 70、ritsu-base 66、ritsu-testkit 15、ritsu-proto 14、ritsu-units 9、ritsu 7、xtask 6、ritsu-emit 5、ritsu-ports 2。コンパイラの警告は rulec のテスト関数の名前の七つだけ（前と同じ）。一度目に回したとき、`ritsu-proto` の `tests/readers.rs` の三つが落ちた。D.8 で足した sakai の変異（`E105_取り込みが合わない規則`）の `.proto` が、三つの読み手で読む `.proto` の一覧に加わったからで、golden（`tests/golden/` の `rulec.txt`、`dandori.txt`、`sakai.txt`）に、そのファイルの行を足した（ほかの行は変わらない）。
2. ritsu のツールどうしが子プロセスで呼び合うところは無い。`crates/*/src` の `Command::new` の相手は、git、go、rustc、python3、psql、curl、Chrome、PostgreSQL、TigerBeetle、kill と ps などの外のもの、rulec が生成したコード、利用者が渡すアダプタと抽出器、geas が外から叩くプログラムである。ritsu の言語のバイナリを走らせるのは、rulec の MCP サーバーが、MCP のツールを呼ばれるたびに自分自身（`current_exe`）を走らせるところだけで、言語どうしではない（rulec の §15.163）。テストでは、各クレートが自分のバイナリ（`CARGO_BIN_EXE_<名前>`）を、`crates/ritsu/tests/` が `ritsu` を走らせる（テストのランナー）。ほかの言語のバイナリを走らせるテストは、D.8 で sakai の最後のもの（例のコピーの確かめ）を口に替えて、無くなった。
3. D.3 の突き合わせは、JSON の読み手を消す前に、rulec のコーパスの 50 本と dandori の 14 本の規則で、dandori が読む 1,939 の項目を比べて通した（違った 7 か所はどれも JSON の側の読み違え。D の二つ目の部分。DESIGN 3.2）。
4. DESIGN 1.4 の二つの例は直っている。`hold.flow` は通り（dandori の `tests/units.rs` の `an_amount_in_jpy_goes_where_yen_is_taken`）、dandori は率の刻みを、説明の文からではなく口の事実の型から読む（D.3、D.4）。
5. yuen の C.1〜C.9 は、書き直した計画の完了の条件（yuen の PLAN の C.13）を満たした。sakai の C.1〜C.5 は、書き直した計画の完了の条件（sakai の PLAN の C.15 の表の、例の `check`、一式の言語、変異、台帳、文書の行）を満たした。
6. 報告に、変えた golden の一覧と、yuen の取り直したハッシュの一覧を書いた（ハッシュは yuen の PLAN の C.13 と DESIGN 19 章にもある）。

## 5. 段階 E：言語をまたぐ検査と一つの入口

大きい段階なので、指示する側が二人に分けてよい。分けるなら E-a（E.1〜E.4 と E.8）を先に、E-b（E.5〜E.7）をそのあとにする（E-b は E.1 の読み込みを使う）。実際には、E の最初の部分（E.1〜E.3、E.8）のあと、E.4〜E.7 をいくつかの担当に並べて渡した。E.1〜E.8 は済んだ。E.9 の状態は E.9 の終わりに書いた。

### E.1 `ritsu-project`

DESIGN 6 章。プロジェクトを歩いて種類を分け、一度ずつ読み、索引を作り、ファイルをまたぐ参照を解決し、出す側の実装を作って受け取る側に渡す。テストのプロジェクトには、sakai の `examples/shop/`（英語の版。日本語の版は `examples/shop.ja/`。rulec の規則、koyomi のカレンダー、chobo の帳簿、dandori のフロー、`.proto`、コードを持つ）を使い、`crates/ritsu/tests/projects/shop/` にコピーする（日本語の `通販/` も残す）。

**したこと**（E の最初の部分）：

- `ritsu-ports` に索引 `Index` と、名指しを引いた答え `Lookup` を置いた（DESIGN 6.4）。各言語の `Items` と `References` の答えを、ツールとルートとファイルで一度だけ尋ねて持ち、`find` で名指しを引く。
- `crates/ritsu-project` を作った（DESIGN 6.1）。`Joined` が言語を一度だけ作って索引でつなぎ、dandori・yuen・sakai に口を渡す。`Project::load` がプロジェクトを歩いて言語を分け、`Project::references` が参照を索引で解く。
- `crates/ritsu/src/main.rs` の `yuen_suite` と `sakai_suite` を消し、`ritsu dandori`・`ritsu yuen`・`ritsu sakai` は `Joined` から口を受け取る。三つが別々に rulec の `Engine` を作ることは無くなった（7.8）。
- 名指しを索引で引く形にした（DESIGN 7.10 の X10）。yuen は `src/ends.rs` と `src/coverage.rs`、sakai は `src/suite.rs` と `src/elements.rs`。口のまとまり（`yuen::suite::Suite`、`sakai::suite::Suite`）は、言語ごとの `Items`・`References` の表に代えて索引を持つ。言語ごとのコードと文は変えていない。
- sakai の例を `crates/ritsu/tests/projects/通販/` にコピーした（92 ファイル。祝日の表は Shift_JIS で改行が CRLF のまま）。sakai の例は、英語を先にする担当がのちに `examples/shop.ja/` に名前を替え、英語の版 `examples/shop/` と、そのコピーの `crates/ritsu/tests/projects/shop/` を足した。

**決めたこと**：

- 索引は `ritsu-ports` に置いた（ritsu-project ではなく）。受け取る側の言語が型として持つもので、ritsu-project に置くと言語がつなぎの層に依存することになる（DESIGN 3.1 の決まり 3）。中身はどの言語の意味も持たない。
- 索引に yuen の `Items` と `References` は入れない。yuen の `Engine` がこの索引を含む口を持つので、入れると輪になる。yuen の要件を名指す言語は、いまは無い（DESIGN 6.4）。
- sakai の語の `means` が名指す規則の要素は、rulec が規則に答えた（`Rules` の事実がある）うえで索引で引く。索引が渡すのは、規則が自分のファイルに書いたもの（rulec の `Items`）で、前の `RuleFacts` の列挙は `apply` で展開したものも含んでいた。どの例とテストにも、その違いが出るものは無い（sakai の DESIGN 4.1）。
- プロジェクトに言語のファイルが一つも無ければ、使い方の誤りとして止める（何も確かめずに通ったように見せない）。名前で渡したファイルが言語のファイルでないときも止める。ディレクトリの下の、言語のファイルでないものは読まない（sakai が地図で読むコードなど）。
- テストのプロジェクトの参照の全部を golden にした（`crates/ritsu-project/tests/golden/shop.references.txt`、119 行）。

**確かめたこと**：触ったクレート（ritsu-ports、ritsu-project、yuen、sakai、ritsu）のテストを、ツールを全部つないだ環境で一度回した（228 件が通り、SKIP 0）。yuen と sakai のコマンドの出力を、替える前のバイナリと突き合わせた。yuen は 1,026 回、sakai は 238 回で、クレートのバイナリでも `ritsu yuen`・`ritsu sakai` でも、一字も違わなかった。

### E.2 `ritsu check`

DESIGN 8.1、8.3、8.4。テキストと JSON の形を決めて golden にし、英語と日本語で取る。各言語の `check` と同じ診断が出ること（ファイルごとに、その言語の `check` の出力と突き合わせる）、見出しにツールの語が入ること、終了コード。

**したこと**（E の最初の部分）：

- `ritsu check [<path>...] [--root <dir>] [--format json] [--lang ja|en]` を作った（`crates/ritsu/src/check.rs`、`src/cli.rs`）。`ritsu-project` の `Project::check` が、言語ごとの `check` を DESIGN 6.1 の順に走らせる。各言語は、自分のコマンドが印字に使う関数で、単位ごとの結果（`ritsu_ports::Checked`：診断一つずつのテキストと JSON、そのほかの行、単位の結果）を返す（`rulec::ports::Engine::checked` など七つ。DESIGN 8.1）。
- テキストと JSON の形を決めた（DESIGN 8.3 の形の案を実物に差し替えた）。テキストは言語が印字するもののまま、見出しにツールの語を足し、最後に一行の要約を置く。JSON は `ritsu`・`root`・`ok`・`files`・`diagnostics`・`borders` の順のキーで、言語の診断はその言語の形のまま、先頭に `tool` を置き `file` をルートからの相対にする。言語をまたぐ検査の結果を載せる場所は、要約の境目の数と JSON の `borders`、診断は `tool` が `ritsu` のもの（中身は E.4）。
- 終了コードを決めた（DESIGN 8.4）。確かめられなかった単位があれば 2、エラーか通らない単位があれば 1。
- `ritsu <言語>` を七つの全部に作り、`ritsu` を言語の名前で呼べばその言語のコマンドとして動くようにした（DESIGN 2.3、8.2）。そのために rulec、koyomi、chobo、geas のコマンドの本体を `src/main.rs` からライブラリの関数に移した（`rulec::cli::run`、`koyomi::run::run`、`chobo::run::run`、`geas::cli::run`）。`ritsu --help` は ritsu-base の表で描く。
- rulec の `Engine` が `check` の報告を覚え、`ritsu check` の中で dandori や sakai が同じ規則の事実を尋ねても、規則を検査し直さないようにした（DESIGN 6.1、rulec の §15.173）。
- テスト：`crates/ritsu/tests/check.rs`（テストのプロジェクトと、受注が注文の状態に値を足したコピーの、英語と日本語のテキストと JSON の golden。各言語のコマンドとファイルごとに突き合わせること。止める場合）、`tests/entry.rs`（`--help`、七つの `ritsu <言語>`、リンクの名前）、rulec の `tests/ports.rs`（`checked` の出力が `rulec check` と同じで、規則を一回ずつしか検査しないこと）。

**決めたこと**：

- 言語は、文を読み直すのではなく、診断を一つずつ型で渡す。ツールの語は、各診断の見出しの最初の `[<コード>]` に足す。言語の文の形（rulec の `-->` の枠、geas の主張の行の中に字下げして入る診断）は変えない。
- rulec、koyomi、chobo、geas、dandori にはファイルを一つずつ渡し、yuen と sakai には渡されたパスのうち自分のファイルを含むものと `--root` を渡す（yuen と sakai は、自分でファイルを探して、プロジェクトや地図として確かめる言語だから。sakai はディレクトリを渡されたときだけ W103 を言う）。
- 言語は `--lang` と `RITSU_LANG` で選び、各言語の `<名前>_LANG` は読まない（一つのコマンドの文面を一つの言語にする）。
- geas の `check` は主張を走らせ、ジャーナルを書く。`ritsu check` でも同じにした。geas が構文の誤りに 2 で終わるのは、`ritsu check` では 1 として読む（DESIGN 8.4）。
- JSON の、境目の検査の結果のキーを `crossings` ではなく `borders` にし、`proved` を `held` にした（DESIGN 8.3。sakai の `crossings` と取り違えないため、0.4 の語の決まりのため）。
- `ritsu <言語>` は七つの全部に作った（PLAN の E のどの項目にも書かれていなかったが、DESIGN 8.1 と 8.2 は E で作るとしていて、`ritsu check` のために言語のコマンドをライブラリの関数にすれば、ほとんどそのまま作れる）。

**確かめたこと**：触ったクレート（ritsu-ports、ritsu-project、ritsu、rulec、koyomi、chobo、geas、dandori、yuen、sakai）のテストを、ツールを全部つないだ環境（PostgreSQL つき）で一度回した。1,472 件が通り、落ちた 3 件（rulec のフラグのテストが `src/main.rs` を読んでいたこと、テストの二つの書き誤り）を直して、その 3 件を回し直して通った。SKIP 0、ignored 1（rulec の、前からあるもの）。dandori の重いテストは、生成器と runner と規則の読み方に触れていないので回していない。

コマンドの出力を、E の前のバイナリと突き合わせた（作業場所の `cap.py`。geas の分を足した）。rulec はコーパスの 1,675 回、koyomi は 289 回、chobo は 277 回、geas は 339 回（`--help`、`explain`、`tests/specs` の 109 の主張のファイルの `check` をコピーの上で）、dandori は `ritsu dandori` で 702 回、yuen は 1,026 回と sakai は 238 回をクレートのバイナリと `ritsu yuen`・`ritsu sakai` の両方で走らせ、どれも一字も違わなかった。新しい `ritsu rulec`・`ritsu koyomi`・`ritsu chobo`・`ritsu geas` の出力も、E の前のそれぞれのバイナリと一字も違わなかった。

### E.3 ritsu の台帳と `ritsu explain`

`crates/ritsu-cross/src/codes.rs` に、言語をまたぐ検査のコードを置く（土台の `ledger`）。どのコードにも、出すプロジェクトの最小の再現を置き、テストが走らせる。

**したこと**（E の最初の部分）：

- `crates/ritsu-cross` を作った。`ritsu_cross::check(プロジェクト, Joined, 言語)` が ritsu 自身の診断（`Finding`、`tool` は `ritsu`）と境目の検査の数（`Borders`）を返し、`ritsu check` がそれを各言語の診断のあとに出す。
- 台帳（`src/codes.rs`）に E101（`.proto` として読めないファイル）を載せた。番号の帯は E1xx がプロジェクトのファイルを読むところ、E2xx が言語の境目の検査（DESIGN 7.1）。再現は小さなプロジェクトのファイルで、英語の名前（`shop.proto`）にした。
- `ritsu explain <コード> | --all [--format markdown|json]` を作った（`crates/ritsu/src/explain.rs`）。言語のコードは `ritsu <言語> explain` で引くように言う。
- `crates/ritsu-cross/docs/codes.md` と `codes.ja.md`（`ritsu explain --all --format markdown` の出力）。
- テスト：`crates/ritsu/tests/codes.rs`（再現の全部が自分のコードを英語と日本語で出すこと、`ritsu explain`）、`crates/ritsu-cross/tests/codes.rs`（同じコードが二度無いこと、二つのページが台帳と同じこと）。

**決めたこと**：

- X10（名指しの解決）のコードは ritsu の台帳に載せない。名指しを書いた言語が自分のコードで言うので（DESIGN 7.10）、ritsu が言えば同じことを二度言う。この部分で ritsu が自分で言うことは、どの言語も確かめない `.proto` の読めなさだけで、それを E101 にした。
- E101 は、`.proto` を読む言語がそれぞれ言う診断（rulec の E013 など）と重なることがある。ファイルのところで一度言い、読む言語が読むところでも言う形にした（直す先はファイルである）。
- 台帳の型（`Borders` を含む）は ritsu-cross に置き、`ritsu check` の JSON の `borders` と要約がそれを読む。

**確かめたこと**：触ったクレート（ritsu-cross、ritsu）のテストを一度回し、18 件が通った（SKIP 0）。`cargo xtask deps` は 17 のクレートで通る。テストのプロジェクトの `ritsu check` の golden は、`.proto` がどれも読めるので変わらない。

### E.4 X1〜X4 と X6 の検査

DESIGN 7.3〜7.6、7.8 のうち、dandori の新しい書き方が要らないもの（X1、X2、X3 の (a)、X4 の rulec の側）から作り、X3 の (b)（rulec の新しい書き方。DESIGN 7.5）を作る。どの検査にも、通るプロジェクトと、落ちる変異（成り立たない例が出るもの、決められないもの）を置き、英語と日本語の golden を取る。X3 の (b) では、rulec の証明書に集合の出どころと集合を書き、`tools/recheck.py` と Lean の再検査が集合の上で通ることも確かめる。

**したこと**（E.4。言語をまたぐ検査の担当）：

- rulec に `range from koyomi "<ファイル>" date <日付の名前>` を足した（X3 の (b)。rulec の §15.174）。構文、日付の入力にだけ書けること（E065）、口 `Dates` から集合を読むこと（`src/days.rs`）、領域のふるい（日付の軸の座標に集合の日が無ければ起きない）、当てはまらない行（E102）、例（E019）、ベクタ、網羅（境界の対の義務を集合の日に限る）、生成コードの入口（九つの言語。`rulec test` で十二の言語の全部が通る）、証明書の `days`・覆いの葉 `days_axis`・集合の日が分ける対 `days_apart`、`tools/recheck.py` と Lean の再検査（点と対の両方）、`doc` のページの注記、前提の `days`（`api`）、`References` の koyomi の名指し、`checked_over`。koyomi がつながっていなければ E129（終了コード 2）、集合を読めなければ E130。rulec の DESIGN、`docs/reference.md` とスキルのコピー、サイトの checks のページ（英語と日本語）、台帳（`docs/codes.md`、`codes.ja.md`）に載せた。`ritsu-project` の `Joined` と `ritsu rulec` が koyomi をつなぐ。
- 口の問いを実装した：`preconditions_hold` の並びの合計と長さ（並びの長さの上限を問いに足した）、`checked_over`、`Books::refusals`。`Rules::output_values`、`Rules::date_range`、dandori の新しい口 `Flows::rule_calls` を足した（DESIGN 3.2）。
- ritsu-cross に X2 を作った（E201、W201。`src/preconditions.rs`）。X3 の (a)、X4、X6 の判定を `src/borders.rs` に作り、台帳に E202〜W205 を `Repro::Later` で載せた（E.5 で再現を足した）。`ritsu check` の境目の数に、X2 の前提と、X3 の (b) の入力を数える。`crates/ritsu-cross/docs/codes.md` と `codes.ja.md` を取り直した。
- テスト：rulec の `tests/days.rs`（7 件、golden 20、`tests/days/golden/`）、`tests/ports.rs` の並びの上限、`tests/codes.rs`（koyomi をつなぐ）。`crates/ritsu/tests/cross.rs`（golden 12、`tests/golden/cross/`）、`crates/ritsu/tests/codes.rs`（E201 と W201 の再現）。`crates/ritsu-cross/tests/borders.rs`（X3 の (a)、X4、X6 の三つの結果）。

**決めたこと**：

- ★ X1 は言語の検査のまま（DESIGN 7.3 の E で決めたこと）。
- ★ `range from koyomi` の書き方（`from` のあとに名指し）。`range` のすぐあとの `from` は射影の始まりとして読まない。
- ★ koyomi がつながっていない rulec（クレートのバイナリ、ブラウザ）は E129 で止め、終了コード 2（dandori の E018、yuen の E206 と同じ扱い）。
- 集合の外の日は、生成コードの入口で受け付けない（範囲の両端だけでは、表が確かめていない日が入口を通る）。
- 前提に `days` の種類を足した（`api` の出力が変わるのは、新しい書き方を使う規則だけ）。
- X2 の例は、dandori の範囲が値ごとに独立に取れるとしたときの角（dandori の E014 と同じ読み方）。同じ値を両方に渡すときだけは、値が連動することを使って決める。
- X3 の (a)、X4、X6 の台帳のコードは、判定と文を先に置き、再現を `Later` にした（呼び出しの場所は E.5 で作った）。
- ritsu-cross の dev-dependency に rulec、koyomi、chobo を足した（判定を本物の答えで確かめるため。DESIGN 3.3 が許す形）。

**確かめたこと**：項目ごとに、触ったクレートのテストを一度ずつ回した（rulec の `days`・`codes`・`ports`・`docs`・`readme`・`website`・`skill`・`cert`・`lean`・`compat`、chobo の `ports`、ritsu・ritsu-project・ritsu-ports・ritsu-cross の全部。落ちたのは、台帳の件数（109 → 112）と README・スキル・サイトの件数とコードの範囲、chobo の `ports.rs` が `refusals` を「決められない」と言うことを確かめていたところで、どれも直して回し直した）。最後に、根から `cargo test --workspace --no-fail-fast -- --nocapture`（dandori の重い十二を `--skip`）を一度回し、1,608 件が通った（17 分 39 秒、ignored 1（rulec の前からのもの）、SKIP 0）。そのあとに直した rulec の網羅（`src/coverage.rs`）と `tests/days.rs` と DESIGN は、`days`・`coverage`・`docs`・`website`・`readme` を回し直した（66 件。PostgreSQL を立てて、生成した SQL の関数の側も通した）。dandori の重い十二は回していない（生成器、runner、`.proto` に触れていない）。rulec の証明は `lake build`（31 の仕事、`sorry`・`axiom`・`native_decide` は 0）と、新しい証明書の `tools/recheck.py` と `rulec-recheck`（`tests/days.rs`。二つの規則とも「証明した」）を通した。コマンドの出力を E.4 の前のバイナリと突き合わせた。rulec はコーパスとテストの規則の 1,682 回で、違ったのは新しい材料 `tests/days/settlement.rule` の 5 回だけ（前は E013、後は E129）。`ritsu rulec` で走らせた 1,682 回も、違ったのは同じ材料の 7 回だけ（後は koyomi をつないで通る）。koyomi の 289 回と chobo の 277 回は一字も違わなかった。`ritsu check` のテストのプロジェクト（`通販`）の golden も変わらない。

### E.5 dandori から koyomi と chobo を呼ぶ

DESIGN 7.7、7.8。`use dates`、`use book`、タスクの呼び方、`case … follows <帳簿>.<振替>`、時刻を読む式 `now` を、dandori のすべてのプラットフォームに作る。作者の決まり（機能はおまけにしない）のとおり、生成、参照インタプリタの見え方（`View`）、E040 と E050、ランナーと突き合わせのテスト、dandori の README と DESIGN.md の全部に載せる。X4 の dandori の側、X5、X6 の検査をここで仕上げる。chobo の振替を流すランナーは、chobo の `tests/common/servers.rs` の PostgreSQL と TigerBeetle の立て方を使う。

**したこと（前半。dandori の構文と、すべてのプラットフォームと、確かめ方。dandori の担当）**：

- dandori に `use dates <名前> from "<file.cal>"`（下に `lambda`・`local`）と `use book <名前> from "<file.book>"`（下に `lambda`）を足した。日付のファイルの日付は `<名前>.<日付>(<入力>: …)` で規則と同じく呼び、`{ day: date, at: timestamp }`（時刻を言わない日付は `day` だけ）を返す。帳簿の振替の操作は、タスクの呼び出し方 `book <帳簿>.<振替>.<操作>`（`do`・`hold`・`post`・`void`）で、`hold`・`post`・`void` は仮押さえのレコード `<帳簿>.<振替>`（キーの引数と `state`）を返す。仮押さえは `case <名前> : <帳簿>.<振替> follows <帳簿>.<振替>` で案件になる。新しい型 `date` と、その文を走らせた時刻を読む式 `now` も足した。細部は dandori の DESIGN 1.16 と 2.7（根の DESIGN 7.7、7.8）。
- 事実は ritsu の口から読む。日付の口（koyomi）と帳簿の口（chobo）を `dandori::cli::run_with_ports` と `dandori::sources::with_ports` で受け取り、dandori のクレートは koyomi と chobo に `[dependencies]` で依存しない（テストは `[dev-dependencies]` でつなぐ）。口を持たないクレートのバイナリでは、規則と同じく E018（読めないものの最初の宣言で一度、exit 2）。
- 七つの出力先のすべてに作った（DESIGN 7.8）。`now` もどのプラットフォームでも作れ、E050 にしたものは無い（DESIGN 7.7）。参照インタプリタの見え方（`View`）、シナリオ、E040（durable functions は `now` の step を数える）と E050（呼ぶ日付と帳簿の `lambda`）、`doc`、ブラウザで試すページ（日付と帳簿の事実も記録する）、参照（`ritsu_ports::References`）にも入れた。口の `BookFacts` に `typescript`・`python`・`go`、`Dates` と `Books` に `joined` を足した（DESIGN 3.2）。
- 例 `examples/invoice`（英語の版と日本語の版。どのプラットフォームでもそのまま動く）、テストのフロー `tests/flows/dates_and_books.flow`、検査のテストのファイル四つ（`books.flow`・`book_refusals.flow`・`dates.flow`・`dates_and_books_targets.flow`、golden は英語と日本語）を足した。
- 確かめ方：例とテストのフローのすべてのシナリオを、七つの出力先と九つのランナーで参照インタプリタと突き合わせる（時計はどのランナーも `2026-03-31T15:30:00Z`）。日付のつなぐコードは koyomi の `vectors` の全件で koyomi と比べる。帳簿の操作は、TypeScript・Python・Go の `Transport` と Step Functions の Lambda 関数のコードから、chobo のクライアントで、chobo のテストと同じく立てた PostgreSQL と TigerBeetle の上で流し、chobo の参照インタプリタと比べる（`books_run_on_postgres_and_tigerbeetle`、170 件ずつ）。dandori の全体（platforms の段）を一度回し、104 件のうち 102 件が通った（落ちた 2 件は負荷のかかったときの揺れで、重いテストを一件ずつ回したときは通った）。重いテスト十二は一件ずつ回して全部通った（SKIP 0）。
- 替える前（ff11288）と後で、例とテストのフローの全部（`check` を英語・日本語・JSON で、`scenarios`、`doc` を二つの形と二つの言語で、`build` を七つの出力先で、939 回）の出力を比べ、違ったのは新しい書き方を使うフローと、`tests/fixtures/values.flow` の `check`（型の一覧を言う注に `date` が増えた）だけだった。
- 前半の担当が残した三つ（`ritsu dandori` に日付と帳簿の口を渡すこと、X4 の dandori の側と X5 と X6、`crates/ritsu-model/tests/dandori.rs` に `Ty::Date` と `TExpr::Now` を足すこと）は、どれも後半とほかの担当が済ませた（下と、F.6）。

**したこと（後半のうち、言語をまたぐ検査。xa の担当）**：

- dandori の流れの口に `crossings` を足した（`crates/dandori/src/crossings.rs`、`ritsu_ports::Flows::crossings`、DESIGN 3.2）。規則・koyomi の日付・chobo の振替を呼ぶところと、渡す値が来うるところ、期限のある仮押さえの確定と取消までの長さの最小と最大を渡す。koyomi の日付の口に `span` と `input_for` を足した。
- `ritsu check` が dandori に日付の口と帳簿の口も渡すようにした（`checked_with`、`Joined::ports`）。前は日付と帳簿を使うフローが E018 だった。
- ritsu-cross に X3 の (a)（`src/dates.rs`、E202・W202）、X4（`src/transfers.rs`、E203・W203・E204・W204）、X6（`src/dates.rs`、E205・W205）、X5（`src/holds.rs`、E206・W206）を作った。判定は `src/borders.rs`（`days_given`、`amounts_given`、`amounts_hull`、`refusals_met`、`input_range`、`held_until`）。X2 の日の前提（`range from koyomi`）を、渡す値が koyomi の日付の日から来るとき決められるようにした（DESIGN 7.3〜7.8）。
- 台帳の `Repro::Later` を、英語の小さなプロジェクトの再現に替え、W203・E206・W206 を足した（13 のコード）。`docs/codes.md` と `codes.ja.md` を取り直した。
- テスト：`crates/ritsu/tests/cross.rs` に X3 の (a)（範囲の 3 つと `range from koyomi` の 3 つ）、X4（5 つ）、X6（3 つ）、X5（3 つ）のプロジェクトを足し、英語と日本語のテキストと JSON の `borders` を golden にした（新しい golden は 34）。`crates/ritsu-cross/tests/borders.rs` に X5 と、出どころごとの判定を足した。`crates/dandori/tests/ports.rs` に、流れの答え（長さの最小と最大を、待ち、分岐、ループ、リトライ、`on failure`、koyomi の日付の時刻までの待ち、仮押さえの前に読んだ `now` で確かめ、値の出どころも見る）を確かめるテストを足した。
- dandori の DESIGN の 0.3 と 7 章、koyomi の DESIGN の 11.1 に書いた。

**決めたこと**（★は作者が決めるべきだったかもしれないもの）：

- ★ X5 が成り立つ（期限は切れない）ときは何も言わない（DESIGN 7.7）。
- ★ X6 はカレンダーのデータの範囲と比べない（DESIGN 7.8）。
- ★ dandori に日付の範囲を書く書き方を足さない。ワークフローの入力や `now` から来る日は W205 になる（DESIGN 7.8）。
- ★ X4 の拒否の理由は、帳簿の境界の理由だけを比べる（DESIGN 7.6）。比べないと、`key_conflict` と `already_refused` の処理をどの `do` と `hold` にも求めることになる。
- X5 は取消（`void`）も数える。期限が切れると取消も拒否されるからである。
- X4 の拒否の理由は `do` と `hold` だけを数える。`post` と `void` は状態で拒否され、dandori が案件で確かめる。
- 規則と日付の呼び出しには上限が無いものとして数える（`.flow` に時間の上限を書けない）。コールバックとイベントのタスクが `timeout` の無いときに使う一日も、プラットフォームの生成の決まりなので数えない。
- 長さの計算（分岐とループと `wait until`）は dandori に置き、ritsu-cross は有効期限と比べるだけにした。分岐をたどるには dandori の文を知っている必要があり、koyomi の日数は日付の口で読める。ritsu-cross の判定が一つの比べになり、Lean のモデルが短くなる。
- E202、E205 の例には、koyomi がその日を返す最初の入力を書く（DESIGN 7.1 の「koyomi の入力の日付」）。そのために `Dates::input_for` を足した。

**確かめたこと**：触ったクレートの関わるテストを、項目を書き終えたところで一度ずつ回した（ritsu-cross の全部、ritsu の `cross` と `codes`、dandori の `ports` の新しいテスト）。落ちたのは、dandori の新しいテストで置いた期待の三つ（行の数え違い、`on failure` で通らない分岐、`timeout` の無い取消のあとの失敗の上限）で、一つ目と三つ目は期待を、二つ目はフローを直して、そのテストだけを回し直した。最後に、ritsu・ritsu-cross・ritsu-ports・ritsu-project・dandori（重い十二を `--skip`）・koyomi・chobo の全部と、rulec の `ports`・`days`・`codes` を、ツールを全部つないだ環境（PostgreSQL つき、TypeSafe の鍵つき）で一度回した。376 件が通り（ritsu 38、ritsu-cross 6、ritsu-ports 3、ritsu-project 5、dandori 93、koyomi 156、chobo 75）、落ちたものは 0、SKIP 0、ignored 1（ritsu の `one_version_for_the_workspace_and_every_crate`。F.7 まで外してある）、9 分 17 秒。rulec の三つは 24 件が通り、26 秒。コマンドの出力を前のバイナリと突き合わせた。`ritsu check` を 14 のディレクトリに英語・日本語・JSON で 42 回、`ritsu dandori check` を dandori の例と `tests/flows` のフローの 50 本に走らせ、違ったのは、日付と帳簿を使うフローを含む三つのディレクトリ（`examples/invoice`、`tests/flows`、`tests/fixtures`）の `ritsu check` の 9 回だけだった（前は E018 で確かめていなかったものが、dandori の検査の結果と言語をまたぐ検査の結果を出すようになった）。`ritsu dandori check` は一字も違わない。

**したこと（後半のうち、決められない X2 の前提を生成コードで確かめること。xb の担当）**：

- 決められない X2 の前提を、dandori の七つのプラットフォームの生成コードが、ワークフローを走らせたときに確かめるようにした（DESIGN 7.4 の 3 と「E.5 で作った形」、dandori の DESIGN 1.17）。ritsu-ports に口 `Undecided` と `UndecidedPrecondition`、ritsu-cross にその実装 `UndecidedCalls`（`ritsu check` と同じ判定）、dandori に確かめる文 `TK::Check`（`src/prechecks.rs` が置く場所を決めて入れる）、七つの生成器、参照インタプリタ、シナリオ（前提が保たれる実行と破れる実行）、図、E040 の見積もり。`ritsu dandori` が ritsu-cross の答えを渡す（`run_with_undecided`）。取り込むときに、`UndecidedCalls` は `Flows::crossings` でフローを読み、`decide` に日付の口も渡す形にした（担当の版は規則の口だけの `rule_calls`）。
- 規則の日付の入力と出力を、つなぐコード（`rules.ts`、`rules.py`、Lambda の Python、`rules.go`）で日の番号に直すようにした。rulec の生成したコードは日付を 1970-01-01 からの日数で受け取り、返すが、つなぐコードは数として読んでいた。
- テストの材料：dandori の `tests/flows/preconditions.flow`（英語）と `preconditions.ja.flow`（日本語）、規則 `refund_check.rule`・`settlement.rule`（`range from koyomi`）と日本語の `返金の確認.rule`・`精算.rule`、koyomi の `payment_terms.cal` と `支払条件.cal`。doc の golden 4 つ。
- テスト：dandori のテストは、フローが呼ぶ規則の前提をすべて決められなかったものとして渡し（`EveryUndecided`）、rulec に koyomi の口をつなぐ（`range from koyomi` の規則を読むため）。どのプラットフォームでも参照インタプリタと突き合わせる。`crates/ritsu/tests/dandori.rs` に、`ritsu dandori build` が二つのフローを七つのプラットフォームで dandori のテストと同じに書くこと、決められる呼び出し（成り立つもの、E201 の例があるもの）の生成物が ritsu-cross を通さないときと一字も違わないことを確かめるテストを足した。

**決めたこと**：

- ★ 確かめる場所：値ができたところですぐに、ただし呼び出しに必ず届く場所に限る（DESIGN 7.4）。A の段階の「その値を作ったタスクの直後」をいつもそうすると、規則を呼ばない実行まで落とす。
- ★ 破ったときのエラー：新しい名前 `Dandori.BrokenPrecondition`。`Dandori.BadResponse` と同じく、リトライせず、`on` でも `on failure` でも処理しない。タスクの結果が型と範囲には合っているので、BadResponse と分けた。
- 口は ritsu-cross が出し、dandori が受け取る（`Undecided`）。dandori は ritsu-cross に依存しない。
- `check` は確かめる文を入れない（診断は変わらない）。`build`・`run`・`scenarios`・`doc` が入れる。
- 確かめる文の理由の文は英語の一文で、値を入れない（どのプラットフォームでも同じ文になり、突き合わせられる。値は実行の履歴にある）。
- 並びの合計と長さの前提は、確かめる文にしない（dandori は並びをたどる規則を呼ばない。E005）。

**確かめたこと**：dandori の tools の段で 100 件が通り、落ちた 5 件（doc の材料の規則が koyomi を要するのにテストがつないでいなかった 2 件と、上の日付の不具合の 3 件）を直して回し直した。ritsu-model は 5 件、ritsu・ritsu-cross・ritsu-ports は 43 件が通った（ignored 1、SKIP 0）。重いテスト十二を一件ずつ回し、全部通った（SKIP 0）。dandori の全部（platforms の段、1,767 秒）は 105 件のうち 102 件が通り、落ちた 3 件（負荷の中の Temporal のタイムアウトなど）は一件ずつ回し直して通った。

### E.6 `ritsu run`

DESIGN 7.9。dandori の参照インタプリタに、rulec の評価、koyomi のインタプリタ、chobo のインタプリタをつなぐ。テストは、E.1 のプロジェクトのフローを、規則と期日と帳簿を計算しながら流し、結果を golden にすること。

**したこと**（E の二つ目の部分）：

- `ritsu dandori` と `ritsu check` に、koyomi と chobo の口をつないだ（`crates/ritsu/src/main.rs`、`crates/ritsu-project/src/check.rs`）。日付か帳簿を使うフローが E018 で止まらなくなった。担当の版は dandori に `ports::Engine::checked_with_ports` を足していたが、言語をまたぐ検査の担当が同じ役目の `checked_with(root, files, &ritsu_ports::Ports, lang)` を足していたので、取り込むときにそれ一つにした（DESIGN 6.1）。`ritsu dandori` は、決められない前提の担当の `run_with_undecided` にまとめた（DESIGN 8.1）。
- `ritsu run <file.flow> --scenario <file.json> [--target <target>] [--format json]` を作った（`crates/ritsu/src/run.rs`。`ritsu` のライブラリの `pub mod run`）。dandori の参照インタプリタに、結果をシナリオのほかから受け取る口（`interp::Answers`）を足し、rulec・koyomi・chobo に結果を計算させる `dandori::computed` を書いた（DESIGN 7.9）。帳簿の口の `Ledger` に `accounts` と `holds` を足した（DESIGN 3.2、chobo の DESIGN 8 章）。シナリオを読むところは `ritsu_base::fs` を通す（DESIGN 4.15）。
- テストのプロジェクト `crates/ritsu/tests/projects/invoice/` を作った。英語の版（`invoice.flow`、規則 `rules/payment_method.rule`、日付 `dates/payment_terms.cal` とカレンダー `calendars/weekdays.cal`、帳簿 `books/stock.book`、シナリオ五つ）と、その横の日本語の版（`invoice.ja.flow`、`支払方法.rule`、`支払条件.cal`、`平日.cal`、`在庫.book`、シナリオ `*.ja.json`）で、日本語の版は英語の版の訳である（同じ日に同じに流れる）。日付とカレンダーは dandori の `examples/invoice` からコピーし、帳簿は有効期限を 30 日にし、規則は新しく書いた。dandori の検査の直し（F.6 の E020）で、二つのフローの `on failure` を dandori の `examples/invoice` と同じ形に直した（`put_back`・`棚に戻す` に `already_posted` を足して受け、取れない腕を消した。どのシナリオも `on failure` に入らないので golden は変わらない）。
- テスト：`crates/ritsu/tests/run.rs`（6 件。golden は英語と日本語のテキストと JSON の 20 個）、`crates/ritsu/tests/dandori.rs`（`ritsu dandori` と `ritsu check` が日付と帳簿を読むこと、2 件）、dandori の `computed` の単体テスト（日と時刻の読み書き）。`ritsu check` で dandori の `examples/invoice` を確かめるテストの期待は、取り込むときに、言語をまたぐ検査の W205 が二つと W206 が六つの 8 件の警告に合わせた（DESIGN 8.3）。

**決めたこと**：

- ★ `ritsu run` に `--target` を足した（DESIGN 8.1 の案には無かった）。流すのは `dandori run` と同じインタプリタで、一つの生成パッケージ（9.3）と突き合わせるには Temporal の見え方が要るからである。
- ★ 走らせる前の帳簿は、シナリオの `books` に、`use book` の名前ごとの操作の並びとして書く。操作の形は `chobo run` のシナリオのもの（`op`、`kind`、`args`、`amounts`）にした。帳簿に入れる値を、chobo の振替を通さずに書けるようにすると、帳簿の境界を破った状態から始められてしまうからである。
- ★ 時間の数え方：`wait`、リトライの前の待ち、タイムアウトした呼び出しのタイムアウトだけを数え、結果が返った呼び出しは 0 にした。`now` は動かさない（dandori の参照インタプリタの決まりのまま）。
- 結果をシナリオの形にして参照インタプリタに渡す形にした（`dandori run` の意味を一字も変えないため。`replay` で確かめられる）。結果の `cause` は、シナリオの結果には無いキーで、計算した結果だけが持つ。
- 値の渡し方と読み方は dandori のもの（生成するコードと同じ）なので、言語に結果を計算させる部分を ritsu の入口ではなく dandori のクレートに置いた。dandori は口だけを通して rulec・koyomi・chobo を使い、言語のクレートには依存しない（DESIGN 3.1）。
- koyomi の日付の評価（`Dates::eval`）はファイルの入力を全部要るので、その日付が読まない入力には範囲のいちばん小さい値を渡す。koyomi の検査を通るファイルでは、範囲のどの入力でもどの日付も計算が止まらないので（koyomi の E202〜E204）、結果は変わらない。
- `Joined` に `dates()` と `books()` を足さず、入口で `j.koyomi.clone()` と `j.chobo.clone()` を渡した。`Joined::dates()` は rulec が読む口（`Arc<dyn Dates + Send + Sync>`）で、dandori が受け取る `Rc<dyn Dates>` には渡せない。
- テストのプロジェクトの名前を `invoice` にし、英語と日本語のファイルを同じディレクトリに並べた（dandori の例の形）。
- `ritsu` の依存に serde_json を足した（入口は使ってよい。DESIGN 3.1 の決まり 4、`cargo xtask deps` は通る）。

**確かめたこと**：項目ごとに関わるテストを一度ずつ回し、落ちた 2 件（使い方の行の書き方をテストが取り違えていたこと、帳簿の終わりの勘定が `chobo run` と違ったこと。後者は口を直した）を回し直して通った。main に載せ直したあと、ritsu・dandori・koyomi・chobo・ritsu-ports・ritsu-project の全部（dandori の重い十二は外した）を一度回し、378 件のうち 377 件が通った（6 分 04 秒、ignored 1、SKIP 0）。落ちた 1 件は dandori の `jev_tasks_answer_on_typesafe` の TypeSafe への接続のタイムアウトで、その一件だけを回し直して通った。替える前と後のコマンドの出力を 11,177 回突き合わせ、違ったのは、日付か帳簿を使うフローの `ritsu dandori` の 972 回（前は E018 で止まった）と、`ritsu --help` の二つ（`run` が増え、dandori の説明が変わった）と、`ritsu check` で dandori の請求の例を確かめた二つだけだった。ほかの 10,201 回（例とテストの全部のフローの `check` と `scenarios`、全部のシナリオを八つのターゲットで `dandori run` したもの、通販のプロジェクトの `ritsu check`）は一字も違わない。`cargo xtask deps`（18 クレート）と `ritsu check ritsu.ctx` も通る。

### E.7 `ritsu gen`

DESIGN 9.3。TypeScript、Python、Go の一つのパッケージ。テストは、E.1 のプロジェクトのパッケージが `tsc --strict`、`mypy --strict`、`go vet` を通ること、`ritsu gen --check` が古い生成物を言うこと、ワークフローが規則と期日と帳簿をパッケージの中から読むこと。

**したこと**：

- `ritsu gen [<path>...] [--target typescript|python|go] [--out <dir>] [--check] [--books postgres|tigerbeetle] [--name <name>] [--module <path>] [--root <dir>]` を作った（`crates/ritsu/src/package.rs`、`src/cli.rs` の表、`src/main.rs` は数行）。プロジェクトは `ritsu check` と同じく `ritsu_project::Project::load` で読み、rulec・koyomi・chobo・dandori のファイルから、言語ごとに一つのパッケージを `<out>/<言語>/` に書く（`--target` が無ければ三つとも）。各言語の生成器はそれぞれのまま（DESIGN 9.1）で、パッケージそのものの部分（インデックスのファイルと、依存を書くファイル）は ritsu が書く。形は DESIGN 9.3（`crates/ritsu/tests/golden/gen/stockroom.txt` が、テストのプロジェクトの三つのパッケージの全部のファイルと、書いたもの、元のファイルを持つ）。
- ワークフローは、規則と期日と帳簿を、同じパッケージの `rules/`・`dates/`・`books/` から読む。dandori のモデルに `package`（`InPackage`）を置き、それがあるときだけ、Temporal の三つの SDK のビルドが読み込む先を替える。帳簿は、トランスポートが受け取るクライアントの型を、パッケージの `books/` のクライアントの型にした。`package` が無い `dandori build` の生成物は前と同じ（dandori の DESIGN 4.2）。
- 生成物の頭を、四つの言語でそろえた（DESIGN 9.2。rulec の §15.175）。ritsu-emit の `header` に `VERSION`、`generated(tool)`、`Source`、`Origin`、`file_name` を置き、rulec・koyomi・chobo・dandori がそれで書く。生成パッケージのモジュールの書き方は rulec の §15.176 に書いた。
- `--check` は書かずに、無いファイル、古いか手で直したファイル、前の `gen` が書いて今は書かないファイルを挙げて 1 で終わる。書くときは、前に書いて今は書かないファイルを消す。
- パッケージの Python が `mypy --strict` を通るように、dandori の Temporal の Python（と同じ部品を使う pydantic-graph と Step Functions の Lambda の Python）と、chobo の Python のクライアントの型の書き方を直した（振る舞いは変えていない。dandori の DESIGN 0.3、chobo の DESIGN 8.1）。chobo の `target::build` は、帳簿のファイルの名前とハッシュ（`Origin`）も受け取る。
- テストのプロジェクト `crates/ritsu/tests/projects/stockroom/`（英語）を足した。rulec の規則 `delivery`（GBP の額で便を決める。この例のために書いたもの）、koyomi の日付のファイルとカレンダー、chobo の帳簿（三つは dandori の `examples/invoice` のコピー）、三つを使う dandori のフロー `orders/order.flow`。取り込むときに、このフローを dandori の検査の直し（F.6 の E020）に合わせた（`put_back` が `already_posted` を宣言し、`on failure` の `held` の腕で受ける。取れない腕 `voided`・`expired` を消した）。
- テスト `crates/ritsu/tests/gen.rs`（6 件）：三つのパッケージを帳簿の二通りで `tsc --strict`・`mypy --strict`・`go vet` と gofmt にかけること（通販は TypeScript だけ）、フローの import がパッケージの `rules/`・`dates/`・`books/` を名指すこと、フローの規則と日付のアクティビティがパッケージのモジュールを通って答えること（Python と Go）、`--check` が古いものを英語と日本語で挙げること、どのファイルの頭も DESIGN 9.2 の形であること、パッケージにできないものをエラーにすること。

**決めたこと**（どれも DESIGN 9.2〜9.5 に理由を書いた）：

- ★ Go のパッケージには `go.mod` を書かない（import のパスは `--module`。モジュールとバージョンは `doc.go` に）。
- ★ パッケージの名前の既定は `generated`。
- ★ 帳簿のクライアントは `--books postgres|tigerbeetle` で選び、既定は PostgreSQL（SQL も書く）。
- ★ パッケージの生成物のコメントは `--lang` の言語で書く。`--check` は書いたときと同じ言語で走らせる。
- ★ 頭のバージョンは ritsu のバージョン（F.7 でそろえるまでは 0.1.0 で、rulec の生成物の頭も 0.22.1 から 0.1.0 になる）。★ 頭のハッシュは 16 桁（rulec は 12 桁から延びた）。日本語の頭の種類の語は、言語のファイルに書く語にした（rulec は前は `規則`）。
- パッケージの同じファイルを二つのファイルが書くときは、名前を替えずに exit 2 で止まる。ワークフローが読む規則・日付のファイル・帳簿がプロジェクトのファイルでなければ exit 1 で止まる。
- 予約語の表は一つにしない。インデックスのファイルと Go のディレクトリが import するモジュールの名前だけを、ritsu-emit の標準の表で確かめる。
- ★ dandori の生成物に `Books` という名前が増える（パッケージの中だけ）。dandori と chobo の Python を `mypy --strict` に通すため、それぞれの単独の出力も変えた（型の書き方だけ）。

**確かめたこと**：`crates/ritsu/tests/gen.rs` の 6 件が通る（SKIP 0）。頭を替えたあと、koyomi・chobo・ritsu-emit のテストが全部通り、dandori は tools の段で全部通り、rulec は 745 件が通った（ignored 1）。dandori のサイトの例のページと `presets.json` を記録し直し、`rulec.wasm` と `dandori.wasm` を作り直した。dandori の重いテストは、生成物の変わった五つ（Temporal の Python、pydantic-graph、言語をまたぐ Temporal、記録した 45 の履歴の再生、LocalStack）を一件ずつ回して通した（SKIP 0）。替える前と後の出力を突き合わせ、違ったのは、rulec の `gen` と `doc --format html`（中に持つ JavaScript の頭）、chobo の `build`、dandori の `build` と `doc --format html` だけで、書いたファイルは頭の行（dandori の TypeScript と Go は `BUILD_ID` も）と、dandori と chobo の Python の型の書き方のほかは変わらない。koyomi は一字も変わらない。全体（ritsu と、生成器を触った言語の全部。dandori の重い十二は外した）は 783 秒で 1,108 件が通り、落ちた 3 件（chobo の TigerBeetle の有効期限 3 秒を混んだ機械で過ぎたこと、dandori の Jev の接続のタイムアウト、koyomi の DESIGN.md の抜粋の塊の言語）は一件ずつ回し直して通った。

### E.8 処理系自身の地図

DESIGN 3.4、7.13。sakai が Rust のクレートの依存を確かめられるようにし（cargo-deny の `wrappers` を書く形を試し、効かなければ `cargo metadata` を読む形）、`ritsu.ctx` を根に置く。わざと言語のクレートに別の言語のクレートを依存させた変異で、sakai がその行を名指すこと。CI の `fast` のジョブに足す。

**したこと**（E の最初の部分）：

- sakai が Rust のクレートの依存を確かめるようにした（sakai の DESIGN 7.7）。地図の `code rust "<パス>"`（ワークスペースの `Cargo.toml` のあるディレクトリ）、公表された言語の `crate "<ディレクトリ>"`（見出しはクレートの名前の `-` を `_` にしたもの。1.4）、成果物の `.rs` とクレートの `Cargo.toml`。依存は `cargo metadata --format-version 1 --no-deps --offline` に尋ね、パスで書いた `[dependencies]` と `[build-dependencies]` を、依存を書いたマニフェストの行からの参照として、ほかの言語と同じ境界の検査（E201〜E206）にかける。Cargo に尋ねられなければ E107（新しいコード）。sakai の `src/cargo.rs`（依存の行を探すところを含む）、`refs.rs` の `Kind::Crate` と `crate_crossings`、`patterns.rs` のクレートの E302 と E301。
- sakai のテストの土台の地図に `tests/maps/rust`（英語の名前の、四つのクレートのワークスペース）を足し、変異を四つ（`E107_no_cargo_manifest`、`E201_a_crate_with_no_relationship`、`E202_a_crate_inside_another_context`、`E302_a_crate_named_otherwise`）と、その golden を英語と日本語で足した。台帳の E107 の再現は、Rust のコードを持つ一つのコンテキストの地図（英語）。
- `ritsu.ctx` を根に、12 のコンテキストのファイルを `contexts/` に置いた（DESIGN 3.4）。`ritsu check ritsu.ctx` は `ritsu.ctx: ok — 12 contexts, 27 relationships; 356 artifacts, each in one context; 50 crossings checked (rust 50)` と言い、0.03 秒ほどで終わる。
- `crates/ritsu/tests/map.rs`：リポジトリの地図が通ること。ワークスペースのマニフェストと地図を一時ディレクトリにコピーし、koyomi のクレートが rulec のクレートに依存する変異を入れると、sakai が `crates/koyomi/Cargo.toml` の足した行を E201 で名指すこと（英語と日本語）、同じ依存を `[dev-dependencies]` に書けば通ること、土台（`ritsu-units`）が言語（`chobo`）に依存しても名指すこと。
- CI の `fast` のジョブに `cargo run --locked -q -p ritsu -- check ritsu.ctx` を足した（`cargo xtask deps` の次）。
- `ritsu check` の要約で、地図が言語のものでないファイル（`Cargo.toml`）のせいで通らないとき、地図のファイルを「通らない」と数えるようにした（E.2 の形では、エラーの数は出るのに `all pass` と言っていた。終了コードは前から 1）。

**決めたこと**：

- ★ 依存は、cargo-deny の `wrappers` の設定を書く形ではなく、sakai が `cargo metadata` に尋ねる形で読む。cargo-deny 0.20.2 は、`exclude-dev = true` を書けばワークスペースの中の依存にも効いたが、指す行が `deny.toml` の行で、「どのクレートも依存してはいけない」クレートを書けず、CI で取ってくる必要がある（DESIGN 3.4、sakai の DESIGN 7.7）。そのかわり、sakai の `check` が、地図に `code rust` があるときだけ子プロセスを一つ走らせる（sakai の DESIGN 6 章の例外。ネットワークにはつながない）。
- ★ sakai に、Rust のクレートを公表された言語にする書き方 `crate "…"` と、地図の `code rust` を足した。キーワードに `rust` と `crate` が増え、名前に使えなくなる。
- dev-dependency は数えない（DESIGN 3.3 が許すもので、cargo-deny の `exclude-dev` と同じ）。ワークスペースのためだけのマニフェスト（`[package]` の無いもの）は成果物にしない。
- 地図の範囲は `crates` から、言語がわざと通らないファイルを持つ 17 のディレクトリ（言語の `tests` と `examples`、rulec の `experiments` と `website`、sakai の `tools`）を除いたもの。確かめるコマンドは `ritsu check .` ではなく `ritsu check ritsu.ctx` にした（DESIGN 7.13 を直した）。
- 土台の五つのクレートは一つのコンテキスト（`Base`）にした。土台の中の向きと外のクレートは、`cargo xtask deps` が確かめる。
- コンテキストの名前は英語にした（作者の決まり）。言語のコンテキストは扱うものの名前（`Rules`、`Workflows`、`Calendars`、`Books`、`Claims`、`Requirements`、`ContextMaps`）で、別名がクレートの名前である。言語の名前（`rulec` など）は sakai のツールの語なので、コンテキストの名前にはできない。

**確かめたこと**：sakai と ritsu のテストを一度回し、114 件が通った（SKIP 0）。`cargo xtask deps` は 17 のクレートで通る。sakai の B〜D の変異の golden は一字も変わらない（Rust のコードを書かない地図の振る舞いは変えていない）。

### E.9 E の完了の条件

1. 根から `cargo test --workspace --no-fail-fast -- --nocapture` が全部通り、この機械で SKIP は 0（B.8 で許したものを除く）。dandori の `platforms` の段も、ほかと同時でなく一度通す。
2. `ritsu check crates/ritsu/tests/projects/shop`（と日本語の `通販`）が、各言語の `check` と同じ診断と、言語をまたぐ検査の結果を出す（golden）。
3. DESIGN 7.2 の X1〜X7 と X13 のそれぞれに、通るプロジェクトと落ちる変異があり、結果が三つ（示した、例がある、決められない）のどれかで出る。
4. `ritsu run` と `ritsu gen` のテストが通る。
5. DESIGN の 7 章と 8 章の「形の案」を、実物の出力に差し替えた。
6. 報告に、検査ごとの再現と、足した書き方（rulec の範囲の宣言、dandori の `use dates`・`use book`・`now`）の最終の形を書く。

**E の終わりの状態**（担当の記録を取り込むときに書いた）：

1. まだ。担当はそれぞれ自分の木で全体を回した（E.4 の担当は 1,608 件、ブラウザで試すページの担当は 1,667 件。どれも SKIP 0）。全部を取り込んだ木で、根から全体を一度、dandori の platforms の段を一度回すのは、指示する側が最後にする（7.10）。
2. 済んだ。日本語の `通販` と英語の `shop` の両方（golden は `通販.*` と `shop.*`。DESIGN 8.3）。
3. 済んだ。X1 は言語の検査（dandori の E003、rulec の E065）、X2〜X6 は `crates/ritsu/tests/cross.rs` と台帳の再現（成り立つ、例がある、決められない）、X7 は `ritsu run` のテスト（証明ではない）、X13 は `crates/ritsu/tests/map.rs`（DESIGN 7.2 の表）。
4. 済んだ。`crates/ritsu/tests/run.rs`（6 件）と `gen.rs`（6 件）。
5. 済んだ。DESIGN 7.2 の表、7.5 と 7.8 の案、8.1 のコマンド、8.3 の例を実物に差し替えた（3.2 と 5.1 のコード、6.4 の表、9.3 の形も）。
6. 済んだ。検査ごとの再現は DESIGN 7.1、足した書き方の最終の形は、rulec の `range from koyomi`（DESIGN 7.5、rulec の §15.174）と、dandori の `use dates`・`use book`・`now`（DESIGN 7.7、7.8、dandori の DESIGN 1.16、2.7）。

## 6. 段階 F：仕上げ

### F.1 yuen の段階 D

済んだ。yuen の PLAN の D.1〜D.7 を、ritsu の中で、土台と口を使う形に直して作った（2026-10-04。yuen の PLAN の 4 章が、書き直した D とその結果）。

- doc（`yuen doc`、`src/doc/`）：Markdown と一枚の HTML。HTML の枠は ritsu-base の `docpage`、条文は `trace` と同じ ritsu-base の `quote_lines` でコピーから引く。グラフは `yuen api` の JSON から組む。1〜4 の段のエラーがあればページを作らない（DESIGN 4.8）。
- 例（`crates/yuen/examples/`）：英語を先にした。`osha`（eCFR の 29 CFR 1910.157、rulec の表）、`greeter`（`greeter.req` と `greeter.ja.req`、geas の主張と記録、`affected` の差分）、`payment_terms`（英語と日本語、koyomi の日付と条件、祝日の表を借りる）、`refunds`（英語と日本語、chobo の勘定と振替）、e-Gov の法令にしか無い `civil_code_periods`、`civil_code_periods_reread`、`stamp_tax`（日本語だけ、`<英語の名前>.ja.req`）。名指すもの（rulec の表と出力、koyomi の日付と条件、geas の主張、chobo の勘定と振替）は口から読む。
- ★わざと止まる例 `civil_code_periods_reread` は E303 が一つ（A の段階の見込みは三つ）。D.7 で koyomi の日付の端がその日付の定義の文になり、書き換えた日付へのリンクだけが止まるため（yuen の DESIGN 15 章、根の DESIGN 6.4）。★コピーしたファイルは koyomi のいまの名前（`civil_code_period_end.ja.cal`）のまま。★greeter・payment_terms・refunds の出どころは `decided`（この例のために決めたもの）。
- `docs/`（`reference.md`、`codes.md`、`codes.ja.md`）、README.md と README.ja.md（日本語は一から書いた）、`skills/yuen`（`SKILL.md` と `skills/sync.sh`）、`THIRD_PARTY_NOTICES.md`。入れ方は ritsu の `cargo install --git https://github.com/i2y/ritsu --locked ritsu`、コマンドは `ritsu yuen …`。
- テスト：`tests/doc.rs`（例の十の `.req` の四十の golden、外を読まない HTML、引いた条文がコピーの行であること、Chrome の画像四枚）、`tests/examples.rs`、`tests/docs.rs`、`tests/skill.rs`。`cargo test -p yuen -p ritsu --no-fail-fast -- --nocapture` が通る（yuen 225 件、ritsu 57 件と ignored 1 件、SKIP 0）。
- 替える前と後の出力を、テストの材料と変異の全部（151 のプロジェクトに `check` 四通り、`api`、`export` 二つ）、ヘルプ、`explain --all`、`affected` の 1,086 回で比べ、1,081 回が一字も違わなかった。違ったのは、`doc` を足したヘルプ（`yuen --help`、引数の無い `yuen`、日本語の `--help`、`yuen help doc`、`yuen doc --help`）の 5 回だけである。ほかに、`from` の無い要件を `affected` が書くときの空の `from ;` を直した（例 `greeter` で出る。替える前の材料には無い。yuen の DESIGN 8 章）。

### F.2 sakai の段階 D

済んだ。sakai の PLAN の D.1〜D.7 を、ritsu の中で作った。doc（`sakai doc`。HTML の枠は ritsu-base の `docpage`、図は sakai の `draw.rs` の SVG と Mermaid）、例の README（英語の例の `examples/shop/README.md` と日本語の例の `examples/shop.ja/README.ja.md`）、`docs/`（`reference.md`、`targets.md`、`codes.md`、`codes.ja.md`）、`README.md` と `README.ja.md`、スキル（`skills/sakai/` と `skills/sync.sh`）、`THIRD_PARTY_NOTICES.md` を `crates/sakai/` の下に置いた。テストは ritsu-testkit で書き（`tests/doc.rs`、`tests/docs.rs`、`tests/skill.rs`、`tests/examples.rs` に一つ）、この機械で SKIP は 0。入れ方は ritsu と同じ（`cargo install --git https://github.com/i2y/ritsu --locked ritsu`、`ritsu sakai …`、リンクの名前 `sakai`）で、sakai だけのリポジトリの URL は書かない（★`Cargo.toml` の `repository` も ritsu にした）。doc のために、sakai の口のまとまりに koyomi の `Dates` を足し、ritsu-project の `Joined::sakai` がそれを渡す（★共有のファイルを一行直した。DESIGN 3.2、4.8、8.6）。★CI の `tools.yml` も直した（新しい Mermaid のテストが CI で SKIP にならないように）。作ったものと変わった振る舞いは sakai の DESIGN の 10 章、11 章、12.5 にある。

残したこと：`crates/sakai/tools/mermaid/node_modules` は git に入れない。本のリポジトリで `npm ci --prefix crates/sakai/tools/mermaid` を一度走らせる（chobo と同じ Mermaid 11.17.2 と 12.1.0。無ければ sakai の Mermaid のテストは SKIP する）。

### F.3 ritsu の文書とスキル

- 根の README.md（英語）と README.ja.md（日本語。英語の訳ではなく一から書く）：何をするか、七つの言語とそれぞれの README への案内、`ritsu check` の出力（本物）、言語をまたぐ検査が言うこと（本物の診断）、入れ方、コマンド、どう確かめているか、ライセンス。README の出力と診断は、テストが実際に走らせて照らし合わせる（koyomi と chobo の `tests/docs.rs` の形）。
- DESIGN.md：A のスケッチと見込みを、実物と実際の数に差し替える。
- `skills/ritsu/`：プロジェクトを `ritsu check` で回す流れ、言語をまたぐ診断の直し方、どの言語のスキルを読むか。各言語のスキルは残す。
- koyomi と chobo の例とテストの材料は、英語を先にした（各言語の DESIGN 10.1、根の DESIGN 10.10）。ritsu の README とスキルで koyomi と chobo の例を見せるときは、英語の例（koyomi は England and Wales の `close_20th_pay_10th.cal` など、chobo は `examples/inventory/inventory.book` など）を先に見せ、日本語の版を並べる。
- sakai と yuen の例とテストの材料も、英語を先にした（各言語の DESIGN の 12.4 と 16.2、根の DESIGN 10.11）。ritsu の README とスキルで sakai の例を見せるときは、英語の `examples/shop/` を先に見せ、日本語の `examples/shop.ja/` を並べる。
- rulec のコーパスと例は、英語を先にした（rulec の DESIGN §15.177、根の DESIGN 10.10）。ritsu の README とスキルで rulec の例を見せるときは、英語の双子（`member_shipping_fee.rule`、`japan_stamp_duty_split.rule`、`order_lifecycle.rule` など）を先に見せ、日本語の版を並べる。

済んだもの：根の README.md と README.ja.md とその確かめ（`crates/ritsu/tests/readme.rs`）、`skills/ritsu`（`crates/ritsu/tests/skill.rs`）、各言語の README の入れ方、dandori の `--help`。DESIGN の 1.2（行数）と 10.1（テストの件数と時間）も、全部を取り込んだ main で測り直した（指示する側）。

**したこと**（F.3 の担当。DESIGN の 8.2、10.12、13.2 に足した文）：

- 根の README.md と README.ja.md の、出力、診断、コード、コマンド、リンクを、テストが確かめる（`crates/ritsu/tests/readme.rs`、7 件。F.8 の完了の条件 3）。`console` の塊の `$ ritsu …` と `$ <言語> …` を実際に走らせて塊の行と突き合わせ、コードの塊の行を、リポジトリの中のその言語のファイルの行と突き合わせ、本文が名指すコマンド、相対リンクとアンカー、七つの言語の表、`cargo install` の行を確かめる。README の文と出力は、実物と食い違うところが無かった。直したのは、テストが走らせられるようにした所（`console` の指定、`cd` の行、再現を指す一言）と、`cargo install` の入れ方に合わせたリンクの文の一か所である。★足したのは、スキルを案内する「AI エージェント向け」の節（言語の README と同じ並びで「コマンド」の前）だけである。
- `skills/ritsu/SKILL.md` を書き、テスト（`crates/ritsu/tests/skill.rs`、6 件）を足した。プロジェクトを `ritsu check` で回す流れ、出力の読み方、言語をまたぐ診断 13 個（E101、E201〜E206、W201〜W206）の読み方と直し方、`ritsu run` と `ritsu gen`、どの言語のスキルを読むか、人に残すこと。ページを読む人は「コードが実現すべきものを理解し、確かめる人」と書く。
- rulec、dandori、koyomi、chobo、geas の README の入れ方を、ritsu から入れる形（`cargo install --git https://github.com/i2y/ritsu --locked ritsu`、その言語だけなら `--locked <言語>`）に直した。言語ごとのリポジトリの URL は入れ方から消えた。★koyomi、chobo、geas、dandori のスキルの `compatibility` も同じにした（dandori のものは、規則に rulec のバイナリが要ると書いていたのも、いまの dandori に合わせて直した）。★rulec のスキルの `license: MIT` は、rulec の `Cargo.toml` と README の `MIT OR Apache-2.0` と食い違っていたので直した（元は `skills/header.md`。`skills/sync.sh` で SKILL.md を作り直した）。
- 実際に確かめた：ローカルのクローンから `cargo install --git file://… --locked rulec` と `--locked ritsu` を走らせると、入る実行ファイルは `rulec` と `ritsu` のそれぞれ一つだけだった（`ritsu` だけを入れた PATH に `rulec` は無く、`ln -s "$(command -v ritsu)" …/rulec` を作ると `rulec --version` が `rulec 0.22.1`、`koyomi check` が README と同じ行を出した）。
- dandori の `--help` の、規則を使うワークフローは `ritsu dandori` で走らせる、という文（★とそのそばの exit code の文二つ）に、日付のファイルと帳簿（`use dates`、`use book`）を足した。サイトの英語と日本語のページ、スキルのコピーも同じにした。

**したこと**（日本語の出力の読み直しと言い方。2026-10-04〜05。作者がブラウザで試すページで、日本語の出力の不自然さと、出力に設計の文書の節の番号が出ることを指摘した。DESIGN の 4.1、4.3、4.8）：

- chobo と koyomi の日本語の出力を読み直した（ja-a）。読む人にしてほしいことは「〜してください」、道具の振る舞いは主語を書き、chobo のキーの行は「<振替> は、<引数> と <引数> の組ごとに一度だけ動きます」の形にした。直した日本語の文字列は 243（chobo 83、koyomi 160）で、英語と言語によらない出力 1,131 ファイルは一バイトも違わない。golden、`docs/codes.ja.md`、koyomi の画像一枚、chobo と koyomi の DESIGN と README に貼った出力、ritsu の `check`・`cross` の日本語の golden を取り直した。
- geas、sakai、ritsu-cross、ritsu、ritsu-base の利用者に見える日本語を読み直した（ja-b）。実物（`--lang ja` の check、doc、explain、`--help`、`ritsu run`・`ritsu gen`、テストの材料と変異の golden）を英語と並べて読み、読む人にしてほしいことを「〜してください」に、道具がすることを「sakai は〜」「geas は〜」のように主語のある文にした。`explain` の直し方は三つの台帳（geas 34、sakai 59、ritsu 13）のすべてを直した。作者が見た sakai の E105 の注は「そのファイルを、rulec の検査を通るように直してください。検査を通らないファイルや読めないファイルからは参照を読み取れないので、sakai はその参照を確かめられません。」にした。直した文を含むソースの行は 383 行で、英語の出力 602 本は差が 0 だった。golden、`docs/codes.ja.md`（sakai、ritsu-cross）、日本語の出力を貼った文書（根の README.ja.md の E201 の注、geas と sakai の README.ja.md、sakai の DESIGN.md の診断の例 11 か所）を取り直した。取り込むときに、根のサイトの日本語の index に貼った `ritsu check` の出力を貼り直した。
- yuen の利用者に見える日本語を読み直した（ja-c）。例・テストの材料・変異の全部に `--lang ja` でかけた実物を英語と並べて読み、236 の文を直した。作者が見た E203 の注は「そのファイルを、rulec の検査を通るように直してください。検査を通らないファイルや読めないファイルからは成果物の定義を読み取れないので、yuen はその成果物のハッシュを取れません。」にした。設計の語「端」は出力から外し、「リンク元」「リンク先」「両端」「要件のハッシュ」と言うようにした。英語の出力 3,458 ファイルは差が 0。取り込むときに、日本語のブラウザで試すページの手順の yuen の見出しを今の出力に直した。
- dandori の利用者に見える日本語を読み直した（ja-d）。例とテストの材料の 105 の `.flow` に check・doc・scenarios・build を両方の言語でかけた出力と、`explain`・`--help` の実物を読み、直した日本語の文字列は 194 になった。英語の出力は 1,767 ファイルを突き合わせて差が 0 だった。日本語の golden（`tests/fixtures` 31、`tests/doc` 31）、サイトの図のページ 6 と貼った診断、dandori の DESIGN.md の出力を取り直した。
- 七つの言語の文書と出力に残っていた「承認する人」「approver」の言い方を、根の README の言い方（"the page for people"、「人が読むページ」、読む人は「コードが実現すべきものを理解し、確かめる人」）にそろえた（言い方の担当。作者の決め。`--audience approver` の値と業務の中の承認は残した。DESIGN 4.8）。利用者に見える出力から、設計の文書の節の番号を 391 か所外した（DESIGN 4.3）。rulec の E032 の注を直した。golden、サイトの図のページ、ブラウザで試すページの presets を取り直した。
- 取り込むときに、言い方の担当と読み直しの担当が同じ文字列を直したところ（dandori の E006・E018・doc の一文、yuen の E206）は、読み直しの新しい日本語を土台に、節の番号を外す直しを載せて解いた。rulec の日本語の出力の読み直し（下の二項）は、二人とも言い方の担当の直しが入った木の上に当て直し、ぶつかったところ（ja-e 15 件、ja-f 17 か所）は言い方の担当の新しい文を土台にして解いた。両方を入れたあと、dandori の golden と図のページを取り直し（変化なし）、ブラウザで試すページの wasm（`ritsu.wasm`、`rulec.wasm`、`dandori.wasm`）を作り直した。これで、作者がブラウザで試すページで見ていた日本語が新しくなった。
- rulec の利用者に見える日本語のうち、explain の台帳（`codes.rs`）と `doc` のページ（`doc.rs`）を除く全部を読み直した（ja-e、2026-10-05）。リポジトリの `.rule` 273 本の全部に `check`（text、json、`--terse`）をかけ、コーパスとページの規則には `--show-shadow`、`coverage`、`api`、`graph`、`certificate`、`schema`、`adapter`、`doc`、`diff`、`gen`（生成物のファイルの全部）も、`--help` は 21 のコマンドの全部、`fixtures lint`・`replay`・`diff` は合成した記録で、使い方の誤り 20 通りと MCP の `tools/list` も、英語と日本語の両方で実物を読んだ（4,544 ファイル）。直した日本語は `tr!` の日本語の側 267 か所と、MCP のつなぎの 3 か所（`tr!` を足した）。英語の側は一字も変えていない（`tr!` の英語 2,288 か所を突き合わせて差 0。出力でも、英語と言語によらないファイルが一バイトも違わない）。rulec の日本語の golden（`tests/golden/*.txt` 25 本、`tests/golden/json` 5 本、`tests/days/golden/*.ja.txt` 5 本）、ritsu の golden 3 本、rulec のサイトの日本語のページに貼った出力（`index.md`、`checks.md`、`scenarios.md`、`tour.md`、`compare.md`）、`checks` の図（`tools/make_checks.py` の言葉と `checks-ja*.svg`、図の URL の `?v=`）を取り直し、テストの期待値を五つのファイルで直した（DESIGN 4.1）。
- rulec の `explain` の台帳（112 項目のうち 75 項目。「いつ出るか」43、「直し方」59。見出しは診断の見出しと同じ文なので変えていない）と、`doc` が書く人が読むページ（読む人向け、顧客向け、HTML）の日本語を読み直した（ja-f、2026-10-05）。実物（`rulec explain --all --lang ja` の text・markdown・json と、リポジトリの `.rule` 273 本すべての `doc --lang ja` の三形）を英語と並べて読み、読む人にしてほしいことを「〜してください」に、rulec がすることを主語のある文にした。直したソースの行は 130（`codes.rs` 103、`doc.rs` 27）。英語の出力 1,002 本の差は 0（日本語は 688 本が変わり、大半はページの頭の刻印の一行）。`docs/codes.ja.md` を取り直し、ページの文を引いた文書（`website/rulec/docs-ja/examples.md` と、それを書く `tools/make_examples.py`、`docs-ja/tour.md`）の引用を、いまの出力の言い方に合わせた。dandori の図のページ（rulec の `doc` を埋め込む）も取り直した（DESIGN 4.1）。

**Agent Skills（2026-10-05 の朝、作者の指示で）**：作者が「ritsu と七つの言語は AI エージェントに使ってもらうためのものなので、Agent Skills を作って提供する」と決めた。八つを根の `skills/` に集め（各言語の `sync.sh` の書き先、geas の埋め込み、テストと README のリンクを合わせた）、配る仕組みと中身の読み直しを二人の担当に頼んだ。

「**したこと**」に足す：

- 八つの Agent Skills を配る仕組みを作った（sk-a の担当、2026-10-05。DESIGN 13.2）。Claude Code のプラグインとマーケットプレイス（根の `.claude-plugin/plugin.json` と `marketplace.json`）、`ritsu skills install` と `ritsu skills list`（`crates/ritsu/src/skills.rs`。`ritsu --help` とコマンドの表にも足した）、リリースの `ritsu-skills-v<版>.zip`（`packaging/skills.sh`、`release.yml`）、根の `skills/README.md` と日本語の版の `skills/README.ja.md`（八つの一覧と四つの入れ方）、根の README の「For AI agents」「AI エージェント向け」とコマンドの一覧、各言語の `crates/<言語>/skills/README.md` の入れ方の節（根の `skills/README.md` を指す短いものにした。権限の書き方と、geas のバイナリが自分のスキルを入れることは残した）。★rulec と dandori のサイトの入れ方のページ（英語と日本語）が、スキルを `crates/<言語>/skills/<言語>` からコピーする古いパスのままだったので、`ritsu skills install` と根の `skills/` に直した（rulec のページの「ほかに五つのファイル」も、いまの六つに直した）。テストは `crates/ritsu/tests/skill.rs` に 9 件、`tests/release.rs` に zip の行。

「済んだもの」に足す：Agent Skills の配り方（プラグイン、`ritsu skills install`、リリースの zip、案内とテスト）。

「残り」に足す：リポジトリを public にしてサイトを切り替えたあと、`/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json` と `/plugin install ritsu@ritsu` を一度走らせ、八つのスキルが `ritsu:<名前>` で読まれることと、手元に入るのが `skills/` だけであることを確かめる。リリースの zip は、最初のタグで `release.yml` が走ったときに確かめる（CI は止めてあるので、手元で `packaging/skills.sh` を走らせて zip ができ、中身が `skills/` と同じことだけを確かめた）。

- 八つのスキルの中身を、エージェントが使う流れで読み直した（2026-10-05、sk-b）。入れ方は八つとも ritsu だけにした：`compatibility` は「`ritsu` を PATH に（`cargo install --git https://github.com/i2y/ritsu --locked ritsu`）、言語は `ritsu <言語> <command>`、言語の名前のリンクでも同じ」。本文で走らせるコマンド（インラインのコードと、`console` の塊の `$` の行）は `ritsu <言語> <コマンド>` の形にした（出力の塊は、コマンドが出すものなので替えない）。rulec は、本文が AGENTS.md と共有で 499 行の上限もあるので、頭（`header.md`）で一度だけ呼び方を言う。各スキルの流れに、`explain` で一つのコードを引くこと、人が読むページ、プロジェクトなら `ritsu check`（koyomi は E202・E205、chobo は E203・E204・E206、dandori は前提・日・額・拒否・保留）を足し、ritsu のスキルの流れに「人に確かめてもらう」段（7）を足した。いまの実物から足したもの：rulec の `import std/<国>/<種類>` と `std/都道府県`、`range from koyomi`、dandori の、決められなかった前提を生成したワークフローが実行時に確かめること（`Dandori.BrokenPrecondition`）、`ritsu gen`、chobo の額 0、yuen の `export prov` の名前空間（`https://i2y.github.io/ritsu/ns/yuen#`）、ritsu のブラウザで試すページ、`ritsu skills install`（sk-a の形）。rulec の AGENTS.md から設計の文書の節の番号（§15.95）を外し、CI の例を `uses: i2y/ritsu@v0.23.0` にした。
- 切り替え（PLAN 7.10 のサイトの切り替え）のときに直すものに、次を足す：rulec のスキルの footer と AGENTS.md の 7 章のサイトの URL（`https://i2y.github.io/rulec/`）、dandori のスキルの最後の行（`https://i2y.github.io/dandori/`）。ritsu のスキルのブラウザで試すページのリンク（`https://i2y.github.io/ritsu/playground/`）は、切り替えまで 404（10/5 07:2x に確かめた）で、切り替えれば生きる。

### F.4 LSP

今回は作らない（作者の決め、2026-10-04。DESIGN 15 章）。A の段階の案は、`ritsu lsp`（標準入出力の JSON-RPC。外のクレートを使わない）で、診断（`ritsu check` の結果をファイルごとに）、定義へ移る（索引で）、型と範囲を見せる、中のものの一覧（`Items`）、rulec の整形を持つ形だった。

### F.5 ブラウザで試すページ

`ritsu-wasm`（wasm32-unknown-unknown。rulec と dandori と同じ「バッファの頭に長さを書く」決まり）と、ページ（複数のファイルをタブで持つ小さなプロジェクトに `ritsu check` を当て、言語をまたぐ診断を出す。各言語の `gen` と `doc`）。テストは、dandori の `tests/playground.rs` の形（node でモジュールを呼んでコマンドの出力と同じか、Chrome でページを開くか）。

済んだ。

**したこと**：`ritsu_base::fs`（ファイルの読み書きの入口）を置き、言語の check・gen・doc の道筋の読み書きを通した。`ritsu_base::paths::rooted`。geas を Unix でない対象でもビルドできるようにした。`ritsu` のクレートにライブラリ（`ritsu::check::run`）を持たせた。`crates/ritsu-wasm`（`playground.rs` と `wasm.rs`）。根の `website/` に、ページ（英語と日本語）、`playground.js`・`playground.css`・`projects.json`・`ritsu.wasm`、`tools/make_wasm.sh`、ページが開くプロジェクト（`website/playground/shop/` と、日本語版の `shop.ja/`）。テストは `crates/ritsu/tests/playground.rs`（四つ）と `crates/ritsu-base/tests/fs.rs`（六つ）。地図の `Entry` に ritsu-wasm を足し、CI の fast に wasm32 のコンパイルを足した（DESIGN 4.15、8.7、10.5）。

**決めたこと**：★対象は wasm32-unknown-unknown のまま、ファイルは土台の入口で読む（WASI を JavaScript で肩代わりする形は捨てた。DESIGN 4.15）。`ritsu check` はバイナリと同じ関数をそのまま呼ぶ。各言語の gen と doc は、書き出し先を引数に取るコマンドはそのまま呼び、標準出力に直接書くコマンドは、コマンドが呼ぶ関数を同じ順に呼ぶ（DESIGN 8.7）。ページは英語のプロジェクトで開き、日本語版を二つ目に置く。開いたときに、契約に値が一つ増えて三つの言語が答える状態にした。`ritsu` は ritsu-wasm を dev-dependency に持つ（DESIGN 3.3）。

**確かめたこと**：`crates/ritsu/tests/playground.rs` の四つ（ページの 738 の答えが、同じファイルを置いたディレクトリで走らせた `ritsu` のバイナリと一字も違わないこと、コミットした ritsu.wasm が 216 の問いにライブラリと同じ答えを返すこと、projects.json が `website/playground/` の今の中身であること、Chrome で英語と日本語のページを開くこと）と、`crates/ritsu-base/tests/fs.rs` の六つ。七つの言語のコマンド 4,927 回を替える前のバイナリと突き合わせ、実の違いは無かった（sakai の 9 回は、書いたファイルの場所を言う行に取り込んだ先のディレクトリの名前が出ていただけ）。パッチを一つずつ当てるたびに `cargo check` が通る。ワークスペースの全体は 1,667 件が通り（18 分 26 秒）、落ちた 1 件（dandori の Jev のテストの、TypeSafe への接続のタイムアウト）は回し直して通った。`ritsu check ritsu.ctx` は 58 crossings（ritsu-wasm の依存の八つが増えた）。

**足したこと（2026-10-04 夜、作者の指示「dandori と rulec のウェブサイトにあったプレイグラウンドの豊富な例がないのはちょっと」による）**：前の rulec と dandori のブラウザで試すページの例を全部、ritsu のページの選択に、言語ごとの組で並べた（rulec の五つの規則の英日、dandori のフロー 38 本と、それぞれが読むファイル）。中身は前のページと同じもと（rulec のコーパスとトップページの表、dandori の例）からテストが作り、もとが変われば projects.json が古いとテストが言う。空のプロジェクト（ファイル一つから始める）と、いまの中身を詰めた共有のリンク（「リンクをコピー」）を足し、dandori の前のページのリンク（`#flow=…`）も読むようにした。ページの文（英日）を直した。例を足して見つかった食い違い（ページの dandori が日付と帳簿の口を受け取っていなかった）を、`ritsu::languages::dandori` をバイナリとページで共有して直し、ritsu.wasm を作り直した（DESIGN 8.7）。

**確かめたこと**（例を足したあと）：`crates/ritsu/tests/playground.rs` の五つ（48 の例を前のページと突き合わせ、ページの 1,536 の答えをバイナリと、ritsu.wasm の 372 の問いをライブラリと突き合わせ、Chrome で 22 の画面を開く）。`website/build.sh` で組んで、英語と日本語のページを Chrome で開き、rulec の例の check、dandori の例の生成と人が読むページ、空のプロジェクトを見た。

**替えたこと（2026-10-05、作者の決め「前のページは、ritsu のページに例を全部持たせたうえで、切り替えのときに ritsu のページへの転送に替える」による）**：サイトを切り替えたので、前の rulec と dandori のブラウザで試すページ（英語と日本語の四つ）を、ritsu のページへ送るだけのページにした。`=` を含むハッシュ（dandori のページの `#flow=…`）はそのまま渡し、そうでなければ前のページが開いていたもの（rulec は `#project=rulec/gap`、dandori は下書き）を開く。言語のサイトのナビとトップページのボタンは ritsu のページを直接指す。前のページだけが使っていたもの（rulec と dandori の `src/wasm.rs`、dandori の `src/playground.rs`・`src/record.rs` と `sources` のバンドル、二つのページの JS・CSS・wasm・`presets.json`、二つの `make_wasm.sh`、前のページのテスト）を消し、ritsu.wasm を作り直した（出す関数は 23 から 9、9,813,955 バイト）。前のページのテストが確かめていたことは、もとのファイル（rulec のコーパスとトップページの表、dandori の例）と ritsu のページに対して確かめる形で、ritsu の `tests/playground.rs` に移した。前のページが並べた例の並びはテストの表（`RULEC` と `DANDORI_PAGE`）にし、rulec の `tests/wasm.rs` と dandori の `tests/playground.rs` がわざと選んでいた規則と編集を、ページの答えをバイナリと、ritsu.wasm をライブラリと突き合わせる編集に足した。rulec の E129 の注と台帳から「ブラウザで試すページ」を外した（rulec の §15.185）。dandori の DESIGN（0.3、5.3、6 章、7 章）とサイトの確かめ方のページ、rulec と dandori の README、サイトの README、`docs.yml` の木の確かめを直した（DESIGN 8.7、13.2）。

**確かめたこと**（転送に替えたあと）：`crates/ritsu/tests/playground.rs` の六つ（48 の例をもとのファイルとテストの表に突き合わせ、ページの 1,744 の答えをバイナリと、ritsu.wasm の 410 の問いをライブラリと突き合わせ、Chrome で 22 の画面を開き、転送のページからの 7 本のリンクを Chrome でたどる）と `tests/website.rs` の 10 件（転送のページのリンク・スクリプト・ナビと、組んだ木）。`cargo test -p ritsu -p rulec -p dandori`（dandori のプラットフォームのテストを除く）の全体で 959 件が通った（rulec 774、ritsu 96、dandori 89。落ちたもの 0、ignored 2、12 分 55 秒）。SKIP の 15 行は rulec の Lean の確かめ直しで、`proofs/` を組まずに回したためだった。`lake build` のあとに回し直した三つのファイル（`tests/lean.rs`・`days.rs`・`machine.rs`）の 42 件は、SKIP なしで通った。`website/build.sh` で組み（六つの組み立てのどれも警告 0）、組んだ木を `/ritsu/` の下で配って、英語と日本語の転送のページを headless の Chrome で開いた（7 本。dandori の `#flow=…&view=build` と `#flow=…&view=rules` とハッシュ無し、rulec のハッシュ無しと、ritsu のページの共有のリンク `#project=rulec/full&view=gen`）。どれも ritsu のページの同じ中身に着いた（画面写真は logs の `site-check-*.png`）。

### F.6 Lean の層

DESIGN 11 章。済んだ。

- 済んだこと（前半、Lean の層の一つ目の担当）：根の `proofs/`（`ChoboModel`、`KoyomiModel`、`DandoriCore` と `ritsu-model`）、Rust との突き合わせ（`crates/ritsu-model` の `chobo.rs`、`koyomi.rs`、`dandori.rs`）、三つのライブラリのどの宣言も三つの公理のほかに立たないことの確かめ（`proofs.rs`）、`cargo xtask deps` の表（`ritsu-model` をテストの層に。DESIGN 3.1）、CI の `proofs` のジョブ。突き合わせの下限（テストが落ちる数）は、実際に走らせた数から少し下に置いた。
- 済んだこと（dandori の検査の担当）：Lean のモデルが見つけた dandori の検査の見落とし（E020）を直した。dandori の `flow.rs` は、案件ごとに、レコードが言う状態（ワークフローが最後に聞いた状態）と、外部のサービスで案件がそのときかそれより後にいた状態の組を持つ（`Check.lean` と同じ形。dandori の DESIGN 2.1）。前は一つの集合しか持たず、失敗した呼び出しのあとの `match` が、レコードで選んだ分岐の中で案件がいる状態まで絞っていた。Lean の担当が置いた flow を `tests/fixtures/capture_timeout.flow` にし（E020 になる）、拒否されたら引き渡す形を `capture_timeout_handed_over.flow` にした（通る）。直すと、例のうち三つ（ホテルの予約の六つの版、請求の二つ、注文の Temporal 版の二つ）とテストのフロー `tests/flows/dates_and_books.flow` が同じ形の見落としを持っていたので、直した（★文言は担当が決めた。ホテルの予約は `fail CleanupFailed … leaving pi`、注文は `fail ShippedAlready … leaving order`、請求は `on already_posted => pass`。レコードが言うはずのない状態の分岐は E011 で外した）。`DandoriCore` に、dandori の日付と帳簿と `now` を足し（`Syntax.lean`、`Replay.lean`、`Check.lean`・`World.lean`・`Sound.lean`）、`crates/ritsu-model/tests/dandori.rs` の lower に足して、日付と帳簿を使う三つの flow を突き合わせに入れ、比べるものに呼び出しの引数を足した（137 か所）。替える前と後で、例とテストのすべての flow の出力を 945 回比べ、違った 113 回は直したフローと `book_refusals.flow` の `check` だけだった。
- 済んだこと（残り、Lean の層の二つ目の担当）：`crates/rulec/proofs/` を根の `proofs/` に移し、一つの Lake のパッケージにした（`RulecCert` と `rulec-recheck`。rulec の `tests/lean.rs`・`days.rs`・`machine.rs`、CI の `proofs.yml` と `tools.yml` を新しい場所に合わせた）。`RitsuCross`（X2、X3 の (a) と (b)、X4、X5、X6 の判定と、その正しさの定理。X3 は `KoyomiModel` と `RulecCert` を、X4 と X5 は `ChoboModel` をまたぐ）と、Rust との突き合わせ（`ritsu-model cross` と `crates/ritsu-model/tests/cross.rs`。8 件）。`#print axioms` の確かめを五つのライブラリの全部に広げた（`proofs.rs`。5,331 個、うち定理 2,064 個）。`DandoriCore` の定理に前提を二つ足し（案件を始める呼び出しの失敗、拒否のエラー。指示する側が決めた）、dandori の検査と同じ細かさにした（DESIGN 11.2 の 4）。確かめたこと：まっさらから `lake build` で 90 の仕事が通り（20 秒）、`cargo test -p ritsu-model` は 13 件（SKIP 0、1 分 29 秒）、rulec の `tests/lean.rs` は 12 件、`days.rs`・`machine.rs` は 30 件（PostgreSQL つき）、コーパスの 50 の規則のすべての証明書が根の `proofs/` の `rulec-recheck` と `tools/recheck.py` を通った。
- X4 を額 0 から数えるようにしたとき（作者の指示）、`RitsuCross` の `amountFits`・`amountsGiven`・`amountsHull` を 0 からにし、`amountFits_fails_chobo` を足した（DESIGN 11.2 の 3。宣言は 5,333 個、うち定理 2,066 個）。
- 残り：X6 を koyomi のモデルにつなぐ定理、X5 の秒数を dandori のモデルから数えること、rulec の並びの上限の答え、証明書の日の集合を証明にすること（7.10）。

### F.7 リリースの準備

- バージョンの番号を 0.23.0 にそろえる（DESIGN 13.1）。`version.workspace = true`、生成物と golden を一度に取り直す。手順と試した結果は DESIGN 13.1 に書いた。そろえるのは F の最後（ほかの担当が golden を触らなくなってから）。
- 作った：`.github/workflows/release.yml`（四つの対象、`ritsu` とリンク七つとライセンス二つのアーカイブ、`.deb` と `.rpm`、`SHA256SUMS`、Homebrew の formula の検査と tap への push）、`.github/workflows/packages.yml`（毎回の確かめ）、`packaging/`（`archive.sh`、`smoke.sh`、`nfpm.yaml`、`linux.sh`、`homebrew.sh`）、根の `action.yml`。リンクの名前で呼ばれたときの入口は E.2 で作ったので、`rulec mcp` が自分を呼ぶときの `arg0` と、リンクのテスト（`crates/ritsu/tests/links.rs`、7 件）を足した。リリースのテストは `crates/ritsu/tests/release.rs`（7 件と ignore 1）。手元で確かめたことは、F.8 の報告のとおり。
- 決めたこと：アーカイブは平らで、リンクは相対（`tar -xzf … -C ~/.local/bin` が入れ方のすべて）、ライセンスのコピーを入れる。`.deb` と `.rpm` は `rulec` を置き換える。`release.yml` は、タグと八つの名前の `--version` がそろっていなければ止まる。`packages.yml` は main への push と pull request（文書だけの変更を除く）で走る。tap へは `Formula/ritsu.rb` を足すのと `rulec.rb` を名前の変更に替えるのを同じコミットでする（DESIGN 13.2。★切り替えの時期は作者に聞く）。formula と `.deb`・`.rpm` の `homepage` は `https://github.com/i2y/ritsu`。
- 配る場所を ritsu に移す準備（DESIGN 13.2。F の最後）：rulec と dandori のサイトの中身（英語と日本語のページ、ブラウザで試すページ）を ritsu のサイトに移し、そのビルドと文書のテスト（いまの rulec の `tests/website.rs`、dandori の `tests/docs.rs` にあたるもの）を ritsu で回す。作者の指示（2026-10-04 19:30）で、rulec と dandori のサイトを `website/` の下に移した（切り替えはしていない）。rulec の Homebrew の formula を ritsu の formula に替える下書き（`packaging/homebrew.sh`）と、`i2y/tap/rulec` を入れている人の移り方は、DESIGN 13.2 に書いた。
  - 済んだ（site の担当）：ritsu のサイトの骨組み（根の `website/` の `zensical.toml`・`zensical.ja.toml`、`docs/index.md` と `docs-ja/index.md`、`build.sh`・`sync.sh`・`serve.sh`、`README.md`）、`.github/workflows/docs.yml`（`workflow_dispatch` だけ）、根のサイトのテスト `crates/ritsu/tests/website.rs`（9 件。yuen の名前空間のページの二つを足した）。dandori のサイトを `website/dandori/` に移した（`git mv`。設定はそのままで、`site_url` を `/ritsu/dandori/`、`repo_url` を ritsu にした）。dandori のテストと道具が読む場所と、CI の三つの段がコピーを作る `sync.sh` の場所を直した。取り込むときに、ritsu のブラウザで試すページが言語の `doc` のページを呼ぶ言い方を、根の index にそろえて「the page for people」「人が読むページ」に直した。
  - 済んだ（rulec のサイトの担当）：rulec のサイトを `website/rulec/` に移した（`git mv`。設定はそのままで、`site_url` を `/ritsu/rulec/`、`repo_url` を ritsu にした。`serve.sh` は消した）。根の `build.sh` は `sites=(rulec dandori)`、根の index（英日）の表は rulec のサイトを指す。rulec のテストと道具（`tests/website.rs`・`docs.rs`・`skill.rs`・`m0.rs`・`verify.rs`・`vdiff.rs`・`doc.rs`・`wasm.rs`、`skills/sync.sh`、`website/rulec/sync.sh`・`tools/make_examples.py`・`make_stack.py`・`make_assurance.py`・`shots.sh`・`social.sh`・`make_wasm.sh`）が読む場所を直した。ritsu の `tests/website.rs` は、`build.sh` が rulec と dandori のサイトを組むことと、組んだ木に `rulec/index.html`・`rulec/ja/index.html` と rulec のブラウザで試すページがあることを確かめる。CI の三つの段の `sync.sh` の場所と、`docs.yml` の木の確かめ（rulec の七つ）を直した。`ritsu.ctx` の `except` から `crates/rulec/website` を消し、★`crates/rulec/.github/workflows/docs.yml` を消した。ページの中の `github.com/i2y/rulec/…` を ritsu の `crates/rulec/…` にし、★入れ方のページ（英日）を rulec の README の入れ方に合わせた。英語のページの日本語の残り三つ（画面写真、突き合わせの実演、`flow.svg` の質問）を `parcel_rate` と英語に替えた（DESIGN 10.10。rulec の §15.181）。前のブラウザで試すページは二つとも残した（★残すか替えるかは作者に聞く。DESIGN 15 章）。ritsu のページに替えると決まり（作者の決め）、ritsu のページが前の二つのページの例とリンクの形を全部持つようにした（F.5）。前のページは、サイトを切り替えたあと（2026-10-05）、ritsu のページへの転送のページに替え、前のページだけが使っていたものを消した（F.5、DESIGN 8.7・13.2）。
  - 済んだ（yuen の名前空間の担当。2026-10-04）：yuen の PROV の名前空間の IRI を、リポジトリの URL `https://github.com/i2y/yuen/ns#` から ritsu のサイトの URL `https://i2y.github.io/ritsu/ns/yuen#` に替えた（作者の決め。yuen はまだ配っていなかったので、替えて困る文書は無い）。書き出し（PROV-N と PROV-JSON）、18 プロジェクトの golden 36 枚、yuen の DESIGN 13 章、`docs/reference.md` とスキルのコピー、`ritsu.wasm` を直した。IRI を開くと語の説明が読めるページ `website/docs/ns/yuen.md`（日本語は `website/docs-ja/ns/yuen.md`。公開先は `/ritsu/ns/yuen/` と `/ritsu/ja/ns/yuen/`。nav には出さない）を足した。語は 28（型 7、関係の種類 4、属性 17）で、一覧は yuen の `src/export/prov.rs` の `TERMS`（名前、種類、書かれる先）。yuen の `tests/export.rs`（4 件から 5 件になった）が一覧と全プロジェクトの書き出しの語を突き合わせ、ritsu の `tests/website.rs`（7 件から 9 件になった）が一覧とページ（英語と日本語）を突き合わせる（DESIGN 13.2）。
  - 取り込むときにしたこと：移す前の `crates/rulec/website` と `crates/dandori/website` の gitignore のコピーを消した。英語の `doc` の約物を直した（DESIGN 10.10）ので、dandori の図の英語の golden、サイトの図のページ、ブラウザで試すページの presets を取り直し、wasm を三つ（rulec・dandori・ritsu。rulec の wasm は `website/rulec/docs/playground/`）作り直した。
  - まだ：サイトの切り替え（7.10）。
- 言語の七つのクレートの `Cargo.toml` の `repository` を、`repository.workspace = true` にそろえた（作者の指示。rulec の `homepage` は、公開している `https://i2y.github.io/rulec/` を指すので、サイトを切り替えるまで残した。DESIGN 13.2）。
- リリース、タグ、push、公開、サイトと formula の切り替えは、作者の指示があるまでしない。
- 済んだ（2026-10-05 の夜、作者の指示で）：v0.23.0 のリリースと tap の切り替えは指示する側がした（tap に書き込む deploy key を作り、`i2y/ritsu` の `HOMEBREW_TAP_KEY` に置いた。`packages.yml` は push のたびに通っていた）。そのあと、リリースのあとの担当が、rulec の入れ方の案内を ritsu のリリースに替え、rulec だけのリリースのファイル（`crates/rulec/.github/workflows/release.yml`、`ci.yml`、`packaging/`、`action.yml`）を消し、`[package.metadata.binstall]` を外した。ritsu の `tests/website.rs` の `/releases` の例外を外した。7.10 と rulec の DESIGN §15.186。
- バージョンを 0.23.0 にそろえた（指示する側。F の最後）。手順は DESIGN 13.1 のとおりで、rulec の `website/tools/make_wasm.sh` を、ワークスペースの `target/` と `CARGO_TARGET_DIR` で動くように直し（前は `crates/rulec/target` を見に行っていた）、ブラウザで試すページの wasm を三つ（`rulec.wasm`、`dandori.wasm`、`ritsu.wasm`）作り直し、全部の `<名前>_BLESS` と `RITSU_BLESS` で golden を取り直した。

### F.8 F の完了の条件

1. 根から `cargo test --workspace --no-fail-fast -- --nocapture` が全部通り、この機械で SKIP は 0（B.8 で許したものを除く）。`lake build` と Lean の突き合わせも通る。
2. yuen と sakai の、書き直した段階 D の完了の条件を満たす。済んだ（F.1、F.2）。
3. 根の README.md と README.ja.md の出力と診断が、テストの確かめを通る。済んだ（`crates/ritsu/tests/readme.rs`。F.3）。
4. ブラウザのページのテストが通る（済んだ。F.5）。LSP は今回は作らない（F.4）。
5. DESIGN.md に、スケッチと見込みが残っていない（DESIGN の 7.2 の表と 8.3 の JSON の形を含む）。E の終わりに、7.2 の表、7.5 と 7.8 の案、8.1 のコマンド、8.3 の例、3.2 と 5.1 のコード、6.4 の表、9.3 の形を差し替えた。
6. 報告に、リリースの準備で手元で確かめたことを書く。リリースの準備の担当が手元で確かめたこと：アーカイブ（ritsu、リンク七つ、ライセンス二つ）を作って展開し、八つの名前で呼べること（`smoke.sh` が七つの言語の例と `ritsu check` を走らせる）。aarch64 と x86_64 の Linux 用の静的な `ritsu` から `.deb` と `.rpm` を作り、Debian と Fedora に入れて、七つの名前が動き、消すと八つのファイルが残らないこと（aarch64 は、`ritsu check` を含む八つの確かめまで）。rulec 0.22.1 のパッケージが入っているところに入れると、rulec が取り除かれること。Homebrew（macOS）で formula の audit・install・test が通り、`formula_renames.json` による `rulec` から `ritsu` への移行が通ること。`rulec mcp` の呼び出しが、リンクを通っても `rulec` のものと同じ出力になること。Intel の macOS のビルド、CI でのワークフローの実行（リモートが無い。musl のビルドは CI では musl-tools で、手元の zig とは違う）、`brew audit --online` は、手元では確かめていない。

## 7. 次の段階への申し送り

各段階の終わりに、ここに書き足す。

### 7.1 A から B へ（A の段階で書いた）

- yuen の二つのテストの扱い（B.7）を、始める前に指示する側に確かめる。
- dandori のテストには rulec 0.22.0 が要る（B.7）。
- 取り込みのマージはコミットを作る。`~/ritsu` に対して走らせるのは、指示する側が許したときだけ（B.2）。
- rulec のテストは `crates/rulec/` で走らせる（`.cargo/config.toml`）。
- 作業場所の試し（DESIGN 12.5）では、geas と sakai は B の形でも全部通り、yuen は二つ落ちた。rulec、dandori、koyomi、chobo は試していない（`.git` からルートを決めないことは、ソースを読んで確かめた）。

### 7.2 B から C へ（B の報告から、C の最初に書いた）

- B の結果：九つのコミット（最初のコミット、七つの取り込みのマージ、yuen の二つのテストの直し。B.2）、コミット 427、タグ 29。各クレートの `cargo test -- --list` は元と同じで、テストは元と同じ件数で通った（rulec 727・ignored 1、dandori 81・Jev の SKIP 1、koyomi 99、chobo 63、geas 236、yuen 94、sakai 97）。
- B で残したもの：yuen の回り道の表示、sakai の `.git` が無いと黙って飛ばす部分、根からの `cargo test --workspace`、クレートの中の `Cargo.lock` と `[profile.*]`（四つとも C.0）。dandori が使う rulec 0.22.0（D.3）。CI だけが回す段（`rulec test --require-all --proofs`、Kani、`tools/recheck.py`、`cargo fmt --check`、dandori のサイトのビルド。C.12）。本物のサービスへの問い合わせ（e-Gov、eCFR、TypeSafe、Ollama）。
- 警告：cargo の「profiles for the non root package will be ignored」が五つ（C.0 で消える）。rulec のテスト関数の名前が snake case でないという警告が七つ（元のリポジトリと同じ）。
- 入れ直したツール（B.5）は、gitignore した場所に約 2.4 GB ある。yuen の venv は `crates/yuen/tools/.venv` にあり、改名で一緒に移った。
- この機械でのテストの回し方（ツールの場所は指示書）：
  - rulec：`website/sync.sh` でコピーするページを作り、`proofs/` で `lake build` を済ませ、Connect・mypy・NumPy・ruff・uvicorn の入った venv（dandori の `tools/connect/requirements.txt` から作る）を PATH の先頭に、OpenJDK と PostgreSQL の bin を PATH に置き、使い捨ての PostgreSQL を `PGHOST`・`PGPORT`・`PGDATABASE` で渡す。
  - dandori：`DANDORI_RULEC` に rulec 0.22.0、PATH に argo CLI v4.1.4、`TYPESAFE_API_KEY` は空。重いもの（Temporal、Argo、LocalStack、durable、pydantic-graph）は `cargo test --test examples <名前> -- --exact` で一つずつ、残りは `--skip localstack --skip temporal --skip pydantic_graph --skip durable --skip argo --skip ollama` で回す。
  - koyomi：`KOYOMI_PG_BIN` と、短いパスの `KOYOMI_PG_SOCKET_DIR`。chobo：`CHOBO_PG_BIN`、`CHOBO_PG_SOCKET_DIR`、`TMPDIR`。geas：`GEAS_PIXIE_GREETER`。yuen：環境変数は要らない（venv とスキーマを既定の場所に入れた）。sakai：`SAKAI_JAVA`・`SAKAI_JAVAC` と、`SAKAI_RULEC`・`SAKAI_KOYOMI`・`SAKAI_CHOBO`・`SAKAI_DANDORI` にワークスペースの `target/debug` のバイナリ（先に `cargo build --workspace`）。Go のビルドキャッシュ（`GOCACHE`）は作業場所に置く。
- テストが OS の一時ディレクトリに残すもの（`dandori-steps-*`、`dandori-protos-*`、`rulec-*-<pid>`、swiftc の `TemporaryDirectory.*`）は、C.2 の一時ディレクトリに替えるときに片づける。

### 7.3 C の最初の部分から、C の残りへ（C の最初の部分の終わりに書いた）

- 済んだもの：yuen への改名、C.0、C.1（`ritsu-base`）、C.2（`ritsu-testkit`）、C.3（`xtask`）。七つの言語は、まだどれも土台を使っていない。残りは C.4〜C.13。
- 土台へ移すときの手がかり：
  - koyomi・yuen・sakai の `explain`（テキストと Markdown）、koyomi の `--help`、yuen の `review --help` は、土台の `ledger` と `cli` で一字も違わずに組み直せることを確かめてある（`crates/ritsu-base/tests/golden/compat/`）。sakai の `--help` は確かめていないが、表の形は koyomi と同じ（違いは `Reading` の一つ）。chobo の台帳と CLI、geas の表は確かめていない（chobo の `explain` の Markdown と、geas の表の形は別のものである）。
  - `diag` は、行の無い診断の JSON に `null` を書く。geas の golden（`tests/golden/en/errors/E081-missing.json` の `"line": 0`）は C.6 で取り直し、理由を geas の DESIGN.md に書く。chobo の JSON のキー（`v`、`column`、`title`、`excerpt`、`operations`、`hint`）は C.5 で替える（DESIGN 4.2）。
  - 文の出し方は、koyomi と sakai が `text::spaced`、yuen と chobo が書いたとおり（`as_written`）。
  - `cli::lang_flag` の説明は `RITSU_LANG` に触れていない。言語を `Lang::pick` に替えるときに説明も直し、`--help` の golden を取り直す。
  - `sources` へ替えると、rulec は漢数字を百と千まで読むようになり、`第0条` を通さなくなる。koyomi の `text_diff` は全体で件数を切り、土台の `text_diff` は片側ずつ切って「あと n 行」を足す（koyomi は `diff_lines` から自分の形を作れる）。koyomi と yuen の curl には `--compressed` が付く。
  - yuen の書き出しの識別子は、実装では SHA-256 を二度かけている（`src/export/mod.rs` の `sha256::hex(&sha256::digest(&b))`。土台の `hex` も同じく、渡されたバイト列のダイジェストを書く）。yuen の DESIGN.md 12 章は一度と書いている。C.7 で yuen を土台の `sha256` に替えるときに、値を保つなら二度かけたままにし、直すなら識別子が全部変わる。どちらにするかは作者に聞く。
  - `naming.tsv` の yuen と sakai のコピーは C.7 と C.8 で消す（PLAN 0.3）。`ritsu-base` のテストは、コピーが無くなれば比べないで通る作りにしてある。
  - yuen の `Project::shown` は C.0 で直した。C.7 で土台の `paths::Shown` に替える（同じ考え。ただし yuen はシンボリックリンクをたどったパスを持っている）。
- この機械で根から回すとき：`cargo test --workspace --no-fail-fast -- --nocapture --skip localstack --skip temporal --skip pydantic_graph --skip durable --skip argo --skip ollama`。dandori の重い段は、7.2 のとおり一つずつ回す。環境は 7.2 のものに、`RITSU_PG_BIN`・`RITSU_PG_SOCKET_DIR`・`RITSU_TIGERBEETLE`（`ritsu-testkit` のテスト）を足す。PostgreSQL のソケットのディレクトリは短いパス（`/tmp` の下）にする（作業場所のパスでは 103 バイトを超える）。dandori の `DANDORI_RULEC` には rulec 0.22.0（GitHub のリリースのバイナリを、チェックサムを確かめて作業場所に置く）。geas の `GEAS_PIXIE_GREETER` は、pixie を作者がビルドした場所にある greeter を読む（pixie の木ではビルドしない）。
- `--skip argo` は `cargo` を含む名前にも当たる。根から回すテストに、その語を含む名前を付けない。
- rulec のテストを足すときは、rulec を走らせるところで `RULEC_LANG=ja`（か `--lang`）を渡し、プロセスの中で日本語の文を読むなら、はじめに `rulec::i18n::set(Lang::Ja)` を呼ぶ。`.cargo/config.toml` はもう無い。
- C.12 で `ci/skips/<段>.txt` を書く。段で外したテストの SKIP は理由の種類が `level` で、一覧に書かなくても `cargo xtask test` は通す。

### 7.4 C の二つ目の部分から、C の残りへ（C の二つ目の部分の終わりに書いた）

- 済んだもの：C.4〜C.10。koyomi・chobo・geas・yuen・sakai は土台（`ritsu-base`、`ritsu-testkit`）を使い、sakai は `ritsu-proto` で、koyomi と chobo は `ritsu-emit` で書く。残りは C.11〜C.13。
- C.11 の手がかり：
  - rulec と dandori の予約語の表は `ritsu-emit` の `copies` にコピーしてあり、`crates/ritsu-emit/tests/copies.rs` が二つのいまの表と等しいことを確かめる。替えるときは `copies` から読むようにし、コピーのテストを消す。rulec は語を大文字と小文字を区別せずに照らすので、`words::rust::KEYWORDS`（`Self` を含む）をそのまま使っても結果は変わらない。rulec の `GO_GLOBAL` には `complex64` と `complex128` が無く、dandori の Python の組み込みの名前は rulec のものと一部が違う。そろえるなら生成物と診断が変わるので、それぞれの DESIGN.md に理由を書く。
  - `ritsu-emit` の `ident` と `lit` は koyomi と chobo の形である。rulec の `go_package` は小文字にもする（koyomi の別名はもともと小文字）。dandori の Go の外に見せる名前（`exported`）は、区切りで分けて頭を大文字にする形で、chobo の `go_exported` とは違う。
  - rulec の `src/sha256.rs` と `src/json.rs` は、まだ rulec の中にある（`grep -rn 0x428a2f98 crates/*/src` は ritsu-base と rulec に当たる）。
- D.10 の手がかり：
  - `ritsu-proto` のテストは、rulec と dandori を dev-dependency にして古い読み手と比べている。二つが `ritsu-proto` で読むようになれば依存が輪になるので、そのとき比べるところを消し、`tests/golden/rulec.txt` と `dandori.txt` と比べる形だけを残す。
  - 新しい読み手が rulec の読み手と違うのは、一行に書いた `package` を読むことと、壊れたファイルを途中まで読まずに誤りを言うことの二つ（C.9）。後者を rulec がどう扱うか（いまは読めたところまでで契約を突き合わせる）は D.10 で決める。
  - dandori が `ritsu-proto` で読むときは、proto2 と edition を受け付けないこと、読めなかった import があるときに型の名前を書いたまま残すこと、型の解決を見えるファイルだけにすることを、dandori の側で書く（`tests/readers.rs` の `as_dandori` がその形で、三つのリポジトリの全部の `.proto` で古い読み手と同じ結果になる）。
- この機械でテストを回すとき（7.3 のものに足す）：sakai のテストは、先に `cargo build --workspace` をして、`SAKAI_RULEC`・`SAKAI_KOYOMI`・`SAKAI_CHOBO`・`SAKAI_DANDORI` にワークスペースの `target/debug` のバイナリを渡す。`ritsu-proto` と `ritsu-emit` のテストは rulec（と dandori）をビルドするので、初めは時間がかかる。
- yuen の名前の漢字は、まだどの文書にも書いていない（作者に聞いているところ）。各クレートの `repository` と yuen の PROV の名前空間の URL は F で決める。（どちらも F.7 で決めた。`repository` は `repository.workspace = true`、PROV の名前空間は `https://i2y.github.io/ritsu/ns/yuen#`）

### 7.5 C から D へ（C の最後の部分の終わりに書いた）

- 済んだもの：段階 C の全部（C.0〜C.13）。七つの言語は `ritsu-base` と `ritsu-testkit` を使う（sakai は `ritsu-proto` も、koyomi・chobo・rulec・dandori は `ritsu-emit` も）。根に CI のワークフローがある（まだ一度も走っていない）。
- D の手がかり：
  - D.3：dandori のテストは、まだ rulec 0.22.0 のバイナリ（`DANDORI_RULEC`）で規則を読む。`Need::Rulec` で段を見るのはこのテストである。dandori の `src/rulec.rs` が返す文の組は、英語が先の `(en, ja)` のまま（C.11 で触らなかった）。つなぎを型の付いた呼び出しにするときに `Text` にする。CI の `tools` と `platforms` も、rulec 0.22.0 をリリースから取ってきている。D.3 が済めば、その段と `Need::Rulec` を消す。
  - D.5：rulec は `RITSU_LANG` をまだ読まない（`RULEC_LANG` と `--lang` だけ。`src/i18n.rs` のまま）。ほかの六つは `<名前>_LANG`、`RITSU_LANG` の順に読む。
  - D.8：sakai の `what_was_copied_passes_the_suite` は、rulec・koyomi・chobo・dandori のバイナリを `SAKAI_RULEC` などで受け取る（`Need::Suite`）。同じプロセスで呼ぶようになれば、その段と環境変数が要らなくなる。
  - D.10：`ritsu-proto` のテストは、まだ rulec と dandori を dev-dependency にして古い読み手と比べている（7.4）。
  - 予約語の表（E で決める）：rulec と dandori は `copies` の自分の表を読む。C.11 で、表の違い（rulec の Go の表に `complex64` と `complex128` が無い、Python の組み込みの名前の一覧が二つで違う）で、dandori の生成物が rulec の生成物の名前を参照するところに食い違いが出るかを調べた。表の違いからは出なかった。dandori は rulec の名前を `rulec api` から読んで書き、二つの表はそれぞれ自分の書く名前を守っているからである（Go で `complex64` を名前にしても、パッケージの中で事前宣言の型を隠すだけで、ビルドは通る）。そのかわり、dandori が自分で書く名前と、規則の別名（rulec の生成物の関数の名前）がぶつかるところが見つかった。どちらの表も守っていない。
    - Temporal の Python（`rules.py`）：規則を `from .rulec.python.<別名> import <別名>` で読み込み、同じモジュールに `from temporalio import activity`、各規則のアクティビティ `rule_<フローでの名前>`、終わりに `rules = [...]` を書く。別名が `activity` なら、`@activity.defn` が規則の関数を指して、読み込んだときに AttributeError で止まる。別名が `rules` なら、終わりの行が名前をリストで上書きし、アクティビティが走ったときに規則の代わりにリストを呼ぶ。別名がそのアクティビティの名前と同じ（`use rule x` で別名が `rule_x`）なら、アクティビティが自分自身を呼ぶ（rulec 0.22.0 で生成して確かめた）。
    - Temporal の TypeScript（`rules.ts`）：別名が `rules` なら、読み込んだ `rules` と `export const rules` が同じ名前になり、コンパイルが通らない。
    - Step Functions の Lambda（`lambda/<モジュール>_handler.py`）：`def handler(event, context)` が規則の関数を呼ぶ。別名が `handler` なら、`def handler` が読み込んだ関数を上書きして自分自身を呼び、`event` か `context` なら、引数が関数を隠す（生成するコードを読んで調べた）。
    - Temporal の Go：ぶつかるところは無かった（dandori の名前に `dd` が付き、規則はパッケージの名前で引く）。
    一つの表にするときは、dandori が自分で書くこれらの名前も入れるか、dandori の側で別名を避ける（`_` を足す）必要がある。いまはどの例もこれらの別名を使っていない。
  - rulec と dandori の `website/tools/make_wasm.sh` は、B からクレートの中の `target/` を見るので動かない（根の `target/` になった）。コミットしてあるブラウザで試すページのモジュール（`rulec.wasm`、`dandori.wasm`）は C で作り直していない。テストは、いまの CLI の出力とページの出力が同じことを確かめていて、どちらも通る。F.5 で `ritsu-wasm` にするときに直す。
  - テストが OS の一時ディレクトリに残すもの（7.2）：dandori のテストは `ritsu-testkit` の一時ディレクトリにした（`dandori-protos-*` はもうできない）。残るのは、dandori の runner（`tools/temporal/run.mjs`、`tools/temporal-python/run.py`、`tools/argo/run.mjs`）が自分で作る `dandori-temporal-*`・`dandori-temporal-python-*`・`dandori-steps-*`・`dandori-argo-*` と、rulec のテストが作る `rulec-<用途>-<pid>`（rulec のテストの一時ディレクトリは C.11 で替えていない）、rulec の Swift のテストで swiftc が作る `TemporaryDirectory.*` である。C の最後の部分で回したテストの全部で 467 個（約 118 MB）が残り、それは消した。runner が終わるときに自分の作業場所を消すか、テストが runner に作業場所を渡せば止まる。それより前からある分（`dandori-steps-*` だけで二千あまり）は消していない。
  - CI：五つのワークフローは手元でしか確かめていない。初めて走らせたときに、runner との違い（Ubuntu 24.04 の Chrome のサンドボックス、Node 24 と手元の 23.11、PostgreSQL 18 を PGDG から入れること、kind を `go install` で作ること）が出るかもしれない。geas の pixie のテストは CI では回さず、greeter のある手元の機械で回すと決めた（`ci/skips/tools.txt`）。TypeSafe には CI から送らない（呼ぶたびにお金がかかるので、鍵を CI に置かない。DESIGN 10.5）。サイトのビルド（rulec と dandori の `docs.yml`）は、F で配布を ritsu に移すまで ritsu の CI では走らない（DESIGN 10.5）。
- この機械でテストを回すとき（7.3 と 7.4 のものに足す）：根から `cargo xtask test`（段を付けなければ見つかったもので走れるものを全部、付ければその段まで）が SKIP の表を最後に出す。dandori の重いテストは `cargo xtask test --level platforms -p dandori -- --exact <名前>` で一つずつ回す（`platforms.yml` と同じ）。rulec の PostgreSQL は、作業場所の使い捨てのクラスタ（ソケットは `/tmp` の下）を `PGHOST`・`PGPORT`・`PGDATABASE`・`PGUSER` で渡す。
- **C のあとに入れた片づけ**（2026-10-03、D の前）：テストと `rulec test` が後に残すものを三つ直した。
  - `rulec test` は、確かめが途中で落ちると、HTTP で立てた生成コードの MCP サーバーを止めずに戻っていた（`std::process::Child` は、捨てても子を止めない）。rulec に子プロセスを包む `child::Owned`（捨てられるときに、終わっていなければ止めて待つ）を足し、rulec が立てて途中でやりとりするプロセス四つ（`runtest` の MCP サーバー、`verify` のアダプタ、`extract` の抽出器、`mcp` が立てる rulec）をこれで包んだ。決めたことは rulec の DESIGN §15.163 に書いた。テストは `tests/child.rs` の二本と `tests/fold.rs` の一本で、どちらも直しを外すと落ちる。`src/` のモジュールが一つ増えたので、README のモジュールの数を 50 にした。
  - rulec のテストの一時ディレクトリ（`rulec-<用途>-<pid>`、43 のファイルの 110 か所）を `ritsu-testkit` の `TempDir` にした。上の「rulec のテストの一時ディレクトリは C.11 で替えていない」は、これで済んだ。`tests/tool.rs` が HTTP で立てるサーバーも包みで立てる。
  - dandori の runner の作業ディレクトリ：Node の runner は `tools/work.mjs` の `workDir`（プロセスが終わるときに消す）で作り、Python の runner は `atexit` でも消す。別の言語の runner を立てる runner（Python の `run.py`、Go の `activitiesBy` と `children`）は、その runner に自分のディレクトリを `TMPDIR` として渡し、止めたときも含めて、終わったら消す。Python の runner が回すたびに残していた `dandori-steps-*`（Python の突き合わせを一度回すと 29 個）は、もうできない。Go のビルドが `go build` に落ちたときに作業ディレクトリを残して場所を言うのは、前のままにした。dandori の `src/proto.rs` の単体テストの一時ディレクトリも `TempDir` にした。
  - 回したもの：rulec のテストを全部（730 件、ignored 1、SKIP 0。足した三本を含む）、dandori の重いテスト 12 件を一つずつと残りの 69 件（SKIP は Jev の 1 件）。回した前と後で、OS の一時ディレクトリの `rulec-*` と `dandori-*` は 0 個と 0 個、親のいない MCP のサーバーは 0 個と 0 個だった。残るのは、swiftc が作る空の `TemporaryDirectory.*`（rulec のテストを一度回すと 51 個）と、テストのヘッドレスの Chrome が作る `com.google.Chrome.*`（dandori のテストを一度回すと 17 個。中は `SingletonCookie` と `SingletonSocket` だけ）である。
  - 続けて、残っていた二つもテストの側で止めた（2026-10-04）。
    - swiftc は `--version` に答えると、ほとんど毎回、空の `TemporaryDirectory.*` を TMPDIR に残す。`rulec test` は全部のツールチェーンにバージョンを聞くので、回すたびに一つ残っていた。rulec のテストは、`rulec test` を走らせるときと、swiftc を自分で呼ぶとき（`--version` で確かめるところも）に、子の TMPDIR をそのテストの一時ディレクトリの下に向ける。下に作るのは `ritsu-testkit` の `tmp::tmpdir_in` である。`rulec test` が swiftc に渡す環境は変えていない。
    - Chrome は、止められるとシングルトンのソケットのディレクトリ（`com.google.Chrome.*`）を一時ディレクトリに残す。macOS の Chrome はその場所を `TMPDIR` ではなく `MAC_CHROMIUM_TMPDIR` から読む。`ritsu-testkit` の Chrome には両方を渡し、プロファイルの下に向けた。geas のテストは、geas が立てる Chrome に同じ二つを渡す（一回で 41 個残していた）。
    - 回した前と後で、`TemporaryDirectory.*` は 0 個と 0 個、`com.google.Chrome.*` は 2,391 個と 2,391 個だった（rulec の全部、dandori の doc と playground、koyomi・chobo の doc、geas の全部、`ritsu-testkit`）。

### 7.6 D の最初の部分から、D の残りへ（D の最初の部分の終わりに書いた）

- 済んだもの：D.1、D.2、D.5、D.10。七つの言語のコマンドの出力は、`.proto` として読めないファイルのとき（rulec は途中まで読まずに E013 で止める（★）。dandori の E016 の注と sakai の E106 の日本語の文は言い方が変わった。D.10）のほかは変わらない。土台の層は `ritsu-units` と `ritsu-ports` が加わって五つになり（テストの共通部分の `ritsu-testkit` は別）、`ritsu-proto` と `ritsu-emit` の dev-dependency に言語のクレートは無くなった。
- D.3 の手がかり：
  - 口は `rulec::ports::Engine`（`ritsu_ports::Rules`）。読んだ規則をパスと中身の SHA-256 で覚えておくので、同じ規則を何度尋ねても検査は一度で済む。
  - D.3 の 1 の突き合わせのうち、口の事実と rulec の JSON が同じことは、rulec の `tests/ports.rs` がコーパスの 50 本で確かめている。dandori の `RuleInfo` と比べるときに気をつけることが三つある。
    - ステートマシンの状態の軸（`Machine::state_axis`）は、口では `Option` で、遷移を決める表に状態の列が軸として無いときは None になる（証明書の `machine` の `axis` が null）。dandori はいま null を 0 と読み、最初の軸を状態の軸として扱う（`axes_with_value` が最初の軸を除く）。口に替えるときに、None の扱いを決める。
    - 率の刻みは、口では単位の型（`Unit::step`）にある。dandori はいま JSON Schema の説明の文から読んでいる（DESIGN 1.4）。
    - 前提と `api` は、dandori はいま JSON（`Value`）のまま持っている。口では型（`Precondition`、`Call`、`Connect`）にし、dandori がいま読んでいる項目だけを入れた。dandori が `api` の別の項目を読むようになるなら、口に足す。
  - 規則のページ（`Rules::doc`）は、言語と、ページの頭に書くファイルの名前を渡す。dandori が埋め込むページは、ファイルの名前だけを書いている。
  - 答えられないときの `Said` の文は `Text`（二つの言語）で返る。dandori の `src/rulec.rs` の文の組は `(en, ja)` のまま。
- D.4 の手がかり：dandori の型の綴りは rulec と同じなので、`ritsu_units::Unit::parse` がそのまま読む。`money[JPY, incl_tax]` と `money[円, incl_tax]` は `Unit::same` で同じになり、範囲の端に付けた単位は `Unit::whole`（整数にならない換算は None）で数える。
- D.5 に残したこと：rulec は `RITSU_LANG` をまだ読まない（`--lang` と `RULEC_LANG` だけ）。DESIGN 4.1 の順（`--lang`、`<名前>_LANG`、`RITSU_LANG`、英語）にすると、`RITSU_LANG` を置いた環境で rulec の言語が変わる。CLI の振る舞いを変えることになるので、作者が決めてからにする。dandori と yuen は `with` を直に呼ばなくてよい。口の `Rules::doc` に言語を渡せば、rulec がそのスレッドの言語で描く。
- D.6：dandori の `Items` と `References` はまだ無い。ほかの六つの言語の `src/ports.rs` が形の手本になる。定義の文は DESIGN 6.4 の表のとおり。
- D.7：yuen の端になるのは、rulec（`rulec fmt` が書く形の行）、koyomi（`date … =` の塊と条件の行）、chobo（yuen の DESIGN 3.2 の形の JSON。yuen の試作が計算したハッシュと同じになることを chobo のテストで確かめた）、geas（主張の塊の行）の `Items` の `text` である。yuen 自身の `Items` は、要件の端が作れないとき（上の端が読めない、`from` が輪になる）は空の文を返す。端のハッシュを取り直すときは、空の文のハッシュを端として使わないこと。
- D.8：sakai の `Items` と `References` はできた（`means` の名指しは `sakai api` と同じ）。D.8 で sakai が尋ねる口（rulec の列挙と `shape` の契約、koyomi のカレンダーへの参照、chobo の勘定と振替）には、それぞれの `Engine` がもう答える。
- D.9：chobo の名前だけの単位の置き場所は `Dim::Count(名前)`。`Unit` に `scale` は無いので、DESIGN 5.4 のとおり `scale 2` の通貨は `<コード>c` の綴りにする。chobo の `Books` の事実の単位（`BookUnit`）は、いまは chobo の名前と scale のまま。
- まだ答えない問い（`Undecided` と理由を返す）：`Rules::preconditions_hold` の並びの合計と長さの上限（問いが並びの長さの範囲を持たない。E の X2 で問いの形を決める）、`Rules::checked_over`（E の X3 (b)）、`Books::refusals`（E の X4）、`Dates::values` と `days` の、確かめる数を超える入力と、計算が途中で止まる入力。
- geas は、口に答えるためにライブラリとコマンドに分けた。`src/lib.rs` は全部のモジュールを公開している。口に要るものだけに絞るかは、F で ritsu の CLI にまとめるときに決める。
- D.1 で見つけたこと：rulec は、税の語の場所に `incl_tax` と `excl_tax` のほかを書いた型（`money[円, foo]`）を黙って通す。その型には単位が無い（`Ty::unit` が None）。直すなら rulec の検査に診断を足すことになり、その規則の出力が変わる。作者の判断を待つ。
- テストの回し方：7.5 と同じ。根から `cargo xtask test -- --skip localstack --skip temporal --skip pydantic_graph --skip durable --skip argo --skip ollama` を回し、dandori の重いテストは `cargo xtask test --level platforms -p dandori -- --exact <名前>` で一つずつ回す。

### 7.7 D の二つ目の部分から、D の残りへ（D の二つ目の部分の終わりに書いた）

- 済んだもの：D.3、D.4、D.6、D.5 の残り（dandori が `Rules::doc` に言語を渡す）と、作者が D で直すと決めた三つ（rulec が `RITSU_LANG` を読む（D.5 に書いた）、rulec が税の語の誤りを E103 でエラーにする（D.1 に書いた）、dandori の名前のぶつかりを E006 でエラーにする（D.3 に書いた））。入口の最小の形として `crates/ritsu` に `ritsu dandori` を作った（DESIGN 8.6）。dandori のクレートは rulec に `[dependencies]` で依存せず、規則は渡された口で読む。
- D.7 の手がかり：
  - dandori の中のものは `dandori::ports::Engine` の `Items` が渡す（種類は `task`・`case`・`record`／`field`・`enum`／`value`・`input`・`output`）。構文まで読むだけなので、規則を読めないフローでも答える。定義の文は dandori の DESIGN 0.3 の形（コメントと空白の幅と字下げの幅に左右されない）で、yuen の端のハッシュはこれから取る。
  - yuen は、dandori の名指しを、ファイルで書いても種類つきで書いても「まだ読めない」と言う（`Unread::NotYet`）。D.7 で、ファイルなら `dandori check` の結果、種類つきなら `Items` の定義の文を端にする。
  - yuen の変異 `E012_dandoriの種類` は、dandori に無い種類（`table`）を試すものになった。
- D.8 の手がかり：
  - dandori の参照は `References` が渡す。参照の仕方は `use rule`（下に書いた呼び方を `use rule … lambda, connect, local` の形で添える）、`use proto`・`use openapi`・`use smithy`、`implements`、`connect`、`flow` である。sakai の DESIGN の「サービスを呼ぶ参照」（`connect` のタスクと `use rule … connect`）は、この `how` で見分けられる。型の中の参照（`<API>.<名前>`、`<規則>.<列挙>`、`follows`）と `http` の操作は渡していない（PLAN D.6 の★）。sakai が要素まで要るなら、dandori の口に足す。
  - sakai は、dandori の種類の語（`task`、`case`、`record`）を予約語に加えた。N101（dandori の参照を確かめていない）を外すのは D.8 である。
- D.11 の手がかり：D.3 の突き合わせは、JSON の読み手を消す前に全部の規則（rulec のコーパスの 50 本と dandori の 14 本、1,939 の項目）で通した（違った 7 か所はどれも JSON の側の読み違え）。DESIGN 1.4 の二つの例は直った（`hold.flow` が通る、dandori は率の刻みを型から読む）。dandori が rulec を子プロセスで呼ぶところは無くなった。残るのは yuen と sakai がツールを呼ぶところ（D.7、D.8）である。
- E の手がかり：
  - `ritsu` の入口は `ritsu dandori` と `--help`、`--version` だけ。ほかの言語の名前には、まだ無いと言って 2 で終わる。E で残りを作るときは、`crates/ritsu/tests/dandori.rs` の確かめ（`ritsu dandori` が規則を同じプロセスで読んで検査し、規則のページを入れて図にすること、無いものは無いと言うこと）を全部の言語に広げる。
  - 生成するコードの予約語の表を一つにするとき（7.5）は、dandori の生成器の `AROUND_RULES`（四つのファイルで、規則の名前と並べて読み込む dandori の名前）も入れるか、dandori の側の名前を替える。替えると、どの出力先の生成物も変わる。
  - rulec の W121（別名が出力先の言語の予約語や標準ライブラリとぶつかる）を、dandori はエラーにしない。`ritsu check` で規則の警告をフローの側にも見せるかを決める。
  - ブラウザで試すページは、記録から答える口（dandori の `src/record.rs` と `sources::Recorded`）で規則を読む。F.5 で rulec と一つの wasm にすれば要らなくなる。
- 見つけたこと（直していない）：rulec が order_state.rule のために生成する TypeScript は、`tsc --strict`（TypeScript 7）を通らない（`Event` を、絞り込んだ値の型に渡しているところが三つ）。dandori のフローで order_state を関数として呼ぶものが無かったので、表に出ていなかった。rulec の生成器の問題である。（D の最後の部分で直した。rulec の §15.171、PLAN の D.11）
- 片づけ：`env-full.sh` が `DANDORI_RULEC` を書いているが、dandori はもう読まない（害は無い）。テストのあとの OS の一時ディレクトリは、この部分の報告に前と後の数を書いた。
- テストの回し方：7.5 と同じ。dandori のテストは rulec のバイナリを要らない（ライブラリの口で読む）。`cargo xtask test --level platforms -p dandori -- --exact <名前>` で重いテストを一つずつ回す。sakai の `what_was_copied_passes_the_suite` は、dandori のワークフローを `ritsu dandori check` で確かめるようになった（dandori のクレートのバイナリは規則を読まないため。D.3 のあとで落ちていたのを、この部分の終わりに直した）。`SAKAI_DANDORI` の代わりに `SAKAI_RITSU` を読み、無ければワークスペースの `target/debug/ritsu` を使う。CI の `tools` も `SAKAI_RITSU` を渡す。

### 7.8 D から E へ（D の最後の部分の終わりに書いた）

- 済んだもの：段階 D の全部（D.1〜D.11）。この部分では D.7（yuen の一式の読み込み）、D.8（sakai の一式の読み込み）、D.9（chobo の単位）、D.11 の確かめをした。入口の最小の形に `ritsu yuen` と `ritsu sakai` を足した（DESIGN 8.6）。7.7 に書いた「見つけたこと」の、rulec が生成する TypeScript が `tsc --strict` を通らないことも直した（rulec の §15.171。コーパスの 50 本の TypeScript を `tsc --strict` にかけるテストを rulec に足した）。言語のクレートで、ほかの言語を読むものは dandori、yuen、sakai の三つで、どれもほかの言語を口で受け取り、自分のクレートのバイナリはほかの言語を持たない（DESIGN 2.3）。
- E の手がかり：
  - 入口：`ritsu` が持つのは `ritsu dandori`、`ritsu yuen`、`ritsu sakai` と `--help`、`--version` だけで、ほかの四つの言語の名前には、まだ無いと言って 2 で終わる。三つの口のつなぎ方は `crates/ritsu/src/main.rs` の `yuen_suite` と `sakai_suite`（dandori は `rulec::ports::Engine` を一つ渡す）にある。E.1 の `ritsu-project` が出す側の実装を作って渡すようになれば、ここは消える。そのとき、同じ規則を yuen と sakai と dandori が別々に読むこと（三つがそれぞれ `rulec::ports::Engine::new()` を持ち、覚えた事実を分け合わない）も無くなる。
  - 名指し（7.10 の X10）：`Items` と `References` は七つの言語の全部が答えるようになった（sakai は `References` を受け取る側でもある）。yuen は名指しを `src/ends.rs` で、sakai は `src/elements.rs` と `src/suite.rs` で、それぞれ自分で引いている。索引に引き方を替えるのは、この二か所である。
  - ほかの言語を読めないときの扱い：yuen のクレートのバイナリは、ほかの言語のものを名指すプロジェクトでは exit 2 で止まり（`ritsu yuen` に同じコマンドを続けた形を言う）、sakai のクレートのバイナリは、規則、カレンダー、ワークフローを含む地図に E104 を出す（exit 1）。どちらも言語のクレートのバイナリのためのもので、リリースのリンクの名前（DESIGN 2.3）がすべての口をつないで動くようになれば出なくなる。台帳の E104 は残す。
  - 確かめていないこと：sakai の api の `not_checked` は、いつも空の並びのまま残した（sakai の DESIGN 9 章）。`ritsu check` が「確かめていないこと」を一つの形で言うなら、それに合わせて決め直す。
  - X4：chobo の単位は ritsu の単位の型になり、口の `BookUnit` が持つ（D.9）。受け取るところ（dandori の `use book`、規則の出力から振替の額へ）で単位と税の区別を確かめる検査は E の X4 である。
  - sakai の doc（sakai の PLAN の D.1）は、chobo の勘定と振替を `Books` で読むところ（`suite::book_names`）まで作った。
  - yuen の `affected` は、geas の記録を二つ渡すと、記録がどちらの側のものかを差分と突き合わせて決める（yuen の DESIGN 8 章）。`ritsu check` が差分を受け取るなら、同じ口（`Claims::affected`）を使う。
- 見つけたこと（直していない）：rulec のテスト関数の名前のうち七つが、Rust の `non_snake_case` の警告を出す（C の前からある。C.13）。
- 片づけ：sakai の環境変数 `SAKAI_RULEC`・`SAKAI_KOYOMI`・`SAKAI_CHOBO`・`SAKAI_RITSU` は、どのテストも読まなくなった（作業場所の `env.sh` には残っていても害は無い）。OS の一時ディレクトリは、テストを回す前と後で、`rulec-*`・`dandori-*`・`TemporaryDirectory.*` は 0 個と 0 個、`com.google.Chrome.*` は 2,391 個と 2,391 個（そのうち今日のものは 18 個と 18 個）、親のいない MCP のサーバーは 0 個と 0 個だった。dandori の Ollama のテストが残す `ritsu-test-*`（四つ、約 106 MB）は、決めたとおりプロセスが終わるまで持つもので、次にテストを回したときに消える（dandori の `tests/examples.rs` の `scratch`）。
- テストの回し方：7.5 と同じ。根から `cargo test --workspace --no-fail-fast -- --nocapture` に、dandori の重い十二を外す `--skip` を付け、十二は `cargo test -p dandori -- --exact <名前>` で一つずつ回す（`cargo xtask test` でも同じ）。rulec の PostgreSQL は使い捨てのクラスタを `PG*` で渡す。yuen と sakai のテストは、ほかの言語のバイナリを要らない（同じプロセスでつなぐ）。TypeSafe の鍵は、手元では作者の環境のものを使い、Jev のテストも回す。CI には鍵を置かない（DESIGN 10.5）。

### 7.9 E の最初の部分から、E の残りへ（E の最初の部分の終わりに書いた）

- 済んだもの：E.1（`ritsu-project`、索引、名指しを索引で引くこと）、E.2（`ritsu check` と、七つの言語の `ritsu <言語>`）、E.3（ritsu の台帳と `ritsu explain`）、E.8（ritsu 自身の地図）。ほかに、受け取る側のクレートのバイナリがほかの言語を読めないときの扱いを一つにし（dandori の E018、yuen の E206、sakai の E104。どれもコードのある診断で、`ritsu <言語> …` で走らせるように言い、exit 2。DESIGN 2.3）、sakai の api の `not_checked` を消した（sakai の DESIGN 9 章）。7.8 の手がかりのうち、入口、名指し、ほかの言語を読めないときの扱い、確かめていないことの四つは済んだ。
- E の残りの手がかり：
  - 言語をまたぐ検査（E.4）の置き場所：`ritsu_cross::check` が、ritsu 自身の診断（`Finding`。`tool` は `ritsu`）と、境目の検査の数（`Borders`。確かめた数と決められない数）を返し、`ritsu check` が言語の診断のあとに出す。要約の行の「borders between the languages」と、JSON の `borders` がその結果である（DESIGN 8.3）。台帳は `crates/ritsu-cross/src/codes.rs` で、E2xx の帯を境目の検査に空けてある（DESIGN 7.1）。再現は英語の名前の小さなプロジェクトにし、`crates/ritsu/tests/codes.rs` が全部を英語と日本語で走らせる。ページ（`crates/ritsu-cross/docs/codes.md` と `codes.ja.md`）は、テストが台帳と突き合わせる。
  - 読み込み：`ritsu_project::Project::load` がプロジェクトを一度歩いて種類を分け、`Joined` が出す側の実装を一度作って受け取る側に渡す（rulec の `Engine` は、読んだ規則の検査の結果を覚えていて、同じ規則を二度読まない）。ファイルをまたぐ参照は `Project::references`、名指しは `ritsu_ports::Index`（各言語の `Items` と `References` を、ファイルごとに一度だけ尋ねる）で引く。E.4〜E.7 はこれを使う。
  - 言語の検査の結果：`ritsu check` は、各言語の `checked`（rulec、koyomi、chobo、geas、dandori の `ports::Engine::checked`、yuen と sakai の `run::checked`）を呼ぶ。言語の `check` の出力を変えると、`crates/ritsu/tests/check.rs` の突き合わせ（ファイルごとに、言語の `check` の出力と比べる）が落ちる。
  - テストのプロジェクト：`crates/ritsu/tests/projects/通販/` は sakai の例をコピーしたもので、日本語の名前のままである。（E の終わりに、英語を先にする担当が英語の `shop/` を足した。`ritsu check` の golden は `通販.*` と `shop.*` を持つ。7.10）
  - ritsu の地図：クレートを足すとき（F の `ritsu-wasm` など）は、`contexts/` のどれかの `owns` に足し、関係を書く（足さなければ `ritsu check ritsu.ctx` が E101 で落ちる）。言語のクレートの下に、わざと通らないファイルを置く新しいディレクトリを作るなら、`ritsu.ctx` の `except` に足す。
- 見つけたこと（直していない）：
  - rulec のテスト関数の名前のうち七つが、Rust の `non_snake_case` の警告を出す（7.8 から変わらない）。
  - 全体のテストで、chobo の `what_only_postgres_has`（REPEATABLE READ で四つの接続から 200 回呼ぶもの）が一度、シリアライズの失敗（40001）で落ちた。生成するクライアントは 10 回までリトライする（chobo の DESIGN）が、ほかの作業で機械が混んでいて使い切ったと見られる。単独で回し直すと通った。この部分は chobo の生成器にもクライアントにも触れていない。
- テストの回し方：段階 E から、作者と決めた回し方にした。書いているあいだは `cargo check` だけにし、項目を書き終えるごとに関わるテストを一回、部分の最後にワークスペースの全体を一回回す。全体は、根から `cargo test --workspace --no-fail-fast -- --nocapture` に、dandori の重い十二を外す `--skip` を付けたもの（7.8 と同じ）で、13 分 17 秒かかり、1,596 件が通った（ignored 1、SKIP 0。chobo の一件は上の「見つけたこと」）。dandori の重い十二は、この部分では回していない。生成器、runner、`.proto` には触れておらず、規則を読むところは、規則がつながっていないとき（dandori のクレートのバイナリ）の止まり方だけを変えたからである（重いテストはどれも規則をつないで走る）。sakai のテストと `crates/ritsu/tests/map.rs` は、Rust の地図のために cargo を子プロセスで走らせる（`CARGO` があればそれ、無ければ PATH の `cargo`。fast の段の決まりのとおり、cargo のほかは要らない）。
- 片づけ：OS の一時ディレクトリは、テストを回す前と後で、`rulec-*`・`dandori-*`・`TemporaryDirectory.*` は 0 個と 0 個、`com.google.Chrome.*` は 2,391 個と 2,391 個（そのうち今日のものは 18 個と 18 個）、親のいない MCP のサーバーは 0 個と 0 個だった。

### 7.10 E から F へ（E の終わりと F の担当の記録を、取り込むときにまとめた）

- 済んだもの：段階 E の全部（E.1〜E.9。E.9 の 1 の、全部を取り込んだ木での全体の回しと dandori の platforms の段も、F の終わりに main で回して通った。DESIGN 10.1）。F のうち、F.1 と F.2（yuen と sakai の段階 D）、F.3、F.5（ブラウザで試すページ）、F.6（Lean の層）、F.7 のうち配るものの準備とバージョンを 0.23.0 にそろえること（サイトの移動と切り替えを除く）。英語を先にすること（koyomi、chobo、sakai、yuen、rulec のコーパス、ritsu のテストのプロジェクト。DESIGN 10.10、10.11）。F.4（LSP）は作らない。
- rulec の組み込みの名前空間に、十二か国の一段目の区分と `std/jp/prefectures` を足した（作者の決め。米国 56、英国 4、中国 33、台湾 22、韓国 17、インド 36、フランス 18、スペイン 19、イタリア 20、ドイツ 16、オーストラリア 8、ブラジル 27、日本 47）。値は英語の名前の ASCII で、規則の中では現地の綴りと ISO 3166-2 の符号（文字で始まるもの）でも書ける。生成の型と値の名前は `prelude.rs` に凍結し、生成先の全部（12）で `Prefecture` と同じ道を通る。検査は、似た綴りと、取り込んでいない国の区分を E012 の注で言い、無い名前空間を E013 の注で近い名前と一覧で言う。`explain` の E012 と E013 を直し、E013 に英語の再現を足した。準用で、取り込んだ列挙を入力にとる呼び先が E042 で準用できなかった欠陥を直し、同じ国の別の名前どうしは区分で読み替えるようにした。英語の双子（`member_shipping_fee`、`yupack_base_fee`、`shipping_fee_proviso`）は英語の綴りにし、英語のページに貼った双子の出力を走らせ直して貼り直した（英語のページの都道府県の日本語の連なりは 109 から 6 に）。rulec の DESIGN §15.182、根の DESIGN 5.6。
- 取り込むときに指示する側が直したこと（それぞれ DESIGN の節に書いた）：
  - dandori の口の API を `checked_with(root, files, &ritsu_ports::Ports, lang)` 一つにした（`ritsu run` の担当の `checked_with_ports` は入れていない。DESIGN 6.1）。`ritsu dandori` は `run_with_undecided` にまとめた（DESIGN 8.1）。
  - 決められない前提の `UndecidedCalls` は、`ritsu check` と同じく `Flows::crossings` でフローを読み、`decide` に日付の口も渡す。koyomi の日の前提で ritsu-cross が決められるものは、確かめる文にならない（DESIGN 7.4）。
  - dandori の `crossings.rs` は、確かめる文 `TK::Check` を、時間を取らない文として `pass` と同じに扱う（DESIGN 7.7）。
  - `ritsu check examples/invoice`（dandori の例）の 8 件の警告（W205 が二つ、W206 が六つ）を正しい振る舞いとし、`ritsu run` の担当のテストの期待をそれに合わせた（DESIGN 8.3）。
  - rulec の DESIGN の節の番号：`ritsu gen` の担当の二つの節は、E.4 の §15.174（`range from koyomi`）とぶつかったので §15.175（生成物の頭）と §15.176（生成パッケージのモジュール）に、英語を先にする担当（rulec のコーパス）の節は §15.177 に送った。
  - chobo の探索（`witness.rs` の `passing_call`）が、額の範囲の上の端がその操作に要る最小の額に届かないときにすぐ答えるようにした。Lean の層の二つ目の担当が、額を 1 に限った在庫の例の `reserve` の探索が終わらず、`ritsu check` が終わらないことを見つけた（chobo の `tests/ports.rs` にテストを足した。DESIGN 7.6）。
  - `ritsu gen` の試しのプロジェクト `stockroom` のフローを、dandori の検査の直し（F.6 の E020）に合わせた（E.7）。
- F の残り：
  - F.1、F.2（yuen と sakai の段階 D）は済んだ。F.3 は、根の README とその確かめ、`skills/ritsu`、各言語の README の入れ方、dandori の `--help` が済み、DESIGN の 1.2 と 10.1 の数も測り直した（F.3）。F.7 のバージョンは 0.23.0 にそろえた（F.7）。
  - F.7 のバージョンを 0.23.0 にそろえることは済んだ（F.7。指示する側がした）。手順は DESIGN 13.1 で、リリースの準備の担当が書いたスクリプト（ルートの `[workspace.package]` に `version` を書き、十五のクレートを `version.workspace = true` にする。指示する側が持っていて、中身はここには載せない）を使った。
  - F.7 のサイトの切り替え（サイトの移動は済んだ。`docs.yml` に push のきっかけを足すこと、元のリポジトリのサイトを止めること、README・スキル・ページの `i2y.github.io/rulec`・`i2y.github.io/dandori` へのリンクと rulec の `Cargo.toml` の `homepage` を ritsu のサイトに直すこと）、formula の切り替え、タグ、公開。クレートの中の古いリリースのファイル（`crates/rulec/.github/workflows/release.yml`、`ci.yml`、`crates/rulec/packaging/`、`crates/rulec/action.yml`）は、ritsu の最初のリリースのあとに消した。rulec の `[package.metadata.binstall]` は外した（`pkg-url` が ritsu のアーカイブと合わず、crates.io にも出さない）。Intel の macOS のビルド、CI での実行（リモートが無い）、`brew audit --online` は、手元では確かめていない。
- 残したこと：
  - E.9 の 1（済んだ。DESIGN 10.1）：全部を取り込んだ木で、根から全体を一度回し、dandori の platforms の段を一度通す。dandori の検査の担当が例のホテルの予約・請求・注文の Temporal 版と `tests/flows/dates_and_books.flow` を直して生成物が変わり、`ritsu gen` の担当が生成物の頭を変えたので、dandori の重い十二（少なくとも Ollama のほかの十一）を回す必要がある。記録した履歴のうち、ホテルの予約（run-52）と注文（run-45 の三つ）は、直したところ（`on failure` と `on cancel`）を通らない実行なので、再生は変わらない見込みである。`ritsu gen` の担当は、生成物の変わった五つを回して通した。決められない前提の担当は、自分の木で重い十二を一件ずつ回して通した。
  - X3 の (a)、X4、X6 が決められないと言うものを、X2 と同じく口で dandori に渡して生成コードで確かめること（`TK::Check` の `PreTest` に種類を足し、`prechecks::insert` で置き、七つの生成器と参照インタプリタとシナリオに一つずつ足す）。確かめる文をブロックをまたいで前へ寄せること（`match` のどの分岐も規則を呼ぶとき）。Lean の `DandoriCore` は確かめる文を知らない（入れるなら、置き場所の決め方が規則を呼ぶ実行を変えないことが定理になる）。
  - dandori の日付の範囲（書き方と、七つのプラットフォームの確かめ）。それまで、ワークフローの入力と `now` から来る日は W205 になる。X5 の長さで、プラットフォームがタスクに付ける既定のタイムアウト（コールバックの一日など）は数えていない。
  - `ritsu run` を、一つの生成パッケージとの突き合わせの基準にすること。`ritsu run --target temporal --format json` の `trace` が計算した結果で流したときの呼び出しの並びで、パッケージを流すランナーを作るなら、帳簿の操作を本物の帳簿（chobo のクライアント）に送り、規則と日付をパッケージの中のコードで計算させ、`trace` と比べればよい。
  - dandori の Temporal の Python は、ネストした関数やラムダの中で読む `let` の変数（`T | None` で宣言する）、サービスを実装するフローのクライアントの型、子のワークフローを名前で呼ぶところで、まだ `mypy --strict` でエラーになる（通販の受注のフローで 6 か所。dandori の DESIGN 7 章）。通販の Go は、二つのフローの名前が日本語で、どちらも Go のパッケージ `workflow` になり、パッケージにできない（`ritsu gen` がエラーで止まることを、テストが確かめる）。
  - `ritsu gen` の `package.rs` の `checked` の dandori の部分は、自分で日付と帳簿の口をつないで確かめている。`Project::check` が三つの口を渡すようになったので、それに替えられる。
  - dandori のクレートの `--help` の終わりの文（「規則を使うワークフローは `ritsu dandori` で走らせます」）と、それをコピーしたサイトの `reference/commands.md`、スキルの `commands.md` と `SKILL.md` は、日付と帳簿のことを言っていない（直すなら `skills/sync.sh` でコピーを作り直し、`crates/ritsu/tests/dandori.rs` の `the_words_after_dandori_are_dandori_s` の文も合わせる）。`ritsu-project/src/joined.rs` の `rules()` の注は、規則の口のことしか言っていない。
  - ritsu-cross の W201 の注の日本語で、名前の後ろに空白が入らない（「`求める額`に範囲がありません」。dandori の W104 は `spaced` で入れている）。
  - Lean の次に足すもの：X6 を koyomi のモデルにつなぐ定理（`inputRange` の範囲で `daysGiven` が成り立てば `KoyomiModel.DatesFile.expect` が「range」のエラーを返さない）、X5 の秒数を dandori のモデルから数えること、rulec の並びの上限の答え、証明書の日の集合を証明にすること（`rulec-recheck` が `.cal` を読むことになる）。`RitsuCross/Borders.lean` と `Relation.lean` は `borders.rs` と `preconditions.rs` を Lean で書き直したものなので、判定を変えたら `crates/ritsu-model/tests/cross.rs` を回し、違えば Lean の側を Rust に合わせる。E201 の注の文を変えたら、`cross.rs` の `example_of` の読み方（`, and at `、`(for one, `、` and can be `）を直す。
  - 英語を先にすることの残り：koyomi の台帳の E111 と W102 の再現は、e-Gov の法令の引用を書くので日本語のまま（足すなら、koyomi が eCFR の節を固定できるようにしてから）。共有の名指しの表 `naming.tsv`（42 行）の 17 行に日本語の名前がある（英語の対を足すなら、ritsu-base、yuen の `names.rs`、sakai の三つのテストの数（`ok == 24 && errors == 18` など）を一緒に直す）。yuen の DESIGN 15 章の `examples/` の 7 つの例は、F.1 で作るときに英語を先にした名前（`<英語の名前>` と `<英語の名前>.ja`）で作る。sakai の `--help` の例のうち日本語の地図 `tests/maps/基本/基本.ctx` を指すものと、yuen の `--help` の例の `tests/fixtures/period` は、日本語の材料を指したまま（直すと `--help` の出力が変わる）。rulec の文書のうち、日本語の規則に走らせた実物のままのもの：`website/rulec/docs/generate.md` の `rulec schema --keys alias` の例（名前と別名が違う規則でしか見せられない）と、`std/都道府県` の値を入力や例に使う所（双子 `member_shipping_fee`、`yupack_base_fee`、`shipping_fee_proviso`）、e-Gov の条文を例にした所（ほかの英語のページの日本語の例は、作者の指示で英語の実物に替えた。DESIGN 10.10）。`std/都道府県` に英語のつづりで書ける別名を足せば、前者は英語の例にできる（言語の変更。作者に聞くことの 4）。rulec の `explain` の台帳の再現例を二つ持たせることと、E043 と E103 の英語の文は、済んだ（DESIGN 10.10、rulec の §15.179）。rulec の英語の出力から、診断の文の日本語の例（30 か所のうち 16 か所。残りは理由のあるもの）、`fmt_big` の `万`・`億`・`兆`（英語は `_` で区切った桁）、`doc` の範囲の `〜`、`--help` の例と引数の名前、E119 の例を外した（DESIGN 10.10、rulec の §15.180）。英語の `doc` の約物（全角のかっこと `「」`）も、取り込むときに直した。残るのは、`tests/golden_en.rs` の `KEPT`（`届け先`）と、JSON の `fix.text` の金額（`witness` と同じデータで、言語で変わらないと `tests/json_v2.rs` が決めているので、英語の出力でも `万` で書く。`range >=-100万円 <=1000万円`）と、`rulec import` が `JPY` の列にも `money[円, incl_tax]` と書くこと（作者が、表の綴りのまま通貨を書くと決め、取り込むときに直した。表に `JPY` と書けば `money[JPY, incl_tax]`）。koyomi の例の名前替えで古くなった sakai の例の一行目のコメントは、英語を先にする担当が直した。
  - 例の名前が変わるとき：`packaging/smoke.sh` と `crates/ritsu/tests/links.rs` が名指す例（`koyomi/examples/net30.cal`、`chobo/examples/inventory/inventory.book`、`yuen/tests/fixtures/rulec`、`rulec/tests/corpus/ec261.rule`、`dandori/examples/hotel/temporal/hotel.flow`）を動かすと、この二つが落ちる。rulec のコーパスに双子を足すときは、`twins.tsv` に一行足し、`tests/threeway.rs` の `CORPUS`、`tests/coverage.rs` の `CORPUS` と `PINNED`、`tests/doc.rs` の `CORPUS` に足す（`doc.rs` の一覧には守りが無い）。
  - `ritsu check` の出力を変えたら、`website/tools/make_wasm.sh` で ritsu.wasm を作り直す（`the_module_answers_as_the_library_does` が落ちる）。ページのプロジェクトについて新しい検査が何かを言うなら、`the_page_starts_in_chrome` が確かめる状態の行の数（英語は `3 errors, 0 warnings`、日本語は `エラー 3 件、警告 0 件`）と、ページの「試してみること」の文を直す。rulec の `make_wasm.sh`（いまは `website/rulec/tools/make_wasm.sh`）は、F.7 でワークスペースの `target/` と `CARGO_TARGET_DIR` で動くように直した。
  - 形の崩れた日付の文字列を返すシナリオが無いので、`date` の形の確かめ（Rust の `render::is_date`、Lean の `isDate`）は試されていない（`dandori scenarios` の範囲）。
- F の担当が残したこと（取り込むときに書いた）：
  - ritsu の `--help` の `--lang` の説明が、`RITSU_LANG` を二度言う（「else the RITSU_LANG environment variable, then RITSU_LANG」）。ritsu-base の `lang_flag(var)` が、`crates/ritsu/src/cli.rs` で `"RITSU_LANG"` を渡されるため（F.3 の担当が見つけた。取り込みのときに、`RITSU_LANG` を渡されたときは一度だけ言うように直した）。
  - 各クレートの `Cargo.toml` の `repository`：済んだ。作者の指示で、言語の七つのクレートを `repository.workspace = true`（`https://github.com/i2y/ritsu`）にした。rulec の `homepage` は、公開している `https://i2y.github.io/rulec/` を指すので、サイトを切り替えるまで残した（DESIGN 13.2）。
  - yuen の担当の提案：`ritsu.ctx` の `except` に `"crates/yuen/examples"` を足すか。いまのままでも `ritsu check ritsu.ctx` は通る（`code rust "."` だけを数えるので、例の `.req` などは数えない）が、ほかの言語の `examples` と並びをそろえるなら足す。
  - F.3 の担当が残したこと：根の README の「リポジトリ」の節の、七つの言語の README へのリンク（yuen と sakai の README が入ったので足せる。足せば `readme.rs` が先の有無を見る）。根の README とスキルの `ritsu gen` のコマンド名の確かめ（`ritsu gen` が入ったので、`ritsu --help` の一覧と突き合わせられる）。yuen と sakai の README の入れ方とスキルの `compatibility` を、`readme.rs` の「入れ方」の確かめの対象に足すこと。rulec と dandori のサイトとスキルのコピーのリポジトリへのリンクは、F.7 で ritsu のリポジトリの `crates/<言語>/…` に直した（`tests/website.rs` が先の有無と、一つの言語のリポジトリを名指さないことを確かめる。rulec のリリースの URL だけは、ritsu の最初のリリースが引き継ぐまで残す）。`Cargo.toml` の `repository` も F.7 でそろえた。サイトへのリンク（`i2y.github.io/rulec`、`i2y.github.io/dandori`）は、切り替えるまで公開しているほうを指す。
  - sakai の Mermaid のテストのための `crates/sakai/tools/mermaid/node_modules` は git に入れないので、本のリポジトリで `npm ci --prefix crates/sakai/tools/mermaid` を一度走らせる（F.2）。
- 作者に聞くこと（★）：
  1. リリースの前のこと（リリースの準備の担当）：リポジトリ `i2y/ritsu` を作って public にし、GitHub Actions を有効にすること（`Cargo.toml` の `repository` と formula の URL が指す）。シークレット `HOMEBREW_TAP_KEY`（`i2y/homebrew-tap` に書き込める deploy key）を `i2y/ritsu` に置くこと。最初のタグの前に `packages.yml` を手で走らせること。`i2y/tap/rulec` を ritsu に移す時期（最初のリリースと同じときか、そのあとか。DESIGN 13.2）。答え：最初のリリースと同じときに移した（2026-10-05）。
  2. X4 の額の下の端：作者が、chobo に合わせて額 0 を通すと決めた（0〜2⁶³ − 1。2026-10-04 19:30）。X4 の担当が、判定、額を範囲に限った chobo の探索、台帳の E203（文と再現。再現は 0 から −20 に）、Lean の `RitsuCross`（`amountFits_fails_chobo` を足し、判定が chobo と過不足なく同じになった）、突き合わせ、ブラウザで試すページの wasm を直した。取り込むときに、その担当が見つけた chobo の `passing_call` の五つ目からの額の引数も、探索を限った範囲の下の端を使うように直した（DESIGN 7.6）。★額 0 を ritsu が何も言わずに通すことでよいか（0 の振替を業務として避けたいかは、帳簿の書き手が決めることとした）。
  3. dandori の例の直し方の文言（ホテルの予約の `CleanupFailed`、注文の `ShippedAlready`、請求の `already_posted`。dandori の検査の担当が決めた）でよいか。DandoriCore の前提二つ（案件を始める呼び出しの失敗、拒否のエラー）は、指示する側が決めて入れた（DESIGN 11.2 の 4）。
  4. rulec のコーパスを英語を先にした担当の四つ：双子の名前（元の別名に言葉を足した形。`_en` を付ける形もある）でよいか。`std/都道府県` に、規則の中で英語のつづりで書ける別名を足すか（足さなければ、三本の双子に都道府県の値が日本語で残る）。日本語の規則のまま残した文書と画面写真（上の「英語を先にすることの残り」）でよいか（続けるなら、`formats.md` と `generated-code.md` は日本語の名前の例を残したうえで英語の名前の例を先に足す形がよい）。E043 と E103 の英語の文を直すか（コマンドの出力が変わる）。（`std/都道府県` の英語の別名は、rulec の英語の文書を直した担当も同じことを聞く。足せば、英語のページから都道府県の値の大部分（examples.md の 47 の名前など）を消せる。）（決まった：作者が各国の一段目の区分を入れると決め、入れた。`std/jp/prefectures` は都道府県を英語の綴りで持ち、どちらの取り込みでも `東京都` と `Tokyo` の両方を書ける。三本の双子は `std/jp/prefectures` と英語の綴りにした。rulec の DESIGN §15.182、根の DESIGN 5.6。）
  5. 担当が★を付けた決めごと（それぞれ DESIGN の節に★で書いた）：X1 を言語の検査のまま残す（7.3）。`range from koyomi` の書き方と、koyomi がつながっていない rulec を E129・exit 2 で止める（7.5）。X5 が成り立つとき何も言わない、X6 をカレンダーのデータの範囲と比べない、dandori に日付の範囲の書き方を足さない、X4 の拒否の理由は境界の理由だけを比べる（7.6〜7.8）。決められない前提を確かめる場所と `Dandori.BrokenPrecondition`（7.4）。`ritsu run` の `--target`、走らせる前の帳簿の書き方、時間の数え方（7.9、8.1）。`ritsu gen` の `go.mod` を書かないこと、既定の名前 `generated`、`--books` の既定、`--lang` のコメント、頭のバージョンと 16 桁のハッシュ、`mypy --strict` のための単独の出力の変更（9.2〜9.4）。ブラウザで試すページを wasm32-unknown-unknown と土台の入口で作ったこと（4.15）。Lean の検査が `for … in parallel` を受け付けないこと（11.2）。
  6. yuen の段階 D の担当の三つ：わざと止まる例の E303 が一つ（A の段階の見込みは三つ）でよいか。コピーしたファイルを koyomi のいまの名前（`civil_code_period_end.ja.cal`）にしたこと。greeter・payment_terms・refunds の出どころを `decided`（この例のために決めたもの）にしたこと（F.1）。
  7. sakai の段階 D の担当の四つ：doc のために `Suite` に `Dates` を足し、共有の ritsu-project を一行直したこと（DESIGN 4.8）。`Cargo.toml` の `repository` を ritsu にしたこと。CI の `tools.yml` を直したこと（外すなら `ci/skips/tools.txt` に `sakai every_mermaid_chart_draws` を足す）。日本語の例の README を `examples/shop.ja/README.ja.md` にしたこと（GitHub はディレクトリを開いたとき `README.ja.md` を自動では見せない。`README.md` にする案もある）。
  8. F.3 の担当の五つ：rulec の配り方（Homebrew、`.deb` と `.rpm`、アーカイブ、`uses: i2y/rulec@v0.22.1`）を残して入れ方の節に書き添えたこと（DESIGN 13.2）。指示の外で直した四つ（四つのスキルの `compatibility`、rulec のスキルの `license`、根の README の「AI エージェント向け」の節、dandori の `--help` の exit code の文二つと README の一文）。README の `console` の塊を走らせる場所の三通り、スキルが言語のスキルを Markdown のリンクでなくパスで案内すること、コードの塊を一つのファイルの行と順に突き合わせること（DESIGN 10.12）。ritsu の `--help` の `--lang` の文の誤りを、どの担当で直すか。
  9. rulec の台帳の再現を英語でも持たせた担当の★：E103 の英語の文の例を、`1JPY` の固定ではなく列の単位から作る形にしたこと（取り込むときもこの形のまま）。E011 と e-Gov の四つ（E037・E038・E039・W119）の再現を日本語のまま残したこと（条の名前を英語の別名で書けるようにするのは言語の変更）。英語の説明 28 項は、取り込むときに入れた。
  10. サイトの担当の★：dandori の前のブラウザで試すページを、ritsu のサイトに残すか、ritsu のページに替えるか（DESIGN 15 章。いまは残してある）。根のサイトにロゴと favicon を付けるか。切り替えるまで、README・スキル・ページから公開しているサイト（`i2y.github.io/rulec`・`i2y.github.io/dandori`）を指したままにしたこと。（決まった：前のページは、ritsu のページに例を全部持たせたうえで、切り替えのときに ritsu のページへ転送する。ロゴは案を作る（案はあるが、まだ入れていない）。サイトの切り替えは、ritsu を public にしたらすぐにする。）転送に替え、サイトも切り替えた（2026-10-05。F.5）。
  11. rulec の英語の文書を直した担当の★：英語の画面写真、`compare.md` と `scenarios.md` の突き合わせの実演、`flow.svg` の三つは、rulec のサイトの担当が `parcel_rate` と英語に替えた（済んだ）。W114 の防壁の材料を `tests/pages/` に置いたこと（`tests/mutants/` に足すと、確かめ方のページと図の本数が変わる）。
  12. rulec のサイトを移した担当の★：ritsu の `tests/website.rs` に `github.com/i2y/<言語>/releases` の例外を足したこと（入れ方のページに rulec のリリースの URL を残すことと、言語のリポジトリを名指さない確かめがぶつかるため。ritsu の最初のリリースのあとで外す）。入れ方のページの冒頭に「ritsu の最初のリリースが引き継ぐ」の段落を置いたこと。`crates/rulec/.github/workflows/docs.yml` を消したこと（`ci.yml` と `release.yml` は残した）。`compare.md` を、北海道の行だけでなく続けて使っている例の全部（沖縄県・江戸・滋賀県を含む）を `parcel_rate` にしたこと。`scenarios.md` の 2-1・2-2 の話の規則名 `rules/shipping_fee.rule` をそのままにし、2-3 から先を実物の `parcel_rate.rule` にして、そのことを書いた一文を足したこと。英語の写真のドックを運賃表の `base_rate` にしたこと。`/releases` の例外は、ritsu の最初のリリースのあと（2026-10-05）に外した。入れ方のページの冒頭の「ritsu の最初のリリースが引き継ぐ」の段落も、ritsu のリリースの案内に替えた。
  13. rulec の英語の出力を直した担当の★：英語の桁は `_` で、5 桁以上のすべての整数を区切ること（`,` にも、1 万の倍数だけにも、それぞれ一行で替えられる）。JSON の `fix.text` は英語でも `万` のままにしたこと。`doc` の範囲の `〜` を英語では `to` にしたこと。英語の `--help` の `doc` の例から `--lang ja` を外したこと。`rulec import` が `JPY` の列にも `円` と書くこと（直していない）。顧客向けのページの金額の書き方（英語では `10_000_000JPY` になる。`,` や単位の位置は別に決めるのがよい）。英語の `doc` の約物は、取り込むときに直した。（決まった：英語の大きい金額は `_` で区切ってよい。`rulec import` は表の綴りのまま通貨を書く（取り込むときに直した）。）
  14. 各国の区分の担当の★：列挙の名前を `<国>_<種類の単数>`（`us_state`、`jp_prefecture`）にしたこと。値の書き方（CLDR の英語の名前からダイアクリティカルマークと略語の点を落とし、空白・ハイフン・アポストロフィを `_` に。大文字小文字は英語のまま）。`std/jp/prefectures` の値を英語、`std/都道府県` を日本語にしたこと（DESIGN 5.6）。
  15. yuen の名前空間の担当の★：名前空間のページを nav に出さないこと（出すなら二つの設定に一行ずつ）。ページに「語の意味は変えない」といった互換の約束を書くかどうか。yuen の公開 API に `Kind`・`Term`・`TERMS` が増えたこと（DESIGN 13.2）。
  16. 前のページの例を移した担当の★：rulec の前のページを転送するときに `#project=rulec/gap`（日本語は `rulec/gap.ja`）を付けること（前のページは URL を読んでいなかった。そのとおりに入れた。ハッシュが `=` を含まないときだけ。DESIGN 8.7）。共有のリンクの `edits` を、開いたときのプロジェクトとの差分にしたこと（例のもとが後で変わると、古いリンクは新しい例に差分を当てる）。プロジェクトの名前と、dandori の例のパスをクレートの中のままにしたこと（DESIGN 8.7）。
  17. 日本語の出力を読み直した担当の★（文体と語）：chobo のキーの行を「一度だけ動きます」にしたこと、chobo の `doc` のページと操作の列の結果の欄を常体のまま残し、koyomi のページの敬体とそろえていないこと、koyomi の「計算の段」を残したこと（ja-a）。`ritsu check` の要約を「どれも検査を通りました」にしたこと、sakai の要約と doc のページをだ・である調のまま残したこと（ja-b）。yuen の出力から「端」を外したこと、yuen の DESIGN の地の文と根の文書・サイトの「端」を言い換えるかどうか、yuen の trace の「そのあと変わっていない」「〜から効く」と、doc のページを常体のまま残したこと（ja-c）。dandori の doc のページの表のセルの並びを日本語で「・」にしたこと、「元のエラーで失敗する」と規則のリトライの並び、`--help` の短い説明を言い切りのまま残したこと（ja-d）（DESIGN 4.1）。
  18. 言い方の担当の★：規則を決める人の役割としての承認（rulec の AGENTS.md と README の「someone approves the table」、rulec の DESIGN 0.1 の「承認する人がいて」、サイトの「Does a person approve the answer」）を残したこと。rulec のサイトのシナリオを「承認」から「確かめる」にしたこと。ritsu-cross の台帳から、検査の印（X2〜X6、「X3 の (a)」）も番号と一緒に外したこと。生成コードのコメントと doc のページに入るスクリプトのコメントも、利用者に見える出力として扱ったこと。rulec のテストのコーパスの規則の `description` にある §（「(§15.132, §15.133)」など）を、コーパスのハッシュが動くので残したこと（DESIGN 4.3、4.8）。
  19. rulec の診断と `--help` を読み直した担当の★：`replay` と `diff` の見出しの「金額」を「差の合計」にしたこと（付与点のような金額でない出力にも出ていたため。英語の amount はそのまま）。「群」を「グループ」にしたこと（キーワードは `group` で、W111 とサイトはすでにグループだった）。MCP のツールの説明のつなぎを日本語にしたこと（`tr!` を足した。英語は同じ）（DESIGN 4.1）。
  20. rulec の台帳と人が読むページを読み直した担当の★：規則の `v1` の「版」を「バージョン」にするか（rulec の診断とページが広く使う rulec の語なので残した。dandori と chobo・koyomi の読み直しは「バージョン」にしている）。E116 と W120 の「金額」（E116 は率も比べるので「出力の値」のほうが正確だが、検査の名前なので残した）。英語の W114 の「いつ出るか」の "`twice` above" は、`explain` では最小の再現が下に出るので "below" が正しい（英語は変えない決まりなので残した。日本語は「下の最小の再現の `倍`」に直した）。ページの中で常体だった文を敬体にそろえたこと。ページの頭の刻印の「本物は」を「もとになるのは」にしたこと。入力の注記を、主語を書いた「生成コードが呼び出し側のオブジェクトから読みます: `…`」にしたこと（DESIGN 4.1）。
  21. `ritsu check` の道の出し方：プロジェクトの根より下のディレクトリで走らせると（例：リポジトリの中の `website/playground/shop` で `ritsu check .`）、yuen はプロジェクトの根からの道（`website/playground/shop/billing/rules/…`）を、ほかの言語はいまの場所からの道（`billing/rules/…`）を出し、一つの出力の中で食い違う。ブラウザで試すページでは根がプロジェクトそのものなので出ない。勧めは、どの言語もいまの場所からの道にそろえること（ritsu-base の `Shown` の使い方を yuen の `shown_any` にも当てる）。
- 作者の指示（2026-10-04 19:30）で入れたこと：X4 は chobo に合わせて額 0 を通す（DESIGN 7.6、11.2）、rulec と dandori のサイトを ritsu のサイトへ移す準備（切り替えはしない。F.7、DESIGN 13.2）、英語のページで rulec の文書が日本語を使っているところを英語に（DESIGN 10.10。rulec の §15.178 と §15.179）、各クレートの `repository` をそろえる（DESIGN 13.2）。rulec の DESIGN に足した節は、§15.178（英語の文書）、§15.179（台帳の英語の再現）、§15.180（英語の出力から日本語の例と万・億・兆を外す）、§15.181（サイトを `website/rulec/` に移し、英語のページの残り三つ）である。
- rulec のサイトの移動は済んだ（F.7。サイトの担当が申し送った手順のとおりに入れた）。
- サイトを切り替えるとき（作者が決めたら）：`docs.yml` に `push: branches: [main]`（パスは `website/**`、`crates/rulec/AGENTS.md`、`crates/rulec/docs/**`、`.github/workflows/docs.yml`）を足し、`crates/ritsu/tests/website.rs` の `the_workflow_that_publishes_the_site_runs_only_by_hand` をそれに合わせて直す。リポジトリの Settings の Pages を「GitHub Actions」にする。README・スキル・ページの `https://i2y.github.io/rulec/…` と `https://i2y.github.io/dandori/…` へのリンクを ritsu のサイト（`https://i2y.github.io/ritsu/rulec/…`、`https://i2y.github.io/ritsu/dandori/…`）に直す。rulec では、README の入れ方の `[how]` のリンク、`tools/make_examples.py` の日本語の例のページへのリンク（直したら `python3 website/rulec/tools/make_examples.py` と `sh crates/rulec/skills/sync.sh` を走らせる）、`Cargo.toml` の `homepage`。dandori では `skills/sync.sh` の `site=`（スキルのコピーが変わるので `skills/sync.sh` を走らせる）。元のリポジトリのサイトを止める（または ritsu のサイトへ案内するページに替える）。
- サイトを切り替えた（2026-10-05 の昼、作者の指示で）：リポジトリを public にし（12:12）、Actions を有効にして、Pages を「GitHub Actions」にし、`docs.yml` を手で一度走らせて `https://i2y.github.io/ritsu/` を出した。`docs.yml` には上の push のきっかけを足し、`tests/website.rs` を合わせた。README・スキル・ページの `i2y.github.io/rulec/…`・`i2y.github.io/dandori/…` へのリンクと、rulec の `Cargo.toml` の `homepage`・`documentation`（まだ残っている rulec の古い packaging の `homepage` も）を ritsu のサイトに直し、rulec の例のページと二つのスキルを作り直した。スキルの中の二つのリンク（ブラウザで試すページと、CI の例の `uses: i2y/ritsu@v0.23.0`）は、そのまま残す（作者の答え）。ritsu のサイトの中の前のブラウザで試すページは、転送のページに替えた（F.5）。元の rulec と dandori のリポジトリのサイトも、来たパスをそのまま ritsu のサイトの下へ、クエリと `#…` ごと送るページだけを出すようにした（各リポジトリの `moved/` と `docs.yml`。前のブラウザで試すページへのリンクは、ritsu のサイトの転送のページを通って ritsu のページに着く）。リリースは、2026-10-05 の夜、作者の指示で v0.23.0 を出した（F.7）。
- CI を初めて回した（2026-10-05 の昼）。`tools` の三つの組が落ちた原因と直しは DESIGN 10.5 と 10.6。直したあと（同じ日の夜の push）は、六つのワークフロー（`fast`、`proofs`、`kani`、`packages`、`docs`、`tools` の三つの組）が全部通った。`kani` を GitHub で回したのも、このときが初めてである。足した段は ubuntu-latest で入り、dandori の組で帳簿の突き合わせと ritsu のパッケージの型の確かめが走った。rulec の組で初めて走った三つの段（生成したコードを全部の言語で走らせる `--require-all`、rulec を使わない証明書の再検査、fmt とコミットした出力）も通った。chobo の突き合わせの流し直し（`run again:`）は、その回は出なかった。geas の揺れと、REPEATABLE READ の 30 回が足りるかは、この先の回でも見る。
- rulec のテストの日本語の SKIP の理由（109）を英語にそろえるか（DESIGN 10.3）。
- chobo の PLAN.md（433 行）の「10 回までリトライ」は、言語の PLAN なので直していない（DESIGN 4.1 は 30 回に直した）。
- geas の `port auto` の揺れに、決まって再現するテストは足していない。起動中のプログラムがいつ exec するかを、テストから決められないからである。確かめは Linux のコンテナで、直す前 14 回のうち 4 回落ち、直したあと 10 回とも通ったことで行った（`reports/ci-logs/geas-linux-repro-*.txt`）。
- 同じ日の昼に直したこと：サイトのトップのページ（英日）に「For AI agents」「AI エージェント向け」の節を足し、八つのスキルと四つの入れ方を載せた。rulec と dandori の入れ方のページに残っていた、マーケットプレイスをリポジトリから足す古い入れ方を直し、`tests/skill.rs` が、リポジトリのどのページでもマーケットプレイスをサイトの URL で足していることを確かめるようにした。chobo の人が読むページは、ブラウザで試すページの枠（`sandbox="allow-scripts"`）の中で `history.replaceState` がエラーになり、シナリオもステップも切り替わらなかったので、rulec と dandori のページと同じく `try` で包んだ。
- ritsu の最初のリリースが出たあと（2026-10-05 の夜）：rulec の入れ方のページ（英日）と README の、Homebrew・`.deb`・`.rpm`・アーカイブ・CI を ritsu のもの（`i2y/tap/ritsu`、`ritsu_<版>-1_<arch>.deb`、`ritsu-v<版>-<target>.tar.gz`、`uses: i2y/ritsu@v0.23.0`）に替え、rulec 自身のリリースから移る人の節を足した。貼った出力は 0.23.0 のリリースの実物で取った（`apt install` と `dnf install` は走らせていないので、出力を貼っていない。Homebrew は、作者の Homebrew とは別の使い捨ての Homebrew 7.0.6 で、新しく入れる場合と、移り方の二つの道を走らせた）。互換の約束（英日とスキルのコピー）の `i2y/tap/rulec` と `uses: i2y/rulec@v1.0.0` も ritsu にした。ritsu の `tests/website.rs` の、言語のリリースへのリンクを許す例外（`/releases`）を外した。クレートの中の古いリリースのファイル（`crates/rulec/.github/workflows/release.yml`、`ci.yml`、`crates/rulec/packaging/`、`crates/rulec/action.yml`）を消し、rulec の `[package.metadata.binstall]` を外した。rulec の `experiments/library/` の CI の見本を `uses: i2y/ritsu@v0.23.0` にした。根の README とサイトのトップページ（英日）の「リリースを出す予定」も直した。rulec の DESIGN §15.186。
- 作者が 10/4 の夜に決めたこと（作者に聞くことの答え）：(1) 各国の一段目の区分を入れる（12 か国。上の「済んだもの」、DESIGN 5.6）。(2) `rulec import` は表の綴りのまま通貨を書く（取り込むときに直した）。(3) 英語の大きい金額は `_` で区切ってよい。(4) 前のブラウザで試すページは、ritsu のページに例を全部持たせたうえで、切り替えのときに ritsu のページへ替える（転送。F.5、DESIGN 8.7、13.2、15 章）。ロゴは案を作る（案はあるが、まだ入れていない）。(5) yuen の名前空間を `https://i2y.github.io/ritsu/ns/yuen#` にする（F.7）。(6) サイトの切り替えは、ritsu を public にしたらすぐにする。(7) 言語の側の「承認する人」も README の言い方にそろえる（`--audience approver` の値は残す。DESIGN 4.8）。あわせて、全部の言語の日本語の出力を読み直し、出力から節の番号を外した（F.3）。rulec の DESIGN の節は、§15.182（各国の区分）と §15.183（ページの言い方と節の番号）である。
- 作者が 10/5 の朝に決めたこと：ロゴの案は取り下げる（サイトにロゴは置かない）。八つの Agent Skills を、Claude Code のプラグインとマーケットプレイス、`ritsu skills install`、フォルダーをコピーする入れ方、リリースの zip で配る（F.3）。担当が決めて作者に聞く項：プラグインの source をリポジトリの根にしたので、入れるとリポジトリ全体がプラグインの置き場所にコピーされる（public にする前に、スキルだけを持つ軽い形にするかを決める）。`ritsu skills install` の既定の書き先（いまいるディレクトリの `.claude/skills/`）と、上書きせずに止まったときの exit code 1。スキルの中のブラウザで試すページへのリンク（`https://i2y.github.io/ritsu/playground/`。切り替えまでは 404）と、CI の例の `uses: i2y/ritsu@v0.23.0`（ritsu の最初のリリースまでは動かない）。
- 10/5 の朝の続き：プラグインの重さは、マーケットプレイスをリポジトリのルートから外し、サイトが公開する `marketplace.json` にして片づけた（プラグインは `git-subdir` で `skills/` だけを取ってくる。DESIGN 13.2）。`ritsu skills install` の既定の書き先と exit code 1 はいまのまま。スキルの中の二つのリンク（ブラウザで試すページと、CI の例の `uses: i2y/ritsu@v0.23.0`）は、まだ決まっていない。日本語の文書と出力から「断る」「写す」を除いた（DESIGN 4.1）。担当を言語ごとに五つに分け（chobo、rulec、yuen と sakai、koyomi と dandori、ritsu と根の文書）、置き換えの語は全員で同じにした。取り込みで、担当の間で食い違った文（`ritsu run` のテストが比べる dandori の文、yuen と koyomi の `source fetch` の報告、yuen の E104 に出る ritsu-base の文）をそろえ、ほかの言語の文を埋め込んだ golden（ritsu、dandori、yuen、ritsu-proto）と、ブラウザで試すページの三つの wasm と `projects.json` を作り直した。合わせて 498 のテキストのファイルで、「断る」の仲間を含む 2,624 行と「写す」の仲間を含む 1,289 行を替えた（golden と生成したページを含む）。
- 区分を取り直すとき：生成の台本は持ち込んでいない。表は `crates/rulec/src/prelude.rs` にあり、CLDR から離れたところ（米国の準州の名前、台湾の `CYI` と `CYQ`、区分の種類の語を外した五つ、韓国の新しい正式の名）は rulec の DESIGN §15.182 に書いた。表を変えたら `tests/regions.rs` の数と、`website/rulec/tools/make_wasm.sh` と `website/tools/make_wasm.sh` の二つの wasm を作り直す。綴りは規則の中でだけ効く（生成コード、`verify`、`replay`、`fixtures lint` は値の綴りだけを受け取る）。記録の入力に現地の綴りが来る現場が出たら、`fixtures lint` で案内するかを決める。
- Unicode CLDR のライセンスのコピー（`crates/rulec/THIRD_PARTY_NOTICES`）を、リリースのアーカイブ（`packaging/archive.sh` の `licenses`）と `.deb`・`.rpm`（`packaging/nfpm.yaml`）に入れる（DESIGN 13.2。まだ。`crates/ritsu/tests/release.rs` も合わせる）。
  - 済んだ（2026-10-06）。根の `THIRD_PARTY_NOTICES` に CLDR の節として入れ、アーカイブ、`.deb`・`.rpm`、Homebrew の keg に入れた。`release.rs` を合わせた（DESIGN 13.2）。
- yuen の PROV の名前空間の IRI（`https://i2y.github.io/ritsu/ns/yuen#`）は、ritsu のサイトを公開したあとにしか開けない（2026-10-04 に `curl -I https://i2y.github.io/ritsu/ns/yuen` は 404 を返した）。yuen と、PROV を書き出す ritsu を配るより前に、`docs.yml` でサイトを公開する。配る前に `git grep -n -I 'i2y/yuen/ns'` が空で、`strings website/docs/playground/ritsu.wasm | grep -c i2y/yuen/ns` が 0 であることを確かめる。
- 日本語の読み直しの残り：ritsu-base の台帳が書く二つの文（`explain` の再現の「…そこで `…` を走らせます」と、`docs/codes*.md` の頭の「手で直しません」）は、決まりでは「走らせてください」「手で直さないでください」になるが、koyomi・chobo・yuen・sakai・ritsu-cross の `docs/codes.ja.md` にも出るので、全部を取り込んだあとに一度で直す（ritsu-base の二行と、それらの `docs/codes.ja.md`、`crates/ritsu-base/tests/golden/compat/` を取り直す）。ritsu-base の `--help` の見出し「exit code:」「この画面を出す」、診断の見出し「確かめたら」、koyomi の `gen --lang ja` の生成物のコメントの空白、yuen の `src/check.rs` の E105 と同じ形の文、`crates/ritsu-base/tests/golden/compat/` の koyomi と yuen の古い文のコピー、根の README.ja.md のクレートの一覧の「口」、根のサイトの `website/docs-ja/ns/yuen.md` の地の文の「端」（六か所。言い換えの案は yuen の読み直しの担当が出した）、dandori の `--lang ja` でも英語のまま出る文（読めないファイル、`scenarios --out` の行など）。 rulec の読み直しの残り：ritsu-base の共通の行（`--help` の見出し「exit code:」と、`出しうる診断（…）` の行。rulec の `cli.rs` が持つ同じ行は「出しうる診断（`rulec explain <CODE>` で一つずつ説明を読めます）:」にした）。rulec の `diag.rs` の見出しの書き方は残した（生成物のコメントの「入口で断る」は、2026-10-05 に「入口でエラーにする」「受け付けない」に替えた。DESIGN 4.1）。rulec のサイトの地の文（出力の貼り付けではないもの）に、`website/rulec/docs-ja/tour.md` の「それを見ていない表が完全性検査で割れるのが狙いです」と `docs-ja/index.md` の「丸め方で円がいくら動くか」が残る。実験の記録（`crates/rulec/experiments/` の下）と、rulec の DESIGN.md の測った記録の引用は、そのときの出力の記録なので取り直していない。rulec の DESIGN の §11 の合意済みの五つの文面は、決めたときの記録として残した。図の URL の `?v=` は、ほかの担当が図を描き直したら `cargo test -p rulec --test website 図のurlは中身のハッシュを持っている` が言う値にそろえる。
- テストの回し方：7.9 と同じ。Lean の層は、根の `proofs/` で `lake build` してから `cargo test --release -p ritsu-model`（`proofs/` を作っていないと SKIP する）。rulec の `tests/lean.rs`・`days.rs`・`machine.rs` は根の `proofs/` の `rulec-recheck` を使う。dandori の重い十二は、ほかと同時でなく一つずつ回す。

### 7.11 OpenSpec、sakai の OpenAPI と AsyncAPI、依存の監査（2026-10-05〜06）

三つ（OpenSpec との連携、依存の監査と生成器の直し、sakai の OpenAPI と AsyncAPI への対応）を、三人の担当が並べて作った。

**OpenSpec との連携**（openspec の担当）

OpenSpec の仕様（要件とシナリオ）と変更の提案を、yuen と geas が読む。変えたのは、ritsu-base の読み手、yuen、geas、ritsu のスキルの四つ。

- **yuen**：OpenSpec の仕様を、法令と同じく、要件ごとに固定して読む出典の種類 `openspec` を足した（yuen の DESIGN 20 章）。仕様の要件が変われば（MODIFIED を含む変更を archive すれば）、`check` が固定で止まって差分を見せ（E103）、固定し直すと、その要件を引くリンクと、その先の規則・フロー・コード・主張へのリンクに印が付く（E302）。まだ archive していない提案は `source outdated`（通信しない）と `affected`（提案のフォルダーを足す差分）が読み、どの要件と持ち主とリンクに届くかを言う。新しいコードは E108（仕様に無い要件）、W102（どの出典も固定していない仕様の要件）、W402（OpenSpec の要件を引き geas の主張で確かめている要件で、シナリオに同じ名前の主張が無い）。
- **geas**：`geas scenarios <spec.geas>... --openspec <path>...` を足した（geas の DESIGN §17）。シナリオごとに同じ名前の主張を探し、主張の無いシナリオを言う（exit 1）。`--draft` は、主張の無いシナリオの主張の下書き（本体はコメントだけで、E005 で止まる）を出す。新しいコードは E090。
- **スキル**：ritsu のスキルに「OpenSpec と各言語の使い分け」の節（8 章）、yuen と geas のスキルに OpenSpec の扱いを足した。
- **ritsu-base**：OpenSpec の読み手（DESIGN 4.16）。
- 例：yuen の `openspec_greeter`（まだ archive していない提案のあるもの）と `openspec_greeter_archived`（`openspec archive` のあと。わざと止まる）、geas の `examples/greeter/openspec/`。どれも英語が先で、日本語の版を `ja/` の下に置いた。
- 残したこと：OpenSpec のストア（ベータ）とほかのリポジトリの仕様、シナリオの単位の固定、Spec Kit と Kiro の仕様、`ritsu check` で OpenSpec のシナリオの網羅を言うこと（言語をまたぐ検査に足すかは、まだ決めていない）。

**sakai の OpenAPI と AsyncAPI**（sakai-api の担当）

- sakai の DESIGN 15 章。入れたもの（`ritsu_base::yaml`、sakai の `src/contracts.rs`、E108・W104・E210、例の webshop）。残したもの：
  1. （2026-10-06 に済んだ。7.12 の「参照の書き方のツール名」。）名指しの決まり（DESIGN 6.2）に、ツールの語 `openapi` と `asyncapi` と、種類の語（`schema`（下に `value`）、`channel`、`message`、`operation`）を足す。yuen の DESIGN 2.2 の表、yuen の診断の文（ツールの語の並び）、ritsu-base の `naming.tsv`、sakai の `.ctx` の長い書き方と api の書き方（いまは `{"pointer": …}`）を一緒に変える。
  2. dandori の口（`References`）で、`http` のタスクが呼ぶ操作（文書と操作）を言う。sakai はそれを E210 で確かめる（いまは `use openapi` を文書の単位で数えるだけ）。
  3. rulec の口で、`import jsonschema` が取り込む JSON Pointer を言う。sakai はそれを対応の先として読む（`import proto` と同じく、規則が対応になる）。
  4. rulec の `import jsonschema` と dandori の `use openapi` が、`ritsu_base::yaml` で YAML の文書も読む。rulec の `src/jsonschema.rs` が YAML を読まない理由（その文書が使う部分だけを読む読み手は黙って読み違える）は、読む部分を決めてその外を止める読み手で解けている。
  5. sakai の `build`：文書の `$ref` から、生成したコードどうしの import を許す（sakai の DESIGN 7.1 の表の最後の行。いまは proto の import だけから作る）。
  6. 根の README（英日）の sakai の段落に、OpenAPI と AsyncAPI の文書を足した（2026-10-06）。

**脆弱性の検査**（vuln の担当）：ritsu 自身の依存の監査（DESIGN 3.6）と、生成器がソースの文字列をコメントやスクリプトの外に出さない直し（9.2）を作った。ritsu を使う人のための脆弱性の検査は、2026-10-06 に、ritsu の地図と契約から見える脆弱性を言う言語の検査にすると決めた（使う人のプロジェクトの依存を調べるものではない）。秘密の値、平文の通信、認証の無い操作、外へ出すデータの境界の四つで、同じ日に作った（7.12、DESIGN 16 章）。ritsu 自身の依存の監査は、その検査とは別のものとして残した。

残したこと：

- `audit.yml` を GitHub で走らせること（push してから）。走らせて確かめるのは、Linux のバイナリが取れること、`cargo-deny --locked check` と osv-scanner がランナーで通ること、`release.yml` から呼べること。
- yuen の `tools/requirements.txt` を、依存まで固定したロックファイルにすること（いまは prov と reqif の二つの名前だけ）。
- dandori の、ほかのコメントに入る文と、Argo の注釈の U+0085・U+2028・U+2029（dandori の DESIGN 7 章）。
- 秘密の値、外へ出すデータの境界、平文の通信の検査：作った（7.12、DESIGN 16 章）。

決めたこと（2026-10-06）：

- パッケージの依存は「ちょうど」で書く（9.3）。
- Dependabot と Renovate は入れない。osv-scanner の結果を SARIF で code scanning に上げることもしない（3.6）。
- `SECURITY.md` を置く（3.6）。GitHub の private vulnerability reporting は、push のときにリポジトリの設定で有効にする。
- rulec が含む Unicode CLDR のデータは、`license` に書き、配るものに通知を入れる。koyomi が含む WHATWG の表と、バイナリが含む外のクレートと、`explain` の例のための法令のコピーも同じに扱う（2.3、13.2）。同じ日に作った。式は `(MIT OR Apache-2.0) AND Unicode-3.0`（rulec）、`(MIT OR Apache-2.0) AND BSD-3-Clause`（koyomi）、三つを合わせたもの（ritsu、ritsu-wasm、`.deb`・`.rpm`、formula）。
- 次のリリース（0.23.0 の次）で、アーカイブ、`.deb`・`.rpm`、formula に `THIRD_PARTY_NOTICES` が入ったことを実物で確かめる（`release.yml` の homebrew のジョブが `brew audit --strict --online`、install、test を通す）。rulec の入れ方のページと README の「0.23.0 より後のリリース」の言い方は、そのとき実物の出力を取り直すのに合わせて直す。
- 依存を上げたら（`cargo update` など）、`THIRD_PARTY_NOTICES` の版と節を直す。`release.rs` が、どの名前・バージョン・行が違うかを言って落ちる。
- OpenSpec の決め方（仕様の要件を固定していなければ W102 の警告、シナリオと主張は書いたとおりの名前で突き合わせる）と、sakai の決め方（契約の文書かどうかを中身で決める）は、このままにする。

- OpenAPI と AsyncAPI の文書の要素を、ツール名 `openapi`・`asyncapi` で指せるようにする（上の sakai の残したものの 1）。同じ日に作った（7.12）。
- 文書と診断の「名指し」を、普通の語（参照、参照の書き方、指す、ツール名）に置き換える。

### 7.12 認可とセキュリティの検査（2026-10-06）

2026-10-06 に、認可を二つの形で ritsu に入れると決めた。標準の Cedar のポリシーとスキーマを ritsu が読むことと、Cedar を生成する八つ目の言語 sekisho（`.gate`。rulec の規則と koyomi の日付を許可の条件に使う）である。あわせて、ritsu を使う人のための言語のセキュリティの検査（DESIGN 16 章）を作る。この節に、担当ごとの記録を足していく。

**Cedar の読み手と書き手**（cedar-reader の担当）

認可を ritsu に入れる二つの形（Cedar を読む確かめと、sekisho が書く Cedar）の土台として、`ritsu_base::cedar` を作った（DESIGN 4.18）。ポリシー（`.cedar`）とスキーマ（人が読む形と JSON の形）を読み、ポリシーの JSON の形、`cedar format`、書き手（`write_policies`）、スキーマの二つの形を書く。どれも公式の CLI 4.13.0 の出力と一字も違わないことを、`crates/ritsu-base/tests/cedar.rs` が材料（`tests/fixtures/cedar/`、作り方は `expected.sh`）で確かめる。

- 残したこと：JSON の形のポリシーとテンプレートのリンクを読むこと、エンティティとリクエストの JSON、スキーマの名前の解決（DESIGN 4.18 の「まだやっていないこと」）。言語をまたぐ確かめ（公開する操作と action、ワークフローの最小権限、yuen の要件とポリシー）は、この部品の上に別の項目として作る。
- Cedar の版を上げるときは、新しい CLI で `expected.sh` を走らせて答えを取り直し、テストが落ちたところを直す。

**言語のセキュリティの検査**（sec-base、sec-dandori、sec-langs の担当）

ritsu を使う人のプロジェクトのファイルから見える、セキュリティの誤りを言う検査を、三人の担当が並べて作った（DESIGN 16 章）。検査は五つで、コードは、どの台帳でも同じ検査を指す 9xx の帯に置いた。鍵の形の値（W901。七つの言語と ritsu）、暗号化しない通信（W902。dandori と sakai）、認証の指定の無い公開の操作（W903。sakai）、プラットフォームの履歴に残る秘密の値（W904。dandori）、秘密の値を外へ送ること（dandori の E906、ritsu の E905・W905。言語をまたぐ検査の X14）である。どれもネットワークを使わない。

- **sec-base**：土台の `ritsu_base::secrets`（鍵の形の検出と、見せる行の鍵を伏せる `mask`）・`urls`・`marks`（DESIGN 4.19）、ritsu-proto の `extend` と `Protos::redaction`、口の型（`Flows::sends` の型と空を返す既定の実装、新しい口 `Maps`）と `Joined::maps`。sakai の W901・W902・W903 と、`Maps` の答え（`tests/maps.rs`）。webshop の例の三つの OpenAPI の文書に、ベアラートークンの方式とルートの `security` を足した。sakai の書き出し（CML、`build` の設定の頭、`doc` の Markdown）に入る `.ctx` の文字列と地図のパスを、コメントや行の中に収めた。ritsu-emit の `one_line` が行を終える文字を `U+XXXX` で書き、rulec の Java の頭を `for_unicode_comment` に通すようにした（DESIGN 9.2）。sakai の DESIGN 16 章、README、スキル、`docs/codes.md`。
- **sec-dandori**：`secret`・`plaintext`・`discloses`・`history encrypted` の構文（予約語の表には足さない）、印の読み取り（`.flow` の `secret`、`.proto` の `debug_redact`、OpenAPI の三つ）と、変数ごとのたどり方、W901・W902・W904・E906、E007 と Argo の E050。`history encrypted` のフローでは、Temporal の TypeScript・Python・Go の生成コードが、ペイロードのコーデックを型で要るようにし、失敗の文とスタックトレースも符号化する（宣言の無いフローの生成物は一バイトも変えない）。Temporal の dev server で、コーデックの無いクライアントで読んだ履歴に、入力の値も失敗の文も平文で残らないことを、三つの SDK で確かめた。`Flows::sends` の答え。例 `examples/payout`（英語と日本語。どのプラットフォームでも、シナリオを参照インタプリタと突き合わせた）と、問い合わせの例の Temporal 版の `plaintext`。dandori の DESIGN 1.18、README、サイトの「Secrets」のページと診断の表、スキル。
- **sec-langs**：rulec・koyomi・chobo・geas・yuen の W901 と、各言語の DESIGN・README・スキル・`docs/codes.md`。ritsu-cross の契約の文書の W901（`src/secrets.rs`）と X14（E905・W905。`src/egress.rs`）、台帳の再現、`skills/ritsu` のコードの表。chobo・geas・rulec が診断に引く行の鍵を伏せること。geas の `check --json` がいつも `diagnostics` を出すこと。yuen の書き出しと geas の下書きのテスト（直すところは無かった）と、`dandori doc` と `rulec doc` の Markdown の `<` のエスケープ（DESIGN 16 章の頭）。

突き合わせ：替える前と後のバイナリを、例とテストの材料の全部にかけ、変わったのが、新しい材料と例と、意図して直したもの（webshop の W903 が消えたこと、問い合わせの例に `plaintext` を書き足したこと、W902 が出る dandori の三つの材料）だけであることを確かめた（DESIGN 16.10）。dandori の重いテストは、例 `payout` のシナリオを七つのプラットフォームと LocalStack で参照インタプリタと突き合わせるものと、暗号化した履歴を Temporal で確かめるものを一つずつ回し、どれも SKIP なしで通った。

残したこと：

- DESIGN 16.12 の項目。
- ritsu のサイトの頭のページ（`website/docs/index.md` と `website/docs-ja/index.md`）に、根の README の「Security checks」（「セキュリティの検査」）の節を足すこと。サイトの頭のページは README の節を並べた形だが、この節はまだ無い。
- rulec の名前に `\` があると、ASCII の別名の無い名前は生成物の識別子にそのまま入り、どの出力先でも生成物が通らない（止まるほうに倒れるので、外へ出る穴ではない）。`\` を名前に書けなくする（字句の決まりと診断）か、ASCII の別名を求めるかを決める。TypeScript の、列挙に無い値のエラーの文（テンプレートリテラル）も同じ決めで直す。
- HTML を開きうる `<` のエスケープ（`md_prose`）が、dandori・rulec・sakai の三つにある。ritsu-base の `docpage` に一つにまとめる。
- dandori の生成物で前からあった問題：サービスを実装するフローの Python が `mypy --strict` を通らない（`start` に TypedDict を渡すところ）。Go のサービスのクライアントが、`RPC` を付けた名前がすでにあるかを確かめない（`go vet` が redeclared と言う）。同じサービスにメソッドとメッセージで同じ名前があると、protoc は止めるが ritsu-proto は通す。どれも、直すと暗号化の無いフローの生成物が変わる。

決めたこと：

- `Maps` は、sakai の検査の段 1・2（構文、名前、パスと、属し方）だけで答える。段 3 から後は、どのファイルがどのコンテキストに属するかを変えない（DESIGN 16.9）。
- webshop の例で `security: []` を付けたのは、受注の `createOrder`（アカウントの無い客の注文）。注文の状態を ID だけでだれでも読める形は、例として勧めにくい（DESIGN 16.11）。
- 土台の `Diag::source` が見せる行の鍵は、`secrets::mask` で伏せる。chobo・geas・rulec・dandori も、自分の診断の型で同じにした（DESIGN 16.3）。
- W901 は、鍵のある行を引用しない。どの診断でも原文の行を引く dandori は、伏せた行を引く（DESIGN 16.3）。
- 生成物の頭は、行を終える五つの文字を `U+XXXX` で書く。rulec の Java は、頭と本文のコメントのバックスラッシュを二つにし（`for_unicode_comment`）、記録の JSON のキーを JSON の文字列から作る（DESIGN 9.2）。
- Markdown のエスケープは、HTML を開きうる `<` だけにした。`<=60cm`、`R&D` のような文を、元の文のまま読めるようにするためである（DESIGN 16 章の頭）。
- geas の `check --json` は、`diagnostics` をいつも出す（鍵の無い仕様では `[]`）。`snap` は鍵を調べないので出さない。
- geas の `.geas/` の journal とベースラインには、鍵が残る（実行したことの記録なので伏せない）。
- `connect` のタスクの引数と結果にも、`.proto` の印が効く（DESIGN 16.6）。
- `Flows::sends` は、dandori の検査を通るフローにだけ答える（DESIGN 16.9）。
- `history encrypted` で生成コードが増やす名前（Go の `EncryptedClient`、`Dial`、`CodecDataConverter`、`CodecFailureConverter` など）が、サービスのメソッドやメッセージ、Go の型の名前と重なるときは、E006 で止めず、生成する側で名前を変える（TypeScript は `Rpc`、Python は `_rpc`、Go は `RPC` か `_` を付ける）。生成する名前のほうが避けるという、dandori のもとからの決まり（dandori の DESIGN 1.14、1.15）に合わせた。`.proto` の名前は利用者が変えられないことが多く、E006 にすると、そのサービスでは暗号化を宣言できなくなる。
- ブラウザで試すページの例に `payout` を足し、`projects.json` と `ritsu.wasm` を作り直した。
- sakai の参照の口は、公表された言語の `openapi "…"`・`asyncapi "…"` の文書も返す。ritsu-cross の W901 が、地図だけが読む文書の鍵も言う（DESIGN 16.3）。
- `.github/secret_scanning.yml` を置き、`paths-ignore` で、偽の鍵を書いた材料と、生成する codes のページを外す（DESIGN 16.10）。
- 契約の文書の W901 は、`ritsu check` だけに出す（DESIGN 16.1）。E905 と E906 はエラーにする（DESIGN 16.2）。

**sekisho の段階 A**（sekisho-a1、sekisho-a2、sekisho-a3 の担当）

八つ目の言語 sekisho の芯を、三人の担当が並べて作った（設計は `crates/sekisho/DESIGN.md`）。`ritsu sekisho check` が例の二つの版（`crates/sekisho/examples/refunds/` の `refunds.gate` と `refunds.ja.gate`）を通し、変異の全部が設計のコードを出す。Cedar はまだ生成しない（段階 B）。

- **sekisho-a1（構文と名前）**：クレート `crates/sekisho` を足し、字句と構文、名前と型（E001〜E008、E101〜E108、E201、E209〜E211）、診断の台帳、`check` と `explain` を作った。`ritsu sekisho` の入口と、`ritsu check` が `.gate` を sekisho に渡すこと（`ritsu-project` の `ORDER` の dandori のあと、`ritsu_base::naming` の `Tool::Sekisho`、地図の `contexts/gates.ctx`）を先に入れた。`ritsu --help` の言語の一覧、リンク、リリースは D3 で足す。`cargo xtask deps` の言語に sekisho を足した。sekisho の DESIGN の 2.10、9〜12 章、15 章、16 章を書いた。
- **sekisho-a2（口とほかの言語）**：ritsu-ports に、口 `Gates` の型と、sekisho が読む口のまとまり `GatePorts` を足した（DESIGN 3.2）。rulec は `Rules::outputs_over` に答え（rulec の DESIGN 15.188。コーパスとテストの規則の 323 の問いを総当たりと突き合わせ、322 が正確に答え、わざと作った一つは決められないと答えた）、口の事実の列挙の値に公開名（`EnumValue::public`）を持たせた。koyomi は `Dates::calendar` と `doc` に答える。土台に `ritsu_base::openapi`（DESIGN 4.20）を、`ritsu-project` に `Joined::sekisho()` を置いた。koyomi の口が検査したファイルを覚えるようにした（DESIGN 3.2。例の `.gate` の検査が 1.8 秒ほどから 0.46 秒ほどになった）。
- **sekisho-a3（検査）**：有限のモデルと、定数で区間に切ること、参照の評価（Cedar の意味）、組み合わせの数え方（rulec の `outputs_over` と koyomi の `eval`・`calendar` に尋ねる。予算と E307）を作った。その上に、E301〜E307 と W301〜W304、契約と境目の検査（E202〜E208、カレンダーと帳簿の E201）、行を合わせた表を置いた。例の組み合わせは 1,078 通り（`view_order` 18、`refund_order` 1,056、`export_refunds` 4）で、`refund_order` の表は 26 行（許す行が 5 行）になる。

取り込み：三人のパッチを一つの worktree に当て、ぶつかったところを直して、全体のテストを一回回した。sekisho のテストは 63 件（lib 15、`cli` 6、`codes` 4、`contracts` 7、`design` 1、`mutants` 3、`names` 13、`parse` 2、`walk` 12）。台帳のコードは 40、変異は英語と日本語の対で 112。全体は 2,219 件で、落ちたものは 0。

```
$ ritsu check crates/sekisho/examples/refunds
ritsu check: 8 files (rulec 2, koyomi 3, dandori 1, sekisho 2): all pass; borders between the languages: 0 checked, 0 undecided
$ ritsu check ritsu.ctx
ritsu.ctx: ok — 13 contexts, 30 relationships; 473 artifacts, each in one context; 66 crossings checked (rust 66)
```

残したこと：

- 段階 C と D（TypeScript・Python・Go のリクエストを組み立てるコード、`sekisho doc`、X15 と X16、八つ目の言語としての取り込み）。段階 B は、下の「sekisho の段階 B」で済んだ。
- W201 の再現。いまは範囲に端の無い値を規則に渡せない（E103）ので、W201 は出ない。安全網として残し、台帳の再現は「まだ無い」のままにした（sekisho の DESIGN 3.2）。
- （済み）rulec の E102 を、導出の取りうる値で強める。2026-10-06 に、E102 の三つ目の形として入れた（rulec の DESIGN 15.189。下の「rulec の E102 の三つ目の形」）。`crates/rulec/tests/over/reach.rule` の行は E102 になった。コーパスの検査を通る 87 の規則では、`rulec check` の出力（英語、日本語、JSON）が一字も変わらなかった。木にあるほかの `.rule` で変わったのは、`reach.rule` と変異 `m_e019.rule`（足した `constraint` で導出が 0 円以下にしかならず、行に当たる入力が無い）の二つだった。
- 本物の Verified Permissions で確かめるか（決めていない。sekisho の DESIGN 15 章）。
- dandori と sakai の OpenAPI の読み手を `ritsu_base::openapi` に替えること（出力が変わらないことの突き合わせと一緒に）。
- 参照の書き方のツール名 `sekisho`（`Tool::ALL`、`naming.tsv`、yuen と sakai の文）。`Tool::Sekisho` はいま `Tool::ALL` の外にあり、拡張子と種類だけを持つ。

決めたこと：

- 名前にも別名にもできない語は、条件と計算と型を書く 16 語だけにする。E008 の生成先の予約語は、TypeScript の予約語と strict モードの予約語、Python のキーワード、Go のキーワードにする。`use gate` で読み合うファイルは名前空間をそろえ（E210）、読んだ二つのファイルは同じ型・列挙・役割・ワークフローを宣言しない（E211）。役割の `includes` の輪は E108（sekisho の DESIGN 16.1 の 15〜18）。
- 口のまとまり `GatePorts` は ritsu-ports に置き、sekisho の `Suite` は `From<GatePorts>` で作る。`.flow` が dandori の検査を通るか（E208）は、`Flows::crossings` で尋ねる（dandori の `Items` は検査をせずに読む）。
- Cedar に渡す規則の列挙の値は、`.rule` に書いた別名（`EnumValue::public`）にする。
- 役割の組は、action ごとに、そのポリシーと期待が読む役割だけを数える（例は 1,078 通り）。`nobody` を書いた action をだれかが許されれば E304。E203 の範囲の向きは dandori の E016 と同じ。値が無いとき、`x is not v` は成り立つ。何も選ばない期待は W304（警告）。
- koyomi の口が覚える検査はスレッドごとに置き、`Engine` は値を持たない型のままにした（ほかのクレートの三十を超える所が値で書いている）。
- 表は、規則の値を `.rule` に書いた名前で出す（`.gate` と並べて読むため）。Cedar に渡す文字列（別名）は、ページのポリシーのところに出る（sekisho の DESIGN 7 章）。
- 名前の検査は、規則の値を、`.rule` に書いた名前、生成したコードの名前、`.rule` の別名のどれでも引く（sekisho の DESIGN 3.2）。
- 二人の担当が同じ名前で作った E201 の変異（rulec の検査を通らない規則を読むもの。日本語の版の名前が重なった）は両方残し、返金の例を元にしたほうを `E201_通らない規則を読む返金` にした（sekisho の DESIGN 4.3）。

**sekisho の段階 B**（sekisho-b1、sekisho-b2 の担当）

Cedar の生成と、公式の CLI との突き合わせを、二人の担当が並べて作った（sekisho の DESIGN 5 章、6.1、6.2、11 章、16.1 の 27〜46）。

- **sekisho-b1（生成）**：`sekisho gen --target cedar` が、`.gate` ごとに Cedar のスキーマとポリシーとその JSON の形の四つのファイルを書く（`<out>/cedar/<別名>.*`）。`sekisho vectors` は全部の組み合わせを `cedar run-tests` のテストにし（例は 1,078 通りから 2,134 件）、`sekisho api` は宣言を JSON で出す。例の生成物は、設計の担当が手で書いた見本と、頭の二行のほかは同じになった。突き合わせで見つかった段階 A の数え方の食い違い二つ（関係の項の綴り、規則の値の生成したコードの名前）を直した。
- **sekisho-b2（突き合わせ）**：生成した Cedar を公式の CLI 4.13.0 にかけるテスト（`crates/sekisho/tests/cedar.rs`）、ritsu-testkit の `Need::Cedar` と `cedar::cli()`、`tools.yml` の CLI を入れる段を作った。検査を通る材料の全部（131 のうち 27）で、`validate`（strict、Cedar と JSON の形）、`format --check`、`translate-*` の一致、`run-tests`（全部のテスト、Cedar と JSON の形）、ポリシーを一つずつ流す確かめ、要る属性を抜いたテストが通る。生成したテキストを一か所ずつ変えた変異は 465 で、全部がどれかの確かめで落ちる（sekisho の DESIGN 6.1、6.2）。`run-tests` が決めたポリシーを含まれるかでしか比べないことを、CLI のソースで確かめ、ポリシーを一つずつ流す確かめで補った。
- **守る操作の参照**（取り込み）：action が守る操作を、診断の文（E202〜E205）、`sekisho api` の `guards`、生成するスキーマの `@guards` で、参照の書き方で言うようにした（sekisho の DESIGN 2.6）。参照は契約の検査が操作を見つけたときに `ritsu_base::naming::Name` で組み（`openapi "…" operation <operationId>`、`asyncapi "…" operation <キー>`、`proto "…" service S method M`、`chobo "…" transfer T operation O`）、パスは `.gate` の `use` に書いたパスではなくルートからにした。ルートは yuen と sakai と同じに決め、`check`・`gen`・`vectors`・`api` が `--root` を取り、`ritsu check` はプロジェクトのルートを渡す。守る契約がルートの外にあれば E201。`api` の `guards` は sakai の `api` と同じ参照の JSON で、段階 D の口 `Gates` の `GateAction::guards` も同じ値（`Action::references`）を入れる。E202 は yuen の E202 と同じく、文書のパスと書いた組で無いものを言う（`There is no operation refundOrders in examples/refunds/api/orders.json`）。例の `@guards` は、例のディレクトリをルートにすると前と同じ `openapi "api/orders.json" operation refundOrder` である。
- **CI**：`tools.yml` の「それ以外」の組で、同じ CLI で ritsu-base の Cedar の材料を `expected.sh` で作り直し、一字も変わらないことを確かめる段を足した（DESIGN 4.18、10.5）。足す前に手元で同じ CLI で走らせ、材料の 586 のファイルが一字も変わらないことを確かめた。

取り込み：二人のパッチと、参照の書き方のツール名（下）のパッチを一つの木に当て、関わるテストを回した（クレートの 781 件、dandori の `ports` 6 件、ritsu の 45 件が通り、SKIP は 0）。そのあと守る操作の参照を入れ、sekisho の全部（77 件。公式の CLI を使う 2 件を含む）と、ritsu の `sekisho`・`check`・`cross`・`yuen`・`sakai`（24 件）が、SKIP なしで通った。

```
$ ritsu check crates/sekisho/examples/refunds
ritsu check: 8 files (rulec 2, koyomi 3, dandori 1, sekisho 2): all pass; borders between the languages: 0 checked, 0 undecided
$ ritsu check ritsu.ctx
ritsu.ctx: ok — 13 contexts, 30 relationships; 486 artifacts, each in one context; 66 crossings checked (rust 66)
```

ritsu 自身の地図の成果物は、段階 A の 473 から、参照の書き方のツール名で 481（例の文書が六つと、`ritsu-base/src/document.rs`、`yuen/src/documents.rs`）、段階 B の新しい五つの `.rs`（sekisho の `api.rs`・`cedar.rs`・`gen.rs`・`vectors.rs`、ritsu-testkit の `cedar.rs`）で 486 になった。

残したこと：

- 計算した値とワークフローの `@doc` の参照（`rulec "…" output …`、`dandori "…"`）は、まだ `use` と `workflow` の行に書いたパスで書く。`.gate` が読むファイルの参照を口 `References` で出す段階 D で、ルートからのパスにする（ルートの外の規則や日付のファイルをどうするかも、そのときに決める。sekisho の DESIGN 5.1）。
- `tools.yml` の Cedar の二つの段を GitHub で走らせること（push してから）。

決めたこと：

- `.cedarschema` は、CLI の `translate-schema --direction json-to-cedar` の形（名前の順、`\'`）のままにする。土台の書き手が CLI と一字も違わないことを確かめているためである（sekisho の DESIGN 16.1 の 27）。
- `vectors` は、区間の両端の二件にする（例は 1,078 通りの 2,134 件。33）。
- 生成物の文（頭の二行目と `@doc`）は `--lang` の言語で書き、koyomi とそろえる（35）。`--target` は省けない（36）。計算した値の `@doc` の最後の文は、どのコマンドが書くかを言わない形にする（46）。
- 決めたポリシーを一つずつ流す確かめと、要る属性を一つ抜いたテストの確かめを持つ（41、42）。CLI を使うテストは二つで、CLI が無いときの SKIP の行はテストごとに一つ、合わせて二つ（44）。変異のテストは材料を絞らない（43）。
- 守る操作は参照の書き方で言い、パスはルートからにする。ルートは yuen と sakai と同じに決め、守る契約がルートの外にあれば E201 にする（39、40）。
- Verified Permissions の `CreatePolicy` の `definition.static.statement` に置くのは、`.cedar` のポリシーを一つずつ書いたテキスト（`ritsu_base::cedar::write_policy`）で、`.policies.json` は三つの実装（cedar-wasm、cedarpy、cedar-go）に渡すものにする。API の文書（CreatePolicy、StaticPolicyDefinition、GetPolicy）で確かめた（sekisho の DESIGN 5.8）。
- CI で、ritsu-base の Cedar の材料を同じ CLI で作り直して確かめる（DESIGN 4.18）。
- rulec のサイトの相対リンクのテスト（`crates/rulec/tests/website.rs` の `サイトの相対リンクは実在する`）は、日本語のページの画像を、`sync.sh` のコピー元である英語のページの `images/` で確かめる。`sync.sh` を走らせていない木でも通るようにするためである（前は、`website/rulec/docs-ja/index.md` が指す `images/overview-ja.svg` が無いと落ちた）。

**参照の書き方のツール名**（tool-names の担当）

参照の書き方（DESIGN 6.2、6.5）に、ツール名 `openapi`・`asyncapi`・`cedar` と、chobo の振替の下の `operation` を足した。土台の `naming` と新しい `document`（DESIGN 4.21）、`naming.tsv` の 24 行。chobo の口の `Items` は振替の操作も渡す。yuen は三つのツール名の要素をリンクの端にし、要素ごとのハッシュで固定する（yuen の DESIGN 3.6。英語の材料 `refund_contracts` と日本語の `contracts`、変異の対 `E303_element_changed`・`E303_要素が変わった`）。sakai は文書と要素を、api、診断、doc のページで参照の書き方で書き、`.ctx` の長い書き方を読む（sakai の DESIGN 15.10）。dandori の `use openapi` の参照も `openapi "…"` になった。

- 残したこと：ツール名 `sekisho`（段階 D、口 `Items` と一緒に）。Cedar の要素の引き方を土台に移すこと（sekisho が手で書いた Cedar に答える段階 D で）。yuen の `affected` が、文書と Cedar のファイルを要素の単位で答えること（いまはファイルの単位）。sekisho の `guards` の診断の文を参照の書き方にすることは、上の「守る操作の参照」で済んだ。
- 決めたこと：`pointer` の種類、asyncapi の `schema` の下の `value`、`message` を二つの場所に置くこと、Cedar の名前に名前空間を付けないこと（DESIGN 6.5）。yuen の端に文書の値（`description` も値として入る。`.proto` のコメントは値ではない）と `$ref` の先を入れること、E205 を広げたこと（yuen の DESIGN 3.6）。sakai の api の形を変えたこと（`pointer`・`pointers`・`contract` のキーを無くした。sakai の DESIGN 15.10）。まだリリースしていない形なので、前の形は残さない。sakai の `.ctx` の予約語に `cedar`・`property`・`pointer`・`policy`・`action`・`entity` の六つを足したこと。dandori の `use openapi` の参照を `openapi "…"` にしたこと（`use smithy` は `file` のまま）。

**rulec の E102 の三つ目の形**（rulec-e102 の担当）

導出の列に、その導出が入力の範囲から実際に取りうる値の外だけを求める行を、E102（どの入力にも当てはまらない行）の三つ目の形にした（rulec の DESIGN 15.189）。

次のリリースノートに書くこと：derive の範囲を入力から計算して、届かない行を E102 にした。derive の範囲を広めに書き、その先に行を書いていた規則は、新しく E102 で落ちる。`policy first` の表で、上の行が derive の取りうる値をすべて覆ったあとに置いた受け皿の行（`-` の行）と、`constraint` で derive の取りうる値が狭まり、その外を求める行も同じく落ちる。対象は、入力の足し算・引き算・定数倍でできた derive の列で、`define` の列と、分数の定数を掛ける derive の列は、これまでどおり見ない。

- 残したこと：分数の定数を掛ける導出（`amount * 10%`）を軸に持つ表で、軸が金額の刻みで切られ、値の刻み（0.1 ポンドなど）のあいだの値を数えないため、E101 が穴を見逃す（生成したコードは、その値で完全性の assert に止まる）。三つ目の形は、この軸を持つ表を読まない。
