// c24_e03_mini_executor.rs
//
// CONCEPT: what an executor is — a queue of tasks, wakers that re-queue a
//          task, and `block_on` that parks the thread until woken. Built with
//          std only.
// GUIDE:   chapters/24-async-internals/guide.md  (section "Build an executor")
// DATABROKER: databroker/src/main.rs — `tokio::runtime::Builder::new_multi_thread()`
//             (line 417) and `runtime.block_on(async { ... })` (line 423) are
//             the industrial-strength versions of what you build here.
// EXPECT:  tests fail: `todo!()` panics
//
// Tokio is "just" (a lot of engineering around) this loop:
//
//     while let Some(task) = queue.pop() {
//         task.poll();               // runs until the task's next .await
//     }                              // a Pending task is re-queued by its waker
//
// YOUR TASK
//   1. `block_on`: poll the future; on `Pending`, park the current thread
//      until the waker unparks it. `ThreadWaker` is written for you.
//   2. `impl Wake for Task`: waking a task sends it back into the queue.
//   3. `Executor::spawn`: box + pin the future, wrap it in an `Arc<Task>`
//      and queue it.
//   4. `Executor::run`: pop tasks and poll each one with a waker made from
//      the task itself, until every task has finished.
//   Do not edit the tests or `YieldNow`.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

// ---------------------------------------------------------------- block_on

/// Waking this unparks the thread that is blocked in `block_on`.
struct ThreadWaker {
    thread: Thread,
}

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.thread.unpark();
    }
}

/// Runs one future to completion on the current thread.
fn block_on<F: Future>(fut: F) -> F::Output {
    // `pin!` pins the future on this stack frame: it can't move again, which
    // is exactly what `poll(self: Pin<&mut Self>)` requires.
    let mut fut = std::pin::pin!(fut);
    let waker = Waker::from(Arc::new(ThreadWaker {
        thread: thread::current(),
    }));
    let mut cx = Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            // WHY: `park` may return spuriously, and an `unpark` that happened
            // before `park` is remembered — so "poll, park, repeat" is correct.
            Poll::Pending => thread::park(),
        }
    }
}

// ---------------------------------------------------------------- executor

type BoxedTask = Pin<Box<dyn Future<Output = ()> + Send>>;
type Queue = Arc<Mutex<VecDeque<Arc<Task>>>>;

struct Task {
    /// `None` once the future has completed.
    future: Mutex<Option<BoxedTask>>,
    queue: Queue,
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        let queue = self.queue.clone();
        queue.lock().expect("queue poisoned").push_back(self);
    }
}

#[derive(Default)]
struct Executor {
    queue: Queue,
}

impl Executor {
    fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) {
        let task = Arc::new(Task {
            future: Mutex::new(Some(Box::pin(fut))),
            queue: self.queue.clone(),
        });
        self.queue.lock().expect("queue poisoned").push_back(task);
    }

    /// Runs until the queue is empty. (Tasks that are `Pending` and not woken
    /// are simply not in the queue — a real runtime would wait for I/O here.)
    fn run(&self) {
        loop {
            // WHY: pop in its own statement so the queue lock is released
            // before polling — the task's waker needs that same lock.
            let next = self.queue.lock().expect("queue poisoned").pop_front();
            let Some(task) = next else { break };

            let mut slot = task.future.lock().expect("task poisoned");
            if let Some(mut fut) = slot.take() {
                let waker = Waker::from(task.clone());
                let mut cx = Context::from_waker(&waker);
                if fut.as_mut().poll(&mut cx).is_pending() {
                    // Not finished: put it back so a later wake can poll it.
                    *slot = Some(fut);
                }
            }
        }
    }
}

/// Returns `Pending` once (waking itself), then `Ready` — like `tokio::task::yield_now`.
struct YieldNow {
    yielded: bool,
}

fn yield_now() -> YieldNow {
    YieldNow { yielded: false }
}

impl Future for YieldNow {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    /// Completes once `flag` is set; the setter thread wakes us.
    struct WaitForFlag {
        flag: Arc<AtomicBool>,
        waker_slot: Arc<Mutex<Option<Waker>>>,
    }

    impl Future for WaitForFlag {
        type Output = &'static str;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<&'static str> {
            *self.waker_slot.lock().unwrap() = Some(cx.waker().clone());
            if self.flag.load(Ordering::SeqCst) {
                Poll::Ready("flag set")
            } else {
                Poll::Pending
            }
        }
    }

    #[test]
    fn block_on_ready_future() {
        assert_eq!(block_on(async { 1 + 2 }), 3);
    }

    #[test]
    fn block_on_sleeps_until_woken_by_another_thread() {
        let flag = Arc::new(AtomicBool::new(false));
        let waker_slot: Arc<Mutex<Option<Waker>>> = Arc::default();
        let (f, w) = (flag.clone(), waker_slot.clone());
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            f.store(true, Ordering::SeqCst);
            if let Some(waker) = w.lock().unwrap().take() {
                waker.wake();
            }
        });
        let out = block_on(WaitForFlag { flag, waker_slot });
        assert_eq!(out, "flag set");
    }

    #[test]
    fn block_on_drives_async_blocks_with_awaits() {
        let out = block_on(async {
            yield_now().await;
            yield_now().await;
            "done"
        });
        assert_eq!(out, "done");
    }

    #[test]
    fn executor_interleaves_tasks_at_await_points() {
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let exec = Executor::default();
        for name in ["A", "B"] {
            let log = log.clone();
            exec.spawn(async move {
                for step in 1..=3 {
                    log.lock().unwrap().push(format!("{name}{step}"));
                    yield_now().await;
                }
            });
        }
        exec.run();
        assert_eq!(
            *log.lock().unwrap(),
            ["A1", "B1", "A2", "B2", "A3", "B3"],
            "each `.await` is a yield point: the tasks take turns"
        );
    }

    #[test]
    fn executor_runs_many_tasks_to_completion() {
        let done = Arc::new(Mutex::new(0));
        let exec = Executor::default();
        for _ in 0..100 {
            let done = done.clone();
            exec.spawn(async move {
                yield_now().await;
                *done.lock().unwrap() += 1;
            });
        }
        exec.run();
        assert_eq!(*done.lock().unwrap(), 100);
        assert!(exec.queue.lock().unwrap().is_empty());
    }
}
