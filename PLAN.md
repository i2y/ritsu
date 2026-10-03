# ritsu 実装計画

DESIGN.md を仕様として、ritsu を五つの段階（B〜F）で作る。どの段階も、ここに書いた順に進め、各段階の最後にある完了の条件が全部成り立ったら終わりにする。DESIGN.md と違うことをしたくなったら、先に DESIGN.md に決定と理由と捨てたものを書き、報告で言う。

この計画は段階 A（設計）で書いた。コードは書いていない。DESIGN.md の 1 章の数と 12.5 の試しは、A の段階に作業場所で測ったもので、試しに使った写しは残していない。

| 段階 | すること | 主に読む DESIGN の章 |
|---|---|---|
| B | `~/ritsu` を作り、七つの履歴を取り込み、中身を直さずに全部のテストを通す | 2、12 |
| C | 重なっているものを土台へ移す。koyomi・chobo・geas・yuen・sakai から先に、rulec と dandori は合うところだけ | 4、9.2、10 |
| D | dandori と rulec のつなぎを型付きの呼び出しに替え、yuen と sakai の一式の読み込みを型付きで作り、単位の型を一つにする | 3、5、6、7.3、7.10〜7.12 |
| E | 言語をまたぐ検査、`ritsu check`、一つの生成パッケージ | 6、7、8、9.3 |
| F | yuen と sakai の段階 D、ritsu の README・DESIGN・スキル、LSP、ブラウザのページ、Lean の層、リリースの準備 | 8、11、13 |

## 0. 全部の段階に共通の決まり

### 0.1 守ること

- 作者の決まり（段階ごとの指示書が挙げるもの）を先に読み、従う。日本語（DESIGN.md、PLAN.md、`--lang ja` の診断、README.ja.md、報告）は、英語の概念語を漢字に直訳しない。カタカナ英語が普通の語はカタカナで書く（ツール、バージョン、イベント、リトライ、タイムアウトなど）。
- git のコミットと push をしない。例外は B の取り込みのマージ（B.2）で、それも指示する側が許したときだけ行う。
- 元のリポジトリ（`~/rulec`、`~/dandori`、`~/koyomi`、`~/chobo`、`~/geas`、`~/yurai`（yuen の前の名前のまま）、`~/sakai`）に書かない、そこでビルドしない、git の状態を変えない。履歴や古いバージョンが要るときは、作業場所に `git clone --no-local` で写して使う。ほかの木（`~/pixie` など）も同じ。
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
- `naming.tsv`（36 行）は、C から `crates/ritsu-base/tests/fixtures/naming.tsv` にあり、yuen と sakai の写しは消す。表を直すときは、この一つを直す。
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
  crates/ritsu-wasm/  proofs/  skills/ritsu/  README.md  README.ja.md                   （F）
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

1. 最初のコミット：`LICENSE-MIT`、`LICENSE-APACHE`（dandori のものを写した。著作権者は同じ）、`DESIGN.md`、`PLAN.md`（A の段階の二つ）、`.gitignore`（`/target`）、ワークスペースの根の `Cargo.toml`、`Cargo.lock`、`tools/import.sh`（B.3）。題は `build: start ritsu, one workspace for seven small languages`。
2. 七つのマージ。題は `chore: bring <名前> into the workspace with its history`。
3. yuen の二つのテストの直し（B.7 の (i) と (ii) の間をとったもの）：`tests/cli.rs` の `the_json_of_check` と、DESIGN.md の `$ yuen check tests/mutants/E302_条が変わった` とその直し方の三行に `--root .` を足した。題は `test(yurai): pass --root . where the root was taken to be the crate`。回り道の表示は C.0 に残した。

まず作業場所の写し（`<作業場所>/ritsu-try`）で、全部を通して走らせる。そこで次を確かめる。

- コミットの数が、七つの元のコミットの数（355、52、2、1、6、1、1 で 418。B.1 で増えていれば足す）に、マージの七つと、最初と最後のコミットの二つを足した数（427）になる（実際に 427 になった）。
- タグが 29（`rulec/v0.1.0`〜`rulec/v0.22.1` の 28 と `dandori/v0.1.0`）で、根の名前空間に `v0.1.0` などが無い。
- 各クレートの、元の HEAD で追っているファイルと、取り込んだ HEAD の `crates/<名前>/` の下のファイルが、同じ並びで同じ blob のハッシュを持つ（`git -C ~/<名前> ls-files -s` と `git ls-tree -r HEAD crates/<名前>`）。
- 各クレートで三つのファイルについて、`git log --format=%h -- crates/<名前>/<パス> | wc -l` が、元のリポジトリの `git log --format=%h -- <パス> | wc -l` と同じ（`--follow` なしで）。
- 作者と日時が元のまま（`git log --format='%an %ae %ad'` の並びが元と同じ）。

`~/ritsu` に対しては、指示する側が許したときに、同じスクリプトを走らせる（作者の決まりで、コミットは勤務時間の外に、実際の時刻で行う。日時を書き換えない）。許しが出るまでは、作業場所の写しで B.3〜B.6 を済ませておく。

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

gitignore したもの（各クレートの `tools/` の `node_modules` と venv、Java の jar、TigerBeetle、`go-arch-lint`、ReqIF のスキーマ、rulec の `website/` の写し、dandori の `website/docs-ja/` の写し、rulec の `proofs/.lake`）は、取り込んでも来ない。各クレートの README、PLAN、`tools/` の README と `requirements.txt` の頭に書いてある手順で、`crates/<名前>/` の下に入れ直す。元のリポジトリに入っているものを指して使わない（元の木の venv を走らせると `__pycache__` を書くことがある）。

