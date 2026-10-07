// A policy set written by hand for a test: every form of condition sekisho counts, among them some
// the Cedar sekisho generates never has (two strings compared, two booleans compared, `if`, the
// resource in a folder of folders), and principals of a type the schema keeps out of a group or a
// team.
@id("staff_read_their_department")
@doc("A member of the staff reads the documents of their department")
permit (
  principal is Docs::User,
  action == Docs::Action::"read",
  resource is Docs::Doc
)
when { principal.department == resource.department };

@id("staff_list_folders")
@doc("A member of the staff lists any folder")
permit (
  principal in Docs::Group::"staff",
  action == Docs::Action::"read",
  resource is Docs::Folder
);

@id("contractors_stay_out_of_legal")
@doc("A contractor does not read the documents of the legal department")
forbid (
  principal is Docs::User,
  action == Docs::Action::"read",
  resource is Docs::Doc
)
when { principal.contractor && resource.department == "legal" };

@id("documents_for_their_readers")
@doc("A document for contractors is for contractors, and one for the staff is for the staff")
forbid (
  principal is Docs::User,
  action in [Docs::Action::"read", Docs::Action::"edit"],
  resource is Docs::Doc
)
when { resource.for_contractors != principal.contractor };

@id("confidential_for_the_cleared")
@doc("A confidential document is read by the cleared, or for an incident")
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
@doc("The owner of a document edits it")
permit (
  principal,
  action == Docs::Action::"edit",
  resource
)
when { resource.owner == principal };

@id("teams_edit")
@doc("A member of a document's team edits it")
permit (
  principal,
  action == Docs::Action::"edit",
  resource
)
when { principal in resource.team };

@id("editors_edit")
@doc("An editor edits any document")
permit (
  principal in Docs::Group::"editors",
  action == Docs::Action::"edit",
  resource
);

@id("staff_use_templates")
@doc("A member of the staff starts a document from any template")
permit (
  principal is Docs::User in Docs::Group::"staff",
  action == Docs::Action::"use_template",
  resource
);

@id("nothing_in_the_archive")
@doc("Nothing in the archive is touched, but by the archivists")
forbid (principal, action, resource)
when { resource in Docs::Folder::"archive" }
unless { principal in Docs::Group::"archivists" };

@id("indexer_reads")
@doc("The indexer reads every folder, and every document that is not confidential")
permit (
  principal == Docs::Bot::"indexer",
  action == Docs::Action::"read",
  resource
)
unless { resource has confidential && resource.confidential };

@id("only_the_indexer_reindexes")
@doc("No one but the indexer reindexes a folder")
forbid (
  principal,
  action == Docs::Action::"reindex",
  resource
)
unless { principal == Docs::Bot::"indexer" };

@id("indexer_reindexes")
@doc("The indexer reindexes any folder")
permit (
  principal == Docs::Bot::"indexer",
  action == Docs::Action::"reindex",
  resource
);
