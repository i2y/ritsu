# ritsu

[English](README.md) | [日本語](README.ja.md) | 简体中文

**用小型语言写下系统必须遵守的规则，并检查每个输入都恰好得到一个答案、规则之间互不矛盾。ritsu 从检查过的规则生成代码、供人阅读的页面，以及用于测试的输入、答案和场景。**

> 本页是 ritsu 的简体中文简介，内容以英文版 [README](README.md) 为准，可能落后于英文版。ritsu 的输出（诊断、帮助和生成的页面）是英文的，加上 `--lang ja` 时为日文，目前没有中文。

编码智能体写代码的速度和数量，已经超过了任何人逐行阅读的能力。这让一个问题变得更尖锐：无论代码由谁来写，甚至由什么来写，它到底要实现什么？服务之间和服务内部的契约。业务规则。日历与期限。账本及其上下限。工作流的骨架。谁可以做什么。每条需求从何而来，现在又由什么来满足。

ritsu 为其中每一项提供一种小型语言，写在人能读懂的文件里。每种语言都在能检查的范围内检查文件所写的内容：覆盖每个输入、每一天、每条路径，而不是抽样。需要代码的地方，它生成代码；需要理解内容的人，它为他们生成页面。九种语言共用一套工具链，所以证明不会在语言的边界处中断：工作流知道它调用的规则有哪些前置条件，规则知道日历可能落在哪些日子，账本知道规则可能返回哪些金额。

胶水代码交给智能体去写，代码必须实现的东西由 ritsu 来把握。

## 九种语言