| クレート | 入れるもの |
|---|---|
| rulec | `website/sync.sh`（CI と同じく、テストの前に）、`proofs/` で `lake build`、Connect・mypy・NumPy・ruff の入った venv（rulec の CI の一覧）、JDK、使い捨ての PostgreSQL（`PGHOST`・`PGPORT`・`PGDATABASE` で渡す）、buf、node、ruby（rbs と steep）、php、go、swiftc。何を入れるかの正は `crates/rulec/.github/workflows/ci.yml` |
| dandori | `npm install --prefix` で `tools`、`tools/temporal`、`tools/durable`、`tools/agents`、`tools/wire`、`tools/mermaid`。venv は `tools/temporal-python`、`tools/pydantic-graph`、`tools/agents`、`tools/wire`、`tools/connect`（作り方は各 `requirements.txt` の頭）。`tools/temporal-go` で `go mod download`。kind のクラスタ（`tools/argo/setup.sh`。この機械にあれば使う）と argo CLI、LocalStack 4.14.0 のイメージ、Chrome。rulec は 0.22.0（B.7） |
| koyomi | `npm ci --prefix tools`（tsc）、`tools/.venv`（mypy）、PostgreSQL のバイナリ（`KOYOMI_PG_BIN`、`KOYOMI_PG_SOCKET_DIR`）、go、rustc、Chrome |
| chobo | `npm ci --prefix tools/runner` と `tools/runner/.venv`、`npm ci --prefix tools/mermaid`、`tools/tigerbeetle/fetch.sh`、PostgreSQL のバイナリ（`CHOBO_PG_BIN`、`CHOBO_PG_SOCKET_DIR`）、go、`TMPDIR` を作業場所に |
| geas | `GEAS_PIXIE_GREETER`（pixie の木ではビルドしない。場所は指示書で渡す）、Chrome、LLVM のツール（`GEAS_LLVM_BIN`）、go、node、python3 |
| yuen | `tools/.venv`（`tools/requirements.txt`、`YUEN_PYTHON`）、`tools/reqif/fetch.sh`（`YUEN_REQIF_XSD`）、xmllint |
| sakai | `tools/.venv`（import-linter）、`npm ci --prefix tools`（dependency-cruiser）、`tools/java/fetch.sh`、`tools/cml/fetch.sh`、`tools/go/install.sh`、`SAKAI_JAVA` と `SAKAI_JAVAC`、buf。`SAKAI_RULEC`・`SAKAI_KOYOMI`・`SAKAI_CHOBO`・`SAKAI_DANDORI` には、B.4 で作ったワークスペースのバイナリを渡す |

### B.6 テストを回す

- 一つのクレートずつ、そのクレートのディレクトリで回す：`cd crates/<名前> && cargo test --no-fail-fast -- --nocapture`。rulec の `.cargo/config.toml`（`RULEC_LANG=ja` を強いる）は、そのディレクトリで走らせたときだけ読まれる（DESIGN 12.4）。dandori は、ほかと同時に回さない。
- テストの並びを元と比べる：各元のリポジトリを作業場所に写し（`git clone --no-local`）、`cargo test --target-dir <作業場所>/target-base -- --list` で、走らせずにテストの名前の一覧を取る。取り込んだクレートの `cargo test -- --list` と、同じでなければならない。
- 落ちたテストがあれば、作業場所の元の写しで同じテストを走らせ、元でも落ちるか（揺れか、元からか）、取り込んだせいかを分ける。dandori の Argo と Temporal の揺れは、DESIGN 10.6 のとおり一度だけ回し直してよく、回し直したことを報告に書く。
- 各クレートの通った数、SKIP の行と理由、かかった時間を報告に書く。

### B.7 分かっている食い違い

A の段階に測ったもの（DESIGN 12.4、12.5）。

- **yuen の二つのテスト**：`tests/cli.rs` の `the_json_of_check` と、`tests/design.rs` の `every_command_in_design_prints_what_design_shows` は、git のルートが `~/ritsu` に移るだけで落ちる。扱いは二つのどちらかで、B を始める前に指示する側が選ぶ。
  - (i) B の中で、テストと文書だけを最小に直す：`tests/cli.rs` の `the_json_of_check` の呼び出しに `--root .` を足し、DESIGN.md の `$ yuen check tests/mutants/E302_条が変わった`、`$ yuen check tests/fixtures/period`、`$ yuen check tests/fixtures/period --lang ja` に `--root .` を足し、E302 の直し方の三行（`yuen review tests/mutants/E302_条が変わった --root . --at …`）も合わせる。出力は元と同じになる（A の段階で確かめた）。ツールのコードは直さない。
  - (ii) B では直さず、二つを「ルートが移ったために落ちる」と報告し、C.0 で直す（表示のパスの回り道も一緒に）。
  勧めは (ii) で、B の「中身を直さない」を字のとおりに保ち、落ちる理由が取り込みにあることを報告で示す。
- **sakai の `tests/cli.rs` の `check_exit_codes_and_formats`**：通るが、`--root` なしの形を確かめる部分を、クレートのディレクトリに `.git` が無いので黙って飛ばす。報告に書き、C.0 で直す。
- **dandori が使う rulec**：dandori の golden は rulec 0.22.0 で取ってある。ワークスペースの rulec（0.22.1）ではなく、0.22.0 を `DANDORI_RULEC` で渡す。作り方は、作業場所に rulec を `git clone --no-local` で写して `v0.22.0` を `cargo build --release --target-dir <作業場所>/…` で作るか、GitHub のリリースのバイナリを取る。
- **rulec と dandori のサイトの写し**：rulec は `website/sync.sh`、dandori は `website/build.sh` が写すページを、テストの前に作る（それぞれの CI と同じ）。

### B.8 B の完了の条件

1. `~/ritsu` に、七つの履歴が `crates/<名前>/` の下に、元の作者と日時のまま入っている（B.2 の確かめが全部通る）。タグは 29 で、どれもツールの名前で始まる。
2. 各クレートの、HEAD で追っているファイルが、元の HEAD と同じ blob のハッシュを持つ（取り込んだクレートの中身を一字も直していない）。B.7 で (i) を選んだときは、直した二つのファイル（yuen の `tests/cli.rs` と `DESIGN.md`）だけが違い、その差分を報告に貼る。
3. `cargo build --workspace --locked --offline` が通る。
4. 各クレートの `cargo test -- --list` が、元のものと同じ。
5. 各クレートのテストが、そのディレクトリで全部通る。SKIP は 0。ただし、次は許す：`TYPESAFE_API_KEY` を空にしたときの Jev の SKIP、Ollama が無いときの SKIP（どちらも dandori）。B.7 で (ii) を選んだときの yuen の二つは、落ちたまま報告する。
6. 立てたものが残っていない（`ps` で、Temporal の dev server、テストが立てたヘッドレスの Chrome（`--user-data-dir` が一時ディレクトリのもの）、PostgreSQL、TigerBeetle が無い。kind の上にワークフローが無い。LocalStack のコンテナが無い）。作業場所の写し（`import/`、`ritsu-try/`、`target-base/` など）を消した。
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
- 各クレートの `Cargo.toml` の `license` と `repository` は、まだワークスペースのものにしていない（F で決める）。yuen の `repository` は、改名で機械的に `https://github.com/i2y/yuen` にした。このリポジトリは作らない（配るのは ritsu だけ）ので、F で全部のクレートの `repository` を決めるときに一緒に直す。

