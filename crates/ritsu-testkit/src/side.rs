//! Running the parts of a test side by side: the generated code of many rules, built and run in
//! every language, where each part waits on programs and shares nothing with the others but what
//! it reads.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Run `each` on every item, as many at once as the machine has cores, on threads named after
/// the test that called it, as libtest names the test's own: a SKIP line said in a part is the
/// test's ([`crate::skip::test_name`]). A part that fails does not stop the others; once every
/// part has run, the test fails with what each part that failed said, in the order of the items.
pub fn side_by_side<T: Sync>(items: &[T], each: impl Fn(&T) + Sync) {
    let at_once = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, items.len().max(1));
    let name = std::thread::current().name().map(String::from);
    let next = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..at_once {
            let mut part = std::thread::Builder::new();
            if let Some(n) = &name {
                part = part.name(n.clone());
            }
            part.spawn_scoped(scope, || {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(item) = items.get(i) else { break };
                    if let Err(p) = catch_unwind(AssertUnwindSafe(|| each(item))) {
                        failed.lock().unwrap_or_else(|e| e.into_inner()).push((i, said(p.as_ref())));
                    }
                }
            })
            .expect("cannot start a thread");
        }
    });
    let mut failed = failed.into_inner().unwrap_or_else(|e| e.into_inner());
    failed.sort();
    assert!(failed.is_empty(), "{}", failed.into_iter().map(|(_, m)| m).collect::<Vec<_>>().join("\n\n"));
}

/// What a panic said.
fn said(p: &(dyn std::any::Any + Send)) -> String {
    match (p.downcast_ref::<String>(), p.downcast_ref::<&str>()) {
        (Some(s), _) => s.clone(),
        (None, Some(s)) => s.to_string(),
        (None, None) => "a panic that said nothing".into(),
    }
}
