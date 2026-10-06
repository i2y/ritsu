@id("foo")
permit (principal, action, resource);

@id("\nfoo\n\nbar\n")
permit (principal, action, resource);

@shadow_mode
permit (principal, action, resource);

@shadow_mode("")
permit (principal, action, resource);

@shadow_mode
permit (principal, action, resource);

@shadow_mode("")
permit (principal, action, resource);

@foo
@bar
@baz("buz")
permit (principal, action, resource);

@foo
@bar
@baz("buz")
@biz
permit (principal, action, resource);

@shadow_mode
permit (principal, action, resource);

@shadow_mode("")
permit (principal, action, resource);