### C.1 `ritsu-base`

DESIGN 4 章のモジュールを作る。どれも std だけで書く。C の最初の部分で作った（形と決めたことは DESIGN 4.12）。koyomi・yuen・sakai の `explain`（テキストと Markdown）、koyomi の `--help`、yuen の `review --help` の出力を `tests/golden/compat/` に写し、土台で組み直したものと一字も違わないことをテストで確かめた。`naming.tsv` は `crates/ritsu-base/tests/fixtures/naming.tsv` に写し、yuen と sakai の写しと同じバイト列であることもテストで確かめる（C.7 と C.8 で二つの写しを消すまで）。

| モジュール | 元にするもの | テスト |
|---|---|---|
| `text`（`Lang`、`Text`、`tr!`、幅と詰め） | koyomi・yuen・sakai の `src/i18n.rs` | 幅（W と F を 2）、`Lang` の選び方（`--lang`、`<名前>_LANG`、`RITSU_LANG`、英語） |
| `diag`（共通の部分と、言語ごとの部分のトレイト、テキストと JSON） | koyomi・chobo・yuen・sakai の `src/diag.rs` | 英語と日本語の golden、JSON のキーの順、`root` |
| `ledger`（`Entry`、`find`、書き出し、再現を走らせるテストの共通部分） | koyomi・yuen・sakai の `src/codes.rs` の後ろ半分 | Markdown のアンカー、関連するコードのリンク |
| `cli`（`Flag`、`Cmd`、`--help`、読み取り） | koyomi・yuen・sakai の `src/cli.rs` | 知らないフラグ、閉じた集合の外の値、値の無いフラグ、二度目のフラグがどれも exit 2 |
| `sha256` | koyomi の `src/sha256.rs` | FIPS 180-4 の既知の値（空、`abc`、長い入力） |
| `naming`、`paths` | yuen の `src/names.rs`、sakai の `src/naming.rs` と `src/paths.rs` | `naming.tsv` の 36 行が全部、yuen と sakai のいまの表と同じ結果。ルートの決め方。表示のパスがいちばん短い相対になる |
| `sources`（引用から要素の名前、写しの場所と本文、固定、固定の行の書き換え、curl、e-Gov、eCFR、base64） | rulec の `src/sources.rs` の 20〜313 行と 1047〜2142 行、koyomi の `src/fetch.rs` と `src/sources.rs`、yuen の `src/fetch.rs`・`src/copies.rs`・`src/base64.rs` | 三つのリポジトリのテストにある、要素の名前と本文の例を全部。通信は小さな HTTP サーバーで（yuen の `tests/fetch.rs` の形）。本物の e-Gov と eCFR には、`RITSU_TEST_LEVEL=platforms` のときだけ問い合わせる |
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
| C.4 koyomi | 計算の段（`Example`）、祝日の表、写しのうち本則の条だけを引くこと、`text_diff`、ページの中身と koyomi だけの色、台帳とコマンドの表の中身 | `--help` と `--lang` の説明（`RITSU_LANG`）、HTML の共通の役割の色の値、E001 の JSON の `line` と `col` が null、curl の `--compressed` と e-Gov の取り直し、写しの本文の表の読み方、固定の行の書き換えの `\"` | `docs/images/doc-{top,months}.{en,ja}.png`、`docs/reference.md` とスキルの写し | 99（C.10 で 100） |
| C.5 chobo | 操作とヒント（`Ops`）、`--help` の組み立て、台帳と `explain` の形、Markdown の頭、ページの中身 | `check --format json` の診断のキーと外側の `v`、`--lang` の説明、`doc` の `generator` と配色の変数の名前 | `tests/doc/*.html`（14）、`docs/formats.md` とスキルの写し、`skills/chobo/SKILL.md` | 63 |
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
- テスト（`tests/readers.rs`）：三つのリポジトリの `.proto` の全部（rulec 4、dandori 20、sakai 39）と、三つの読み手が自分のテストで使っていた例（ritsu-proto の `tests/fixtures/` に写した 22 のファイルと、rulec のテストの `buf.yaml` と `buf.lock` の五つ）を、三つの読み手の言葉に直して比べる。rulec の読み手とは rulec の型のまま等しいこと（`package`、`imports`、`enums`、`messages`、`buf_deps`、`buf_lock`）、dandori の読み手とは `load` が出すもの（全部のメッセージと列挙、型の解決、JSON の名前、`presence`、規則の木、サービスとメソッドのオプション、読めなかった import、型が無いときの誤り）が一行ずつ等しいことを確かめる。sakai の読み手とは、替える前に一時的に sakai を dev-dependency にして、ファイルごとの読み取りと、まとめて読んだときの import の行き先と型の解決が一字も違わないことを確かめ、それを `tests/golden/sakai.txt` に残した（いまはその golden と比べる）。rulec と dandori の分も golden に残す（D.10 で古い読み手を消したあとに比べるため）。
- 食い違いは二つだけだった。どちらも新しい読み手の方が多く読む。rulec の `package` は行の頭にしか見つけないので、`syntax = "proto3"; package a.v1;` のように一行に書いたファイルでは何も返さない（テストは、rulec が何かを返したときだけ比べる）。三つのファイル（sakai の E106 の変異、dandori のテストの `.proto` でないファイル、proto2 の `group`）は、新しい読み手が読めないと言い、rulec の読み手は読めたところまでを返す（テストは、この三つが読めないことを確かめる）。
- sakai を替えた：`src/proto.rs` は ritsu-proto の型と関数をそのまま公開するだけになり、名指しの作り方（`Naming`）と、何も設定していないことを言う列挙の値の決め方（`value_prefix`、`is_unset`）だけが sakai に残った。読み手の単体のテスト四つは ritsu-proto に移した。sakai の出力は、全部のコマンド（205 回）で替える前と一字も違わない。
- 決めたこと：言語が読んだものから作るもの（rulec の列挙の値の別名と `upper_snake`、sakai の `value_prefix`、dandori が proto2 と edition を断ること、dandori の型の解決が読んだ全部のファイルから引くこと）は、言語に残した。ritsu-proto の型の解決は sakai の形（import したファイルと `import public` の先だけを見る）で、dandori の例では結果が同じだった。


