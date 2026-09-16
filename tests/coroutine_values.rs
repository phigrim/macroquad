use macroquad::{
    experimental::coroutines::{start_coroutine, stop_coroutine},
    telemetry,
    window::next_frame,
};

#[macroquad::test]
async fn coroutine_value() {
    let mut coroutine = start_coroutine(async move {
        next_frame().await;
        1
    });

    coroutine.set_manual_poll();

    assert_eq!(coroutine.retrieve(), None);

    coroutine.poll(0.0);
    coroutine.poll(0.0);

    assert_eq!(coroutine.retrieve(), Some(1));
}

#[macroquad::test]
async fn coroutine_memory() {
    use macroquad::prelude::*;

    for _ in 0..20 {
        start_coroutine(async move {
            next_frame().await;
        });

        next_frame().await;
    }

    // wait for the last one to finish
    next_frame().await;

    assert_eq!(telemetry::active_coroutines_count(), 0);
}

#[macroquad::test]
async fn stopping_cloned_coroutine_handles_is_idempotent() {
    let task = start_coroutine(async { String::from("result") });
    let alias = task.clone();

    stop_coroutine(task);
    stop_coroutine(alias);

    // The second stop must not leave a duplicate free-list entry behind.
    let first = start_coroutine(async {});
    let second = start_coroutine(async {});
    stop_coroutine(first);
    stop_coroutine(second);
}
