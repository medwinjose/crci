// Session 78 — Part A: Tokio spawned-task panic isolation tests
//
// Verifies that a panic inside a tokio::spawn task does NOT kill the Tokio
// runtime thread — it surfaces as Err(JoinError::is_panic()) on the JoinHandle
// and the runtime continues processing subsequent tasks.
//
// Architectural context:
// - runtime.rs:566 spawns a storage write task (fire-and-forget).
// - Session 77's pre-audit found that panics inside spawned tasks bypass
//   UniFFI's catch_unwind (which only covers the direct FFI call stack).
// - The fix in Session 78 wraps the JoinHandle with a watcher task that logs
//   the panic rather than silently dropping it.
// - These tests prove the isolation mechanism works.

/// A panic inside a tokio::spawn task surfaces as Err(JoinError) on the
/// JoinHandle and does NOT propagate to the awaiting thread.
/// This is the fundamental property that makes the watcher pattern safe.
#[tokio::test]
async fn test_spawned_task_panic_does_not_kill_runtime() {
    // Spawn a task that unconditionally panics.
    let handle = tokio::spawn(async {
        panic!("intentional test panic — must not kill runtime thread");
    });

    // Awaiting a panicked JoinHandle returns Err, not a panic propagation.
    let result = handle.await;
    assert!(
        result.is_err(),
        "panicked task should return Err from JoinHandle"
    );

    let join_err = result.unwrap_err();
    assert!(
        join_err.is_panic(),
        "JoinError should report is_panic() == true for a panicking task"
    );

    // Prove the runtime is still alive by running a subsequent task.
    let post_panic = tokio::spawn(async { 42u32 });
    let val = post_panic
        .await
        .expect("post-panic task must complete normally");
    assert_eq!(
        val, 42,
        "runtime must remain functional after a spawned task panicked"
    );
}

/// The watcher-task pattern used in runtime.rs: spawn the real task, then spawn
/// a watcher that awaits the handle. If the inner task panics, the watcher
/// observes Err(JoinError::is_panic()) and the runtime survives.
/// This mirrors the exact structure of the fix applied to runtime.rs:562-586.
#[tokio::test]
async fn test_watcher_task_observes_panic_without_crashing() {
    use std::sync::{Arc, Mutex};

    // Shared log to capture what the watcher observed.
    let panic_observed = Arc::new(Mutex::new(false));
    let panic_observed_clone = panic_observed.clone();

    // Inner task: panics immediately.
    let inner_handle = tokio::spawn(async {
        panic!("inner task panic for watcher test");
    });

    // Watcher task: awaits inner handle, records whether it saw a panic.
    let watcher_handle = tokio::spawn(async move {
        if let Err(join_err) = inner_handle.await {
            if join_err.is_panic() {
                *panic_observed_clone.lock().unwrap() = true;
            }
        }
    });

    // Watcher itself must complete without panicking.
    watcher_handle.await.expect("watcher task must not panic");

    // Confirm the watcher saw the inner panic.
    assert!(
        *panic_observed.lock().unwrap(),
        "watcher task must have observed the inner task's panic via JoinError::is_panic()"
    );

    // Runtime still alive.
    let post = tokio::spawn(async { "alive" });
    assert_eq!(
        post.await.unwrap(),
        "alive",
        "runtime must survive the panic chain"
    );
}

/// Verifies that a non-panicking spawned task returns Ok from its JoinHandle,
/// and that the watcher pattern correctly ignores it.
/// (Regression guard: ensure the watcher doesn't spuriously report panics.)
#[tokio::test]
async fn test_watcher_does_not_false_positive_on_success() {
    use std::sync::{Arc, Mutex};

    let spurious_panic_logged = Arc::new(Mutex::new(false));
    let spurious_clone = spurious_panic_logged.clone();

    // Inner task: succeeds normally.
    let inner_handle = tokio::spawn(async { 99u32 });

    let watcher = tokio::spawn(async move {
        match inner_handle.await {
            Ok(_val) => {
                // Success — watcher does nothing.
            }
            Err(join_err) if join_err.is_panic() => {
                *spurious_clone.lock().unwrap() = true;
            }
            Err(_) => {}
        }
    });

    watcher.await.expect("watcher must complete normally");

    assert!(
        !*spurious_panic_logged.lock().unwrap(),
        "watcher must NOT report a panic when the inner task succeeded"
    );
}