### C.10 `ritsu-emit`

生成先の言語ごとの予約語（rulec の `src/backend.rs` の `words`、koyomi の `src/reserved.rs`、dandori の `src/temporal_py.rs` と `src/temporal_go.rs`、chobo の `src/client/`）、識別子の作り方、リテラル、生成物の頭（DESIGN 9.2）。koyomi と chobo を替え、生成物が一バイトも変わらないことを、各クレートの突き合わせのテスト（koyomi の五つの出力先、chobo の七つの組み合わせ）と golden で確かめる。rulec と dandori の表は、ritsu-emit に写して突き合わせるテストだけを置き、二つのコードは C.11 で替える。

C.10 でしたこと：

- `crates/ritsu-emit`（std と `ritsu-base` だけ）を作った。`words`：標準が並べる語を、標準ごとに一つずつ（TypeScript は ECMAScript 2025 の予約語、strict mode の予約語、strict mode で名前にできない `arguments` と `eval` と大域の値の `undefined`・`NaN`・`Infinity`、Python 3.14.6 の `keyword.kwlist` と `softkwlist`、Go 1.25 のキーワードと事前宣言の識別子、Rust 1.94 のキーワード、PostgreSQL 18.0 の `kwlist.h` と PL/pgSQL の予約語）。生成器が名前を照らし合わせる語は、いくつかの表をまとめた `Words` で表し、各言語の `NAMES` が koyomi の表と同じ語になる。`copies`：rulec の 12 の出力先ごとの三つの表と、dandori の四つの表を写した。標準の表と同じものはそれを指し、違うところだけを自分の表に持つ。`ident`（`pascal`、`go_package`、`go_exported`、`is_ascii_ident`、`aside`、`unique`）、`lit`（JSON の文字列、Python の `'…'`、Go の `"…"`、SQL の `'…'` と `"…"`）、`header`（`Code generated … DO NOT EDIT.` の一行と、それを書くコメント）。
- koyomi を替えた：`src/reserved.rs` は ritsu-emit の表を出力先の名前と組にするだけになり、`pascal` と `go_package`、五つの生成器の文字列のリテラル、生成物の頭の一行とコメントが ritsu-emit のものになった。生成物が使う名前（`GENERATED`、`MODULES`）は koyomi に残した。
- chobo を替えた：Python と Go のキーワード、Go の外に見せる名前、ASCII の識別子の見分け、名前を `_2` で分けること、TypeScript・Python・Go・SQL のリテラル、Go のファイルの頭の一行が ritsu-emit のものになった。生成物が自分で使う名前（`self`、`_str` など）と、TypeScript と Python のファイルの頭の文（`Written by …`）は chobo に残した。
- 確かめたこと：例の全部の `.cal` を五つの出力先に生成したファイル（80 個）と、三つの帳簿を七つの組み合わせに生成したファイル（72 個）が、替える前と一バイトも違わない。koyomi の五つの出力先の突き合わせ、chobo の生成したクライアントを本物の PostgreSQL と TigerBeetle で走らせるテスト、golden は SKIP なしで通る。koyomi に、ritsu-emit の JSON の文字列が serde_json の書くものと同じことを確かめるテストを一つ足した。rulec の表は rulec の `backend::ALL` と、dandori の表は dandori のソースの定数と（公開していないので文字で読む）、語の組として等しいことをテストで確かめる。


### C.11 rulec と dandori（合うところだけ）

- rulec：`src/sha256.rs`、`src/json.rs`（土台の `json` の元なので、置き場所が移るだけ）、`src/sources.rs` の共通の部分（20〜313 行と 1047〜2142 行。写しと表の突き合わせ、534〜1009 行は rulec に残す）、テストの SKIP の書き方（`注意:` → `SKIP: rulec: …`）。診断（`src/diag.rs`）、CLI の表（`src/main.rs`）、`tr!` は残す（DESIGN 4.1、4.2、4.4）。
- dandori：診断の `(en, ja)` の組を `tr!` の順に（機械的に）、CLI を土台の表に移し `--version` とコマンドごとの `--help` を足す、テストの一時ディレクトリと Chrome と golden を `ritsu-testkit` に。rulec とのつなぎ（`src/rulec.rs`、`src/sources.rs`）は D まで触らない。
- どちらも、コーパスとテストの全部、rulec の証明書（`rulec certificate` の出力）、golden が一字も変わらない。

C.11 でしたこと（C の最後の部分）：

