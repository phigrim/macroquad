use std::future::Future;
use std::pin::Pin;
#[cfg(feature = "waker")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(feature = "waker")]
use std::task::Wake;
use std::task::{Context, Poll, Waker};
#[cfg(not(feature = "waker"))]
use std::task::{RawWaker, RawWakerVTable};

use crate::Error;

/// Scheduling state associated with one future.
///
/// With the `waker` feature this is a one-bit runnable queue shared by the
/// future and all wakers created while polling it. Without the feature, this
/// scheduling state becomes a zero-sized compatibility shim and the executor
/// keeps polling futures every frame as it did historically.
pub(crate) struct Task<T> {
    future: Pin<Box<dyn Future<Output = T>>>,
    #[cfg(feature = "waker")]
    state: Arc<TaskState>,
}

#[cfg(feature = "waker")]
struct TaskState {
    runnable: AtomicBool,
    schedule_update: bool,
}

#[cfg(feature = "waker")]
impl TaskState {
    fn wake(&self) {
        // Coalesce repeated wakes while the task is already runnable. Apart
        // from reducing native requests, this also keeps a noisy channel from
        // causing unbounded work in the event loop.
        if !self.runnable.swap(true, Ordering::AcqRel) && self.schedule_update {
            miniquad::window::schedule_update();
        }
    }
}

impl<T> Task<T> {
    pub(crate) fn new(future: impl Future<Output = T> + 'static) -> Self {
        Self::from_pinned(Box::pin(future))
    }

    pub(crate) fn from_pinned(future: Pin<Box<dyn Future<Output = T>>>) -> Self {
        Self {
            future,
            #[cfg(feature = "waker")]
            state: Arc::new(TaskState {
                runnable: AtomicBool::new(true),
                schedule_update: miniquad::window::blocking_event_loop(),
            }),
        }
    }

    /// Claims the current runnable notification before polling the future.
    ///
    /// Clearing the bit first is important: if a future calls `wake()` from
    /// inside its own `poll()`, that wake is observed and is not lost.
    #[inline]
    pub(crate) fn claim(&self) -> bool {
        #[cfg(feature = "waker")]
        {
            self.state.runnable.swap(false, Ordering::AcqRel)
        }
        #[cfg(not(feature = "waker"))]
        {
            true
        }
    }

    pub(crate) fn poll(&mut self) -> Poll<T> {
        let waker = self.waker();
        let mut futures_context = Context::from_waker(&waker);
        self.future.as_mut().poll(&mut futures_context)
    }

    fn waker(&self) -> Waker {
        #[cfg(feature = "waker")]
        {
            Waker::from(self.state.clone())
        }
        #[cfg(not(feature = "waker"))]
        {
            noop_waker()
        }
    }
}

#[cfg(feature = "waker")]
impl Wake for TaskState {
    fn wake(self: Arc<Self>) {
        TaskState::wake(&self);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        TaskState::wake(self);
    }
}

/// Stores the current future waker for a callback that may complete on
/// another thread. It is a zero-sized no-op when `waker` is disabled.
#[derive(Clone)]
pub(crate) struct WakerRegistration {
    #[cfg(feature = "waker")]
    waker: Arc<Mutex<Option<Waker>>>,
}

impl WakerRegistration {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(feature = "waker")]
            waker: Arc::new(Mutex::new(None)),
        }
    }

    #[inline]
    pub(crate) fn register(&self, _context: &Context<'_>) {
        #[cfg(feature = "waker")]
        {
            // Register before inspecting the result. If the callback wins the
            // race, the second result check in the future observes it; if it
            // wins after this registration, it wakes this task.
            *self.waker.lock().unwrap() = Some(_context.waker().clone());
        }
    }

    #[inline]
    pub(crate) fn clear(&self) {
        #[cfg(feature = "waker")]
        {
            self.waker.lock().unwrap().take();
        }
    }

    pub(crate) fn wake(&self) {
        #[cfg(feature = "waker")]
        {
            let waker = self.waker.lock().unwrap().take();
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }
}

#[cfg(not(feature = "waker"))]
fn noop_waker() -> Waker {
    unsafe fn clone(data: *const ()) -> RawWaker {
        RawWaker::new(data, &VTABLE)
    }
    unsafe fn wake(_data: *const ()) {}
    unsafe fn wake_by_ref(data: *const ()) {
        wake(data);
    }
    unsafe fn drop(_data: *const ()) {}
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

/// Wakes a frame-driven future when waker support is enabled.
#[inline]
pub(crate) fn wake(_context: &Context<'_>) {
    #[cfg(feature = "waker")]
    _context.waker().wake_by_ref();
}

// Returns Pending until the next frame, then Ready.
#[derive(Default)]
pub struct FrameFuture {
    done: bool,
}

impl Future for FrameFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Self::Output> {
        if self.done {
            Poll::Ready(())
        } else {
            self.done = true;
            // Frame-driven futures are the one intentional exception to the
            // event-driven rule: arrange for this task to be polled on the
            // following frame instead of relying on an unconditional scan.
            wake(context);
            Poll::Pending
        }
    }
}

pub struct FileLoadingFuture {
    pub contents: Arc<Mutex<Option<Result<Vec<u8>, Error>>>>,
    pub(crate) waker: WakerRegistration,
}

impl Future for FileLoadingFuture {
    type Output = Result<Vec<u8>, Error>;

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<Self::Output> {
        self.waker.register(context);

        let contents = self.contents.lock().unwrap().take();
        match contents {
            Some(contents) => {
                self.waker.clear();
                Poll::Ready(contents)
            }
            None => Poll::Pending,
        }
    }
}

#[cfg(all(test, feature = "waker", not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn waker_can_be_cloned_and_woken_from_another_thread() {
        let task = Task {
            future: Box::pin(std::future::pending::<()>()),
            state: Arc::new(TaskState {
                runnable: AtomicBool::new(false),
                schedule_update: false,
            }),
        };
        let waker = task.waker();

        let thread = std::thread::spawn(move || {
            waker.clone().wake();
            waker.wake_by_ref();
        });
        thread.join().unwrap();

        assert!(task.claim());
        assert!(!task.claim());
    }

    #[test]
    fn wake_during_poll_is_not_lost() {
        let task = Task {
            future: Box::pin(std::future::pending::<()>()),
            state: Arc::new(TaskState {
                runnable: AtomicBool::new(true),
                schedule_update: false,
            }),
        };
        let waker = task.waker();

        assert!(task.claim());
        waker.wake_by_ref();
        assert!(task.claim());
    }
}
