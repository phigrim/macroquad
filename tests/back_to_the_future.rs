use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

#[cfg(all(feature = "waker", not(target_arch = "wasm32")))]
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    task::Waker,
};

#[macroquad::test]
async fn back_to_the_future() {
    struct Kaboom;
    impl Future for Kaboom {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            #[cfg(feature = "waker")]
            {
                cx.waker().wake_by_ref();
                cx.waker().clone().wake();
            }
            #[cfg(not(feature = "waker"))]
            let _ = cx.waker().clone();
            Poll::Ready(())
        }
    }
    Kaboom.await;

    #[cfg(all(feature = "waker", not(target_arch = "wasm32")))]
    {
        // A channel-like future is allowed to keep and use the executor's
        // waker from another thread. This is the pattern used by oneshot/mpsc
        // receivers.
        struct WaitForThread {
            ready: Arc<AtomicBool>,
            waker: Arc<Mutex<Option<Waker>>>,
            started: Option<mpsc::Sender<()>>,
        }

        impl Future for WaitForThread {
            type Output = ();

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.ready.load(Ordering::Acquire) {
                    return Poll::Ready(());
                }

                *self.waker.lock().unwrap() = Some(cx.waker().clone());
                if let Some(started) = self.started.take() {
                    started.send(()).unwrap();
                }

                if self.ready.load(Ordering::Acquire) {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            }
        }

        let ready = Arc::new(AtomicBool::new(false));
        let waker = Arc::new(Mutex::new(None::<Waker>));
        let (started_tx, started_rx) = mpsc::channel();

        let thread_ready = ready.clone();
        let thread_waker = waker.clone();
        let thread = std::thread::spawn(move || {
            started_rx.recv().unwrap();
            thread_ready.store(true, Ordering::Release);
            thread_waker.lock().unwrap().take().unwrap().wake();
        });

        WaitForThread {
            ready,
            waker,
            started: Some(started_tx),
        }
        .await;

        thread.join().unwrap();
    }
}
