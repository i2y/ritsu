// テストのために手で書いたポリシー。sekisho が数える条件の形を全部使う。sekisho が生成する Cedar には
// 出ない形（二つの文字列を比べる、二つの真偽を比べる、`if`、入れ子のフォルダに入った resource）と、
// スキーマがグループやチームに入れない型の principal も含む。
@id("staff_read_their_department")
@name("職員は自分の部署の文書を読める")
@doc("職員は、自分の部署の文書を読める")
permit (
  principal is Docs::User,
  action == Docs::Action::"read",
  resource is Docs::Doc
)
when { principal.department == resource.department };

@id("staff_list_folders")
@name("職員はフォルダの中を見られる")
@doc("staff のグループの職員は、どのフォルダの中も見られる")
permit (
  principal in Docs::Group::"staff",
  action == Docs::Action::"read",
  resource is Docs::Folder
);

@id("contractors_stay_out_of_legal")
@name("契約社員は法務の文書を読まない")
@doc("契約社員は、法務部の文書を読まない")
forbid (
  principal is Docs::User,
  action == Docs::Action::"read",
  resource is Docs::Doc
)
when { principal.contractor && resource.department == "legal" };

@id("documents_for_their_readers")
@name("文書は向けた相手だけが読み書きする")
@doc("契約社員向けの文書は契約社員だけが、職員向けの文書は職員だけが読み書きする")
forbid (
  principal is Docs::User,
  action in [Docs::Action::"read", Docs::Action::"edit"],
  resource is Docs::Doc
)
when { resource.for_contractors != principal.contractor };

@id("confidential_for_the_cleared")
@name("機密の文書は許可のある人か障害のときだけ")
@doc("機密の文書を読めるのは、cleared のグループの人か、障害に対応するときだけ")
forbid (
  principal,
  action == Docs::Action::"read",
  resource is Docs::Doc
)
unless
{
  if
    resource.confidential
  then
    principal in Docs::Group::"cleared" ||
    context has incident
  else
    true
};

@id("owners_edit")
@name("持ち主は文書を編集できる")
@doc("文書の持ち主は、その文書を編集できる")
permit (
  principal,
  action == Docs::Action::"edit",
  resource
)
when { resource.owner == principal };

@id("teams_edit")
@name("チームのメンバーは文書を編集できる")
@doc("文書のチームのメンバーは、その文書を編集できる")
permit (
  principal,
  action == Docs::Action::"edit",
  resource
)
when { principal in resource.team };

@id("editors_edit")
@name("編集者はどの文書も編集できる")
@doc("editors のグループの人は、どの文書も編集できる")
permit (
  principal in Docs::Group::"editors",
  action == Docs::Action::"edit",
  resource
);

@id("staff_use_templates")
@name("職員はテンプレートを使える")
@doc("staff のグループの職員は、どのテンプレートからも文書を作れる")
permit (
  principal is Docs::User in Docs::Group::"staff",
  action == Docs::Action::"use_template",
  resource
);

@id("nothing_in_the_archive")
@name("アーカイブの中のものには触れない")
@doc("アーカイブの中のものには、archivists のグループの人のほかはだれも触れない")
forbid (principal, action, resource)
when { resource in Docs::Folder::"archive" }
unless { principal in Docs::Group::"archivists" };

@id("indexer_reads")
@name("indexer は機密でないものを読める")
@doc("indexer のボットは、どのフォルダの中も、機密でないどの文書も読める")
permit (
  principal == Docs::Bot::"indexer",
  action == Docs::Action::"read",
  resource
)
unless { resource has confidential && resource.confidential };

@id("only_the_indexer_reindexes")
@name("インデックスを作り直すのは indexer だけ")
@doc("フォルダのインデックスを作り直すのは、indexer のボットだけ")
forbid (
  principal,
  action == Docs::Action::"reindex",
  resource
)
unless { principal == Docs::Bot::"indexer" };

@id("indexer_reindexes")
@name("indexer はインデックスを作り直せる")
@doc("indexer のボットは、どのフォルダのインデックスも作り直せる")
permit (
  principal == Docs::Bot::"indexer",
  action == Docs::Action::"reindex",
  resource
);
