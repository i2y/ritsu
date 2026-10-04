# 診断のコード

`ritsu explain --all --format markdown --lang ja` の出力です。手で直しません。

<a id="e101"></a>

## E101 — .proto として読めないファイル

**いつ出るか**: プロジェクトの `.proto` を、ritsu の一つの読み手（ritsu-proto）が読めないとき。閉じていない `{`、`;` の無い文、知らない `syntax`、proto2 の `group`、UTF-8 でないファイル。どの言語もこの読み手で `.proto` を読むので、読めないファイルは、どの言語からも読めません。それを読む言語は、読むところで自分のコードでも言います（rulec の E013、dandori の E016、sakai の E106、yuen の E205）。

**直し方**: 示された位置を直し、proto3 の `.proto` にします。`buf build` が組めるファイルなら、ritsu の読み手も読みます。

**再現**: 下のファイルを一つのディレクトリに置き、そこで `ritsu check .` を走らせます。

`shop.proto`:

```proto
syntax = "proto3";

package shop.v1;

message Order {
  string id = 1;
```