- rulec：`src/sha256.rs` を消し、`rulec::sha256` は土台の `sha256` を指す。`src/json.rs` は土台の `json` の上の薄い層になった（456 → 204 行）。残したのは、読めないときの rulec の文（土台が返す種類から選ぶ。位置の数も前と同じ）、値の種類の名前、オブジェクトをキーの順に並べて読み書きすること、`--format json` を組み立てる `Obj` である。`src/sources.rs` は、引用から要素の名前を作ること、写しの場所と本文、固定の行の書き換え、curl と e-Gov と eCFR と GitHub への問い合わせ、base64 を土台のものにした（2,142 → 1,640 行）。写しと表の突き合わせと、`source fetch | pin | outdated` を rulec の文で言う部分は残した。予約語の表（`src/backend.rs` の `words`）は `ritsu-emit` の `copies` から読む（998 → 812 行）。テストの SKIP の行は、29 のファイルで `SKIP: rulec: <理由>` になった。外のツールが要るテストは `ready(Need::…, 見つかるか, 理由)` で先に段を見る。理由の文は前のまま（日本語）。診断、コマンドの表、`tr!` は残した。rulec の DESIGN §15.162 に書いた。
- dandori：診断の文と注、そこに至る実行の一歩、文の切れ端の `(en, ja)` の組を、スクリプトで機械的に `tr!("日本語", "English")` に替えた（613 か所。片方が文字列でも `format!` でもない 9 か所は `Text::new(ja, en)`）。診断の型（`Diag`、`Step`）は、文を土台の `Text` で持つ。CLI は `src/cli.rs` の表（土台の `cli`）に移し、`--version` とコマンドごとの `--help` を足した。テストの一時ディレクトリ、Chrome、golden、SKIP、段は `ritsu-testkit` のものにした。予約語の表（Python、Go、TypeScript）は `copies` から読む。`src/rulec.rs` と `src/sources.rs` は触っていない。dandori の DESIGN 0.3 に書き、README、サイトのコマンドのページ（英語と日本語）、スキルの写し（`skills/sync.sh`）を直した。
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
- `fast`：作業ツリーと同じ中身を、まっさらに取り出した木に置き（gitignore したものは無い）、PATH には cargo と git と curl とシステムのコマンドだけを残して（ほかのツールは、呼ぶと記録を残して失敗するものに替えて）、ジョブのとおりに走らせた。写しを作るのに 2 秒、`cargo build --workspace --locked` に 12 秒、`cargo xtask deps` は通り、`cargo xtask test --level fast` は 1,489 件が通った（ignored 1。SKIP は段で外したもの 303 で、許していないものは 0。ツールの呼び出しは 0）。テストのビルドを含めて 3 分 30 秒、ビルド済みなら 2 分 15 秒だった。
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

DESIGN 5 章。rulec の `src/types.rs` の `money_unit`・`unit_info`・`unit_offset`・`CURRENCIES` と `src/num.rs` の有理数を移し、`Unit`（次元、単位、税込か税抜か、刻み）を足す。テストは、表のすべての綴りの係数、`JPY` と `円`、`USD` と `USDc` の換算、℉ の一次式、順序だけの次元、整数にならない換算を断ること。rulec をこれに替え、コーパスの golden と、すべての規則の `rulec certificate` と `rulec api` の出力が一字も変わらないことを確かめる。

### D.2 `ritsu-ports`

DESIGN 3.2 の型とトレイト（`RuleFacts`、`DateFacts`、`BookFacts`、`Claims`、`Items`、`References`、`Answer`）。出す側が自分で実装する：rulec（`Rules`、`Items`、`References`）、koyomi（`Dates`、`Items`、`References`）、chobo（`Books`、`Items`）、geas（`Claims`、`Items`）、dandori（`Items`、`References`）、yuen と sakai（自分の名指しの `Items` と `References`）。

### D.3 dandori と rulec のつなぎ

1. 突き合わせ：rulec のコーパスの全部の規則と dandori の例の全部の規則について、`Rules::facts` で得た事実と、いまの dandori の `src/rulec.rs` が `rulec schema`・`certificate`・`api` の JSON から組み立てる `RuleInfo` が同じであることを確かめるテストを書き、通す。食い違いがあれば、どちらが正しいかを調べて報告する（DESIGN 1.4 の率の刻みのように、JSON の側が説明の文から読んでいるところは、型の側が正しい）。
2. 通ったら、dandori の `src/rulec.rs` の JSON の読み手と、`src/sources.rs` の rulec を子プロセスで呼ぶところを消し、`Rules` の口を使う。ブラウザで試すページの記録した rulec の出力（`Bundle`）も、口に替える。
3. dandori の CLI を、引数と口と出力先を受け取る関数（`dandori::cli::run`）にする。dandori のテストのうち規則を読むものは、`[dev-dependencies]` の rulec をつないでこの関数を呼ぶ形にする（DESIGN 3.3）。`DANDORI_RULEC` を消し、dandori の README と DESIGN.md を直す。
4. `rulec doc` を埋め込むところも口から。dandori の golden に入っていた `rulec 0.22.0` は、ワークスペースの rulec のバージョンになるので、一度取り直して差分を読む。

### D.4 dandori の単位

DESIGN 5.3。`Ty::Num(Unit)`、`UNIT_KINDS` を `ritsu-units` から、`rate_unit` と `rate_per` を単位の型の刻みに、範囲の端に単位を付けて書けるようにする。DESIGN 1.4 の `hold.flow`（`money[JPY, incl_tax]` を `money[円, incl_tax]` に渡す）が通るテストと、`mass[kg]` を `mass[g]` に渡すと E003 になるテストを足す。生成物はどのプラットフォームでも変わらない（突き合わせのテストで確かめる）。

### D.5 rulec の言語をスレッドごとに

DESIGN 4.1。`i18n::with(lang, || …)` を足し、dandori と yuen が rulec を呼ぶところで使う。テストで、英語と日本語の `rulec doc` を同時に（別のスレッドで）描いて、どちらも正しい言語になること。

### D.6 dandori の種類の語

DESIGN 6.3。dandori の `Items`（`task`、`case`、`record` と `field`、`enum` と `value`、`input`、`output`）と `References`。`naming.tsv` の `dandori "order.flow" task reserve` の行を JSON の行に替え、dandori の入れ子の行を足す。

### D.7 yuen の一式の読み込み

yuen の PLAN の C.1〜C.9 を、子プロセスと JSON ではなく口で作るように書き直してから作る（書き直した計画を yuen の PLAN.md に書く）。

- 端は DESIGN 6.4 の定義の文。rulec の表、koyomi の日付と条件、geas の主張、dandori のタスクと案件が、一つずつ端になる。yuen の PLAN の C.13 に並べたハッシュは、端の中身が変わるので取り直し、yuen の DESIGN.md の 3.2 と 19 章を直す。
- 借りた出典と E107 は土台の `sources` で。
- `affected` は geas の `Claims` の口で記録を読む。
- `.proto` は `ritsu-proto` で。
- E203（ツールがファイルを読めない）と E204（ツールの JSON が知らない形）は、出す側の検査のエラーを名指す形に意味を替えるか、退かせる（DESIGN 7.10）。
- テストの例と確かめた記録（`.req` のハッシュ）を取り直す。yuen の DESIGN の「形の案」のうち、8 章と 11 章を実物にする。