| | 文件 | 用途 | 生成 |
|---|---|---|---|
| [rulec](README.md#rulec--business-rules) | `.rule` | 业务规则：费率、手续费、资格判定、税费 | 十二种语言的函数 |
| [dandori](README.md#dandori--the-skeleton-of-a-workflow) | `.flow` | 工作流的骨架 | Temporal、Step Functions、Argo 等 |
| [koyomi](README.md#koyomi--dates) | `.cal` | 日期：结账日和付款日、工作日、法定期间 | TypeScript、Python、Go、Rust、SQL、Wasm |
| [chobo](README.md#chobo--ledgers) | `.book` | 账本：账户及其上下限、转账、预留 | PostgreSQL、TigerBeetle、MySQL、TiDB 和 Vitess 的客户端 |
| [geas](README.md#geas--claims-about-the-code-an-agent-wrote) | `.geas` | 对智能体所写代码的断言 | — |
| [yuen](README.md#yuen--where-requirements-come-from) | `.req` | 需求从何而来，又由什么来满足 | ReqIF、W3C PROV |
| [sakai](README.md#sakai--the-map-of-bounded-contexts) | `.ctx` | 限界上下文的映射图 | 导入检查工具（import linter）的配置；Context Mapper |
| [sekisho](README.md#sekisho--who-may-do-what) | `.gate` | 谁可以做什么：角色、属性、关系，并以规则和日期为条件 | Cedar，以及调用它的 TypeScript、Python 和 Go |
| [kaname](README.md#kaname--a-functional-core) | `.kn` | 函数式核心：不做输入输出的通用代码，可以按各自的类型调用规则、日期和账本 | TypeScript、Python 和 Go，放在 `ritsu gen` 写出的包里 |

语言名的链接指向英文 README 中介绍该语言的一节。每种语言都可以单独使用：不写 `.flow` 也能用 rulec。一个文件不会混用两种语言，因为费率、日历、账本和工作流的读者各不相同。

## 语言交汇之处

九种语言共用一套工具链，因此一种语言检查过的东西，另一种语言可以直接当作前提。下面这条退款规则假定，没有人会申请超过已付金额的退款：

```rule
rule refund_check v1
description "Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted"

enum path = at_once | review

inputs
  paid  : number  range >=0 <=10000
  asked : number  range >=0 <=10000

constraint asked <= paid

outputs
  route : path

table pick
policy unique
| asked | -> route : path |
| <=100 | at_once         |
| >100  | review          |
```

工作流用顾客申请的金额调用它：

```flow
workflow refund v1
description "Pays a refund back at once or sends it to review, as the rule decides"

use rule check from "refund_check.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:refund-check"

inputs
  order : string
  paid  : int  range >=0 <=10000

task ask_amount(order: string) -> int range >=0 <=10000
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-amount"
  idempotent

task pay_back(order: string, amount: int range >=0 <=10000)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:pay-back"
  key

flow
  let asked = ask_amount(order: order)
  let decision = check(paid: paid, asked: asked)
  match decision.route
    at_once => pay_back(order: order, amount: asked)
    review => pass
```

两个文件各自都能通过自己语言的检查，放在一起却通不过（这两个文件来自 `ritsu explain E201` 输出的复现示例，该示例还从一个 kaname 模块调用这条规则）：

```console
$ ritsu check .
ok refund_check.rule
refund.flow: ok
error[ritsu E201]: refund.flow:21:1: The call of the rule check can give it values that break its precondition `asked <= paid`
    21 |   let decision = check(paid: paid, asked: asked)
  = `asked` is `>=0 <=10000` and `paid` is `>=0 <=10000`, and at asked = 10000, paid = 0 `asked <= paid` does not hold
  = The rule's generated code refuses a call that breaks a precondition at its door, so this call fails only when the workflow runs. The ranges are dandori's, gathered from every place the values come from.
  = Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result).
ritsu check: 2 files (rulec 1, dandori 1): 1 fail (1 error); borders between the languages: 1 checked, 0 undecided
```

输出的意思是：`asked` 和 `paid` 都可能是 0 到 10000 之间的任何值，当 asked = 10000、paid = 0 时，前置条件 `asked <= paid` 不成立。规则生成的代码会在入口拒绝违反前置条件的调用，所以这样的调用要到工作流运行时才会失败。修正的方法是在调用之前加上分支，保证前置条件成立，或者缩小取值范围（输入的 `range`，或任务结果的 `range`）。

每一项跨语言的检查都会得到三种答案之一：成立；给出一个不成立的例子；或者无法判定，并说明原因。没有任何检查会悄无声息地通过。跨语言的检查还包括：koyomi 的日期可能落在的日子与规则输入的范围、规则的输出作为 chobo 转账的金额、kaname 模块传给规则、日期和转账的每个值，以及在 rulec、dandori、chobo 和 kaname 之间通用的金额与单位类型。完整的列表见英文 README 的 [Where the languages meet](README.md#where-the-languages-meet)。

## 供人阅读的页面

同样的文件还能生成英文或日文的页面，供需要理解代码要实现什么的人阅读，比如业务负责人、财务和法务、运维人员，以及审查代码的开发者。他们可以读懂写下的内容，并与自己掌握的情况对照。

- `rulec doc`：规则的表格、附带条件，以及对它们证明了什么，输出 Markdown 或 HTML；
- `dandori doc`：工作流的图，以及它运行的每个场景；
- `koyomi doc`：按月展示的日历；
- `chobo doc`：账本、它的上下限和转账；
- `sekisho doc`：按操作列出谁可以做什么，旁边是编译出的 Cedar；
- `kaname doc`：模块中表格里的决策、按效果分类的函数及其可能失败的地方，以及写在代码中的决策；
- `yuen trace`：把一条需求追溯到来源，并找到满足它的东西。

## 证明

rulec 的证书会在 Lean 4 中重新检查。chobo、koyomi、dandori 的核心、kaname 的求值与范围分析，以及 ritsu 的跨语言检查，都有与 Rust 实现并列的 Lean 模型，两者在同样生成的输入上运行，逐个比对答案。没有任何证明依赖 `sorry` 或自行添加的公理。

## 安装

从源码安装（需要较新的稳定版 Rust）：

```
cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

从 0.23.0 起，每个版本都在[发布页面](https://github.com/i2y/ritsu/releases)提供 macOS 和 Linux 的压缩包，以及 `.deb` 和 `.rpm` 包。使用 Homebrew 时为 `brew install i2y/tap/ritsu`。

发布的压缩包里有 `ritsu` 和指向它的九个链接（`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`yuen`、`sakai`、`sekisho`、`kaname`），每个链接都作为对应的语言运行。`cargo install` 只安装 `ritsu` 这一个二进制文件：可以用 `ritsu <语言> …` 调用某种语言，也可以自己按语言名建立链接（`ln -s "$(command -v ritsu)" ~/.cargo/bin/rulec`）。项目的检查用 `ritsu check`，生成代码用 `ritsu gen`。

只安装一种语言、在 GitHub Actions 中使用，以及 npm 包，见英文 README 的 [Install](README.md#install)。

## 面向 AI 智能体

ritsu 及其语言是为 AI 智能体使用而设计的。[skills/](skills) 中有十个 [Agent Skills](https://agentskills.io)：[skills/ritsu](skills/ritsu) 用于包含多种语言的项目，其余九个各对应一种语言。`ritsu skills install` 会把它们写入项目的 `.claude/skills/`。其他安装方式见 [skills/README.md](skills/README.md)（英文）。

## 更多

- [英文 README](README.md)：全部内容，包括模块、安全检查和命令一览。
- [文档网站](https://i2y.github.io/ritsu/)（英文）、[日文版](https://i2y.github.io/ritsu/ja/)和[简体中文首页](https://i2y.github.io/ritsu/zh/)。
- [在浏览器中试用](https://i2y.github.io/ritsu/playground/)，无需安装。
- 整体设计见 [DESIGN.md](DESIGN.md) 和 [PLAN.md](PLAN.md)（日文）。

## 许可证

MIT OR Apache-2.0（[LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE)）。其中三种语言包含他人的数据，因此 `ritsu` 二进制文件的许可证为 `(MIT OR Apache-2.0) AND Unicode-3.0 AND BSD-3-Clause`。各项数据的来源和许可证全文见 [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES)。
