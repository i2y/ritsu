// テストのために手で書いたポリシー。二つは sekisho が数えないもの（パターンの `like` と、算術の `*`）を
// 読み、ほかは sekisho が数える部分で書いてある。
@id("readers_read")
@name("利用者は文書を読める")
@doc("利用者は、停止中でなければ、どの文書も読める")
permit (
  principal in Library::Role::"reader",
  action == Library::Action::"read",
  resource is Library::Doc
)
unless { principal.suspended };

@id("drafts_are_not_printed")
@name("下書きは印刷しない")
@doc("下書きは印刷しない")
forbid (
  principal,
  action == Library::Action::"print",
  resource
)
when { resource.title like "*draft*" };

@id("readers_print_within_their_limit")
@name("利用者は自分の上限まで印刷できる")
@doc("利用者は、自分のページ数の上限まで印刷できる")
permit (
  principal in Library::Role::"reader",
  action == Library::Action::"print",
  resource is Library::Doc
)
when { resource.pages * context.copies <= principal.limit };

@id("owners_archive")
@name("持ち主は保管に回せる")
@doc("文書の持ち主は、その文書を保管に回せる")
permit (
  principal,
  action == Library::Action::"archive",
  resource is Library::Doc
)
when { resource.owner == principal };