### D.8 sakai の一式の読み込み

sakai の PLAN の C.1〜C.5 を、口で作るように書き直してから作る。

- rulec の列挙と `shape` は `Rules` の口で、koyomi のカレンダーへの参照は `References` で、chobo の勘定と振替は `Books` で（doc のため）。
- dandori の参照を `References` で読み、N101 をやめ、DESIGN 7.10 の四つの検査を足す（コードは sakai の台帳に）。子の `.flow` の扱いを sakai の DESIGN.md に決める。
- E104（ツールが無い）と E105（ツールの api が失敗した）は、意味を替えるか退かせる。
- 例の `check` の要約に、rulec 2、koyomi 1 と、dandori の参照の数が出る（sakai の PLAN の C.15 の表の値を、dandori の分を足して直す）。

### D.9 chobo の単位

DESIGN 5.4。chobo の単位を単位の型に載せ、お金の単位に税込と税抜の区別（`unit 円 incl_tax`）を書けるようにする。区別の無い chobo の単位は、区別の無い額だけを受け取る。chobo の README（英語と日本語）、DESIGN.md、スキル、`explain` の台帳に、新しい書き方を載せる。

### D.10 rulec と dandori を `ritsu-proto` に

二つの `src/proto.rs` を `ritsu-proto` に替える。rulec の契約の検査（コーパスと変異）、dandori の `connect`・`implements`・proto から作る型（`tests/protos.rs` と例）の結果が、替える前と同じであること。

### D.11 D の完了の条件

1. 根から `cargo test --workspace --no-fail-fast -- --nocapture` が全部通り、この機械で SKIP は 0（B.8 で許したものを除く）。
2. ritsu のツールどうしが子プロセスで呼び合うところが無い（`Command::new` で `rulec`・`koyomi`・`chobo`・`geas`・`dandori` を呼ぶコードが、テストのランナーのほかに無い）。
3. D.3 の突き合わせが、JSON の読み手を消す前に全部の規則で通った（報告に件数）。
4. DESIGN 1.4 の二つの例が直っている（`hold.flow` が通る。dandori が率の刻みを文から読まない）。
5. yuen の C.1〜C.9 と sakai の C.1〜C.5 が、書き直した計画の完了の条件を満たす。
6. 報告に、変えた golden の一覧と、yuen の取り直したハッシュの一覧を書く。

## 5. 段階 E：言語をまたぐ検査と一つの入口

大きい段階なので、指示する側が二人に分けてよい。分けるなら E-a（E.1〜E.4 と E.8）を先に、E-b（E.5〜E.7）をそのあとにする（E-b は E.1 の読み込みを使う）。

### E.1 `ritsu-project`

DESIGN 6 章。プロジェクトを歩いて種類を分け、一度ずつ読み、索引を作り、ファイルをまたぐ参照を解決し、出す側の実装を作って受け取る側に渡す。テストのプロジェクトには、sakai の `examples/通販/`（rulec の規則、koyomi のカレンダー、chobo の帳簿、dandori のフロー、`.proto`、コードを持つ）を使い、`crates/ritsu/tests/projects/通販/` に写す。

### E.2 `ritsu check`

DESIGN 8.1、8.3、8.4。テキストと JSON の形を決めて golden にし、英語と日本語で取る。各言語の `check` と同じ診断が出ること（ファイルごとに、その言語の `check` の出力と突き合わせる）、見出しにツールの語が入ること、終了コード。

### E.3 ritsu の台帳と `ritsu explain`

`crates/ritsu-cross/src/codes.rs` に、言語をまたぐ検査のコードを置く（土台の `ledger`）。どのコードにも、出すプロジェクトの最小の再現を置き、テストが走らせる。

### E.4 X1〜X4 と X6 の検査

DESIGN 7.3〜7.6、7.8 のうち、dandori の新しい書き方が要らないもの（X1、X2、X3 の (a)、X4 の rulec の側）から作り、X3 の (b)（rulec の新しい書き方。DESIGN 7.5）を作る。どの検査にも、通るプロジェクトと、落ちる変異（成り立たない例が出るもの、決められないもの）を置き、英語と日本語の golden を取る。X3 の (b) では、rulec の証明書に集合の出どころと集合を書き、`tools/recheck.py` と Lean の再検査が集合の上で通ることも確かめる。

### E.5 dandori から koyomi と chobo を呼ぶ

DESIGN 7.7、7.8。`use dates`、`use book`、タスクの呼び方、`case … follows <帳簿>.<振替>`、時刻を読む式 `now` を、dandori のすべてのプラットフォームに作る。作者の決まり（機能はおまけにしない）のとおり、生成、参照インタプリタの見え方（`View`）、E040 と E050、ランナーと突き合わせのテスト、dandori の README と DESIGN.md の全部に載せる。X4 の dandori の側、X5、X6 の検査をここで仕上げる。chobo の振替を流すランナーは、chobo の `tests/common/servers.rs` の PostgreSQL と TigerBeetle の立て方を使う。

### E.6 `ritsu run`

DESIGN 7.9。dandori の参照インタプリタに、rulec の評価、koyomi のインタプリタ、chobo のインタプリタをつなぐ。テストは、E.1 のプロジェクトのフローを、規則と期日と帳簿を計算しながら流し、結果を golden にすること。

### E.7 `ritsu gen`

DESIGN 9.3。TypeScript、Python、Go の一つのパッケージ。テストは、E.1 のプロジェクトのパッケージが `tsc --strict`、`mypy --strict`、`go vet` を通ること、`ritsu gen --check` が古い生成物を言うこと、ワークフローが規則と期日と帳簿をパッケージの中から読むこと。

### E.8 処理系自身の地図

