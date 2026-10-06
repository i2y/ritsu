permit (principal == Organization::Department::Team::User::"a-very-long-user-identifier-that-goes-on", action in [Organization::Action::"first-long-action-name", Organization::Action::"second-long-action-name", Organization::Action::"third"], resource is Organization::Department::Team::Document in Organization::Department::Team::Folder::"shared-folder-with-a-long-name");

permit (principal, action, resource)
when { principal.department.team.manager.assistant.calendar.events.first.attendees.contains(resource.owner.manager.assistant) };

permit (principal, action, resource)
when { context.request.headers.authorization.token.claims.groups.containsAny(["administrators", "operators", "auditors", "security-reviewers", "release-managers"]) };

permit (principal, action, resource)
when { {first: "aaaaaaaaaaaaaaaa", second: "bbbbbbbbbbbbbbbbbbbbbbbb", third: {fourth: "cccccccccccccccccc", fifth: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]}} == context.payload };

permit (principal, action, resource)
when { ((((context.a + context.b) * (context.c - context.d)) + ((context.e * context.f) - (context.g + context.h))) * 1000000) > 2000000000 };

permit (principal, action, resource)
when { if context.flag && context.other_flag && context.third_flag then context.value_when_true else context.value_when_false };

permit (principal, action, resource)
when { decimal(context.amount).lessThanOrEqual(decimal("123456789.1234")) && ip(context.source_address).isInRange(ip("192.168.100.0/24")) };

permit (principal, action, resource)
when { principal has profile.settings.notifications.email.frequency && principal.profile.settings.notifications.email.frequency like "*daily*weekly*" };

permit (principal, action, resource)
when { "日本語の長い文字列です。この文字列は、全角の文字を数える幅を確かめるためのものです。" == context.message };
