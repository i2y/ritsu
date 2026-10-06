// A policy set written by hand for a test: two of its policies read what sekisho does not count,
// a pattern (`like`) and arithmetic (`*`); the others are written in the part it does.
@id("readers_read")
@doc("A reader reads any document, unless suspended")
permit (
  principal in Library::Role::"reader",
  action == Library::Action::"read",
  resource is Library::Doc
)
unless { principal.suspended };

@id("drafts_are_not_printed")
@doc("A draft is not printed")
forbid (
  principal,
  action == Library::Action::"print",
  resource
)
when { resource.title like "*draft*" };

@id("readers_print_within_their_limit")
@doc("A reader prints up to the reader's own limit of pages")
permit (
  principal in Library::Role::"reader",
  action == Library::Action::"print",
  resource is Library::Doc
)
when { resource.pages * context.copies <= principal.limit };

@id("owners_archive")
@doc("Whoever owns a document archives it")
permit (
  principal,
  action == Library::Action::"archive",
  resource is Library::Doc
)
when { resource.owner == principal };