DESIGN 3.4、7.13。sakai が Rust のクレートの依存を確かめられるようにし（cargo-deny の `wrappers` を書く形を試し、効かなければ `cargo metadata` を読む形）、`ritsu.ctx` を根に置く。わざと言語のクレートに別の言語のクレートを依存させた変異で、sakai がその行を名指すこと。CI の `fast` のジョブに足す。

### E.9 E の完了の条件

1. 根から `cargo test --workspace --no-fail-fast -- --nocapture` が全部通り、この機械で SKIP は 0（B.8 で許したものを除く）。dandori の `platforms` の段も、ほかと同時でなく一度通す。
2. `ritsu check crates/ritsu/tests/projects/通販` が、各言語の `check` と同じ診断と、言語をまたぐ検査の結果を出す（golden）。
3. DESIGN 7.2 の X1〜X7 と X13 のそれぞれに、通るプロジェクトと落ちる変異があり、結果が三つ（示した、例がある、決められない）のどれかで出る。
4. `ritsu run` と `ritsu gen` のテストが通る。
5. DESIGN の 7 章と 8 章の「形の案」を、実物の出力に差し替えた。
6. 報告に、検査ごとの再現と、足した書き方（rulec の範囲の宣言、dandori の `use dates`・`use book`・`now`）の最終の形を書く。

## 6. 段階 F：仕上げ

### F.1 yuen の段階 D

yuen の PLAN の D.1〜D.7 を、ritsu の中で、土台（doc の枠、`ritsu-testkit`）と口を使う形に直してから作る。例には、型付きの読み込みで名指すもの（rulec の表、koyomi の日付、geas の主張）を入れる。README.md、README.ja.md、スキル、THIRD_PARTY_NOTICES.md は `crates/yuen/` の下に。

### F.2 sakai の段階 D

sakai の PLAN の D.1〜D.7 を、同じく ritsu の中で作る。

### F.3 ritsu の文書とスキル

- 根の README.md（英語）と README.ja.md（日本語。英語の写しではなく一から書く）：何をするか、七つの言語とそれぞれの README への案内、`ritsu check` の出力（本物）、言語をまたぐ検査が言うこと（本物の診断）、入れ方、コマンド、どう確かめているか、ライセンス。README の出力と診断は、テストが実際に走らせて照らし合わせる（koyomi と chobo の `tests/docs.rs` の形）。
- DESIGN.md：A のスケッチと見込みを、実物と実際の数に差し替える。
- `skills/ritsu/`：プロジェクトを `ritsu check` で回す流れ、言語をまたぐ診断の直し方、どの言語のスキルを読むか。各言語のスキルは残す。

### F.4 LSP

`ritsu lsp`（標準入出力の JSON-RPC。外のクレートを使わない）。診断（`ritsu check` の結果をファイルごとに）、定義へ移る（索引で。dandori の `use rule` から規則へ、名指しから中のものへ）、型と範囲を見せる（単位の型、範囲、koyomi の値の集合）、中のものの一覧（`Items`）、rulec の整形（`rulec fmt`）。テストは、LSP のメッセージを標準入力から流して、応答を golden にする。

### F.5 ブラウザで試すページ

`ritsu-wasm`（wasm32-unknown-unknown。rulec と dandori と同じ「バッファの頭に長さを書く」決まり）と、ページ（複数のファイルをタブで持つ小さなプロジェクトに `ritsu check` を当て、言語をまたぐ診断を出す。各言語の `gen` と `doc`）。テストは、dandori の `tests/playground.rs` の形（node でモジュールを呼んでコマンドの出力と同じか、Chrome でページを開くか）。

### F.6 Lean の層

DESIGN 11 章。`crates/rulec/proofs/` を根の `proofs/` に移し、rulec の `tests/lean.rs` と CI を新しい場所に合わせる。`ChoboModel`、`KoyomiModel`、`RitsuCross`、`DandoriCore` を DESIGN 11.2 の順に足し、`ritsu-model` と Rust との突き合わせのテストを作る。`sorry`・`axiom`・`native_decide` が無いことを、全部のライブラリについて `#print axioms` で確かめる。

### F.7 リリースの準備

- バージョンの番号を 0.23.0 にそろえる（DESIGN 13.1）。`version.workspace = true`、生成物と golden を一度に取り直す。
- `release.yml`（四つの対象、`ritsu` と七つのリンクのアーカイブ、`SHA256SUMS`）、`packaging/`（`.deb` と `.rpm`）、Homebrew の formula の下書き、`action.yml`。手元で、アーカイブを作ってリンクの名前で呼べること、`.deb` と `.rpm` を作れることを確かめる。
- 配る場所を ritsu に移す準備（DESIGN 13.2。F の最後）：rulec と dandori のサイトの中身（英語と日本語のページ、ブラウザで試すページ）を ritsu のサイトに移し、そのビルドと文書のテスト（いまの rulec の `tests/website.rs`、dandori の `tests/docs.rs` にあたるもの）を ritsu で回す。rulec の Homebrew の formula を ritsu の formula に替える下書きと、`i2y/tap/rulec` を入れている人の移り方を DESIGN 13.2 に書く。
- リリース、タグ、push、公開、サイトと formula の切り替えは、作者の指示があるまでしない。

### F.8 F の完了の条件

1. 根から `cargo test --workspace --no-fail-fast -- --nocapture` が全部通り、この機械で SKIP は 0（B.8 で許したものを除く）。`lake build` と Lean の突き合わせも通る。
2. yuen と sakai の、書き直した段階 D の完了の条件を満たす。
3. 根の README.md と README.ja.md の出力と診断が、テストの確かめを通る。
4. LSP とブラウザのページのテストが通る。
5. DESIGN.md に、スケッチと見込みが残っていない（DESIGN の 7.2 の表と 8.3 の JSON の形を含む）。
6. 報告に、リリースの準備で手元で確かめたことを書く。

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
  - rulec：`website/sync.sh` で写すページを作り、`proofs/` で `lake build` を済ませ、Connect・mypy・NumPy・ruff・uvicorn の入った venv（dandori の `tools/connect/requirements.txt` から作る）を PATH の先頭に、OpenJDK と PostgreSQL の bin を PATH に置き、使い捨ての PostgreSQL を `PGHOST`・`PGPORT`・`PGDATABASE` で渡す。
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
  - `naming.tsv` の yuen と sakai の写しは C.7 と C.8 で消す（PLAN 0.3）。`ritsu-base` のテストは、写しが無くなれば比べないで通る作りにしてある。
  - yuen の `Project::shown` は C.0 で直した。C.7 で土台の `paths::Shown` に替える（同じ考え。ただし yuen はシンボリックリンクをたどったパスを持っている）。
