# ritsu の Agent Skills

このフォルダーには、九つの [Agent Skills](https://agentskills.io) があります。ritsu のスキルが一つと、八つの言語のスキルが一つずつです。Agent Skills を読むエージェント（Claude Code など）は、作業がスキルの説明に合うと、そのスキルの `SKILL.md` を読み、フォルダーのほかのページは要るときにだけ読みます。どのスキルも、プロジェクトで ritsu と言語を*使う*ためのもので、このリポジトリで作業するためのものではありません。

| スキル | 使う場面 |
|---|---|
| [ritsu](ritsu/SKILL.md) | 二つ以上の言語のファイルを持つプロジェクト。言語をまたぐ `ritsu check`、`ritsu run`、`ritsu gen`、ritsu 自身の診断 |
| [rulec](rulec/SKILL.md) | 業務の規則（`.rule`）。運賃表、手数料、割引、資格の判定を、証明つきのコードにする |
| [dandori](dandori/SKILL.md) | ワークフロー（`.flow`）。走らせる前に検査し、Temporal、Step Functions、durable functions、Argo、pydantic-graph 向けにビルドする |
| [koyomi](koyomi/SKILL.md) | 締め日、支払日、営業日（`.cal`） |
| [chobo](chobo/SKILL.md) | 在庫、お金、ポイント、予約の枠の帳簿（`.book`）。PostgreSQL か TigerBeetle で守る |
| [geas](geas/SKILL.md) | 人が読んだ主張に、エージェントが書いたコードを従わせる（`.geas`） |
| [yuen](yuen/SKILL.md) | 要件の来歴と、それを満たすもの、確かめるもの（`.req`） |
| [sakai](sakai/SKILL.md) | 境界づけられたコンテキストの地図。地図が指す規則、ワークフロー、コードと突き合わせる（`.ctx`） |
| [sekisho](sekisho/SKILL.md) | だれが何をしてよいか（`.gate`）。すべての組み合わせで確かめ、Cedar と、Cedar に尋ねるコードを生成する |

どのスキルも ritsu を動かすので、PATH に `ritsu` が要ります。入れ方は [README](../README.ja.md#入れ方) にあります。言語の名前で呼ぶと（`rulec` という名前のリンクなど）ritsu はその言語として動き、`ritsu <言語> …` でも同じです。

## 入れ方

入れ方は四つあり、どれでも同じファイルが入ります。

### Claude Code：プラグイン

ritsu のサイトが、Claude Code のプラグインのマーケットプレイス `ritsu` を公開しています。マーケットプレイスには同じ名前のプラグイン `ritsu` が一つあり、そこに九つのスキルが入っています。

```text
/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json
/plugin install ritsu@ritsu
```

マーケットプレイスはこの一つのファイルで、プラグインはこのリポジトリの `skills/` のフォルダーです。Claude Code はこのフォルダーだけを取ってくる（1 MB ほど）ので、リポジトリ全体はダウンロードしません。Claude Code は、プラグインのスキルにプラグインの名前を付けて呼びます（`ritsu:ritsu`、`ritsu:rulec`、`ritsu:dandori` など）。プラグインのバージョンは ritsu のバージョンと同じなので、ritsu の新しいリリースが、プラグインの新しいバージョンになります。

### どのエージェントでも：`ritsu skills install`

バイナリが自分と同じ版のスキルを持っているので、PATH に `ritsu` があれば足ります。

```console
$ ritsu skills list --lang ja
ritsu    二つ以上の言語のファイルを持つプロジェクト（`ritsu check`、`ritsu run`、`ritsu gen`、言語をまたぐ診断）
rulec    業務の規則（.rule）
dandori  ワークフロー（.flow）
koyomi   締め日、支払日、営業日（.cal）
chobo    在庫、お金、ポイント、予約の枠の帳簿（.book）
geas     人が読んだ主張に、コードを従わせる（.geas）
yuen     要件の来歴（.req）
sakai    境界づけられたコンテキストの地図（.ctx）
sekisho  だれが何をしてよいかを書き、Cedar を生成する（.gate）
$ ritsu skills install
$ ritsu skills install --user
$ ritsu skills install rulec dandori
$ ritsu skills install --dir path/to/skills
```

`ritsu skills install` は、スキルを一つずつ `<dir>/<名前>/` に書きます。書く場所は、走らせたディレクトリのプロジェクトの `.claude/skills/` です。`--user` を付けると `~/.claude/skills/`（このマシンのすべてのプロジェクト）に、`--dir` を付けると、スキルをほかの場所から読むエージェントのために、そのディレクトリに書きます。`install` のあとに名前を挙げると、そのスキルだけを書きます。すでにあって中身が同じファイルは、そのままにします。ritsu の持つものと中身が違うファイル（手で変えたもの、または別の版の ritsu が書いたもの）があると、何も書かずに止まります。`--force` を付けると上書きします。ritsu が書かないファイルには触りません。

### 手で：フォルダーをコピーする

リポジトリのクローンから、要るフォルダーをコピーします。

```console
$ cp -r skills/rulec skills/dandori ~/.claude/skills/                  # このマシンのすべてのプロジェクトで使う
$ cp -r skills/rulec skills/dandori <your-project>/.claude/skills/     # 一つのプロジェクトで使い、一緒にコミットする
```

### リリースから

リリースごとに `ritsu-skills-v<版>.zip` があり、`SHA256SUMS` にも載っています。中身は九つのフォルダーと二つのライセンスなので、エージェントがスキルを読む場所に展開すれば入ります。

```console
$ unzip ritsu-skills-v0.24.0.zip -d ~/.claude/skills -x 'LICENSE-*'
```

## スキルにコマンドを走らせてもらう

スキルが ritsu と言語のコマンドを毎回確かめずに走らせられるように、プロジェクトの設定（Claude Code なら `.claude/settings.json`）でコマンドを許します。

```json
{ "permissions": { "allow": ["Bash(ritsu check:*)", "Bash(ritsu explain:*)", "Bash(rulec:*)", "Bash(dandori:*)", "Bash(koyomi:*)", "Bash(chobo:*)", "Bash(geas:*)", "Bash(ritsu sekisho:*)"] } }
```

毎回人に聞くべきものは入れません。`ritsu yuen review` は人が見たことを記録するので、いつも聞くようにします。`ritsu skills install` はファイルを書きます。

## どう保っているか

`ritsu/SKILL.md` は手で書いています。言語のスキルは、その言語と一緒に保っています。`crates/<言語>/skills/sync.sh` が、言語の文書からコピーしたページを書き、どのページがそれに当たるかは `crates/<言語>/skills/README.md` にあります。言語ごとのテストが、スキルとツールの食い違いを確かめます。`crates/ritsu/tests/skill.rs` は九つをまとめて確かめます。各フォルダーの `SKILL.md` がフォルダーの名前とリポジトリのライセンスを書いていること、バイナリが九つのフォルダーの全ファイルを持ち、`ritsu skills install` がそれをそのまま書くこと、リリースの zip が同じファイルを持つこと、サイトのマーケットプレイスがワークスペースのバージョンを書き、`skills/` だけを配ること、です。