- この機械で根から回すとき：`cargo test --workspace --no-fail-fast -- --nocapture --skip localstack --skip temporal --skip pydantic_graph --skip durable --skip argo --skip ollama`。dandori の重い段は、7.2 のとおり一つずつ回す。環境は 7.2 のものに、`RITSU_PG_BIN`・`RITSU_PG_SOCKET_DIR`・`RITSU_TIGERBEETLE`（`ritsu-testkit` のテスト）を足す。PostgreSQL のソケットのディレクトリは短いパス（`/tmp` の下）にする（作業場所のパスでは 103 バイトを超える）。dandori の `DANDORI_RULEC` には rulec 0.22.0（GitHub のリリースのバイナリを、チェックサムを確かめて作業場所に置く）。geas の `GEAS_PIXIE_GREETER` は、pixie を作者がビルドした場所にある greeter を読む（pixie の木ではビルドしない）。
- `--skip argo` は `cargo` を含む名前にも当たる。根から回すテストに、その語を含む名前を付けない。
- rulec のテストを足すときは、rulec を走らせるところで `RULEC_LANG=ja`（か `--lang`）を渡し、プロセスの中で日本語の文を読むなら、はじめに `rulec::i18n::set(Lang::Ja)` を呼ぶ。`.cargo/config.toml` はもう無い。
- C.12 で `ci/skips/<段>.txt` を書く。段で外したテストの SKIP は理由の種類が `level` で、一覧に書かなくても `cargo xtask test` は通す。

### 7.4 C の二つ目の部分から、C の残りへ（C の二つ目の部分の終わりに書いた）

- 済んだもの：C.4〜C.10。koyomi・chobo・geas・yuen・sakai は土台（`ritsu-base`、`ritsu-testkit`）を使い、sakai は `ritsu-proto` で、koyomi と chobo は `ritsu-emit` で書く。残りは C.11〜C.13。
- C.11 の手がかり：
  - rulec と dandori の予約語の表は `ritsu-emit` の `copies` に写してあり、`crates/ritsu-emit/tests/copies.rs` が二つのいまの表と等しいことを確かめる。替えるときは `copies` から読むようにし、写しのテストを消す。rulec は語を大文字と小文字を区別せずに照らすので、`words::rust::KEYWORDS`（`Self` を含む）をそのまま使っても結果は変わらない。rulec の `GO_GLOBAL` には `complex64` と `complex128` が無く、dandori の Python の組み込みの名前は rulec のものと一部が違う。そろえるなら生成物と診断が変わるので、それぞれの DESIGN.md に理由を書く。
  - `ritsu-emit` の `ident` と `lit` は koyomi と chobo の形である。rulec の `go_package` は小文字にもする（koyomi の別名はもともと小文字）。dandori の Go の外に見せる名前（`exported`）は、区切りで分けて頭を大文字にする形で、chobo の `go_exported` とは違う。
  - rulec の `src/sha256.rs` と `src/json.rs` は、まだ rulec の中にある（`grep -rn 0x428a2f98 crates/*/src` は ritsu-base と rulec に当たる）。
- D.10 の手がかり：
  - `ritsu-proto` のテストは、rulec と dandori を dev-dependency にして古い読み手と比べている。二つが `ritsu-proto` で読むようになれば依存が輪になるので、そのとき比べるところを消し、`tests/golden/rulec.txt` と `dandori.txt` と比べる形だけを残す。
  - 新しい読み手が rulec の読み手と違うのは、一行に書いた `package` を読むことと、壊れたファイルを途中まで読まずに誤りを言うことの二つ（C.9）。後者を rulec がどう扱うか（いまは読めたところまでで契約を突き合わせる）は D.10 で決める。
  - dandori が `ritsu-proto` で読むときは、proto2 と edition を断ること、読めなかった import があるときに型の名前を書いたまま残すこと、型の解決を見えるファイルだけにすることを、dandori の側で書く（`tests/readers.rs` の `as_dandori` がその形で、三つのリポジトリの全部の `.proto` で古い読み手と同じ結果になる）。
- この機械でテストを回すとき（7.3 のものに足す）：sakai のテストは、先に `cargo build --workspace` をして、`SAKAI_RULEC`・`SAKAI_KOYOMI`・`SAKAI_CHOBO`・`SAKAI_DANDORI` にワークスペースの `target/debug` のバイナリを渡す。`ritsu-proto` と `ritsu-emit` のテストは rulec（と dandori）をビルドするので、初めは時間がかかる。
- yuen の名前の漢字は、まだどの文書にも書いていない（作者に聞いているところ）。各クレートの `repository` と yuen の PROV の名前空間の URL は F で決める。

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
  - CI：五つのワークフローは手元でしか確かめていない。初めて走らせたときに、runner との違い（Ubuntu 24.04 の Chrome のサンドボックス、Node 24 と手元の 23.11、PostgreSQL 18 を PGDG から入れること、kind を `go install` で作ること）が出るかもしれない。geas の pixie の greeter を CI でどう用意するかは決まっていない（`ci/skips/tools.txt`）。サイトのビルド（rulec と dandori の `docs.yml`）は、F で配布を ritsu に移すまで ritsu の CI では走らない（DESIGN 10.5）。
- この機械でテストを回すとき（7.3 と 7.4 のものに足す）：根から `cargo xtask test`（段を付けなければ見つかったもので走れるものを全部、付ければその段まで）が SKIP の表を最後に出す。dandori の重いテストは `cargo xtask test --level platforms -p dandori -- --exact <名前>` で一つずつ回す（`platforms.yml` と同じ）。rulec の PostgreSQL は、作業場所の使い捨てのクラスタ（ソケットは `/tmp` の下）を `PGHOST`・`PGPORT`・`PGDATABASE`・`PGUSER` で渡す。
