use core::ffi::c_void;
use tokio::runtime::dump::TraceMeta;

use crate::unwind::{
    _Unwind_Backtrace, _Unwind_Context, _Unwind_GetIP,
    _Unwind_Reason_Code::{self, _URC_NO_REASON},
};

/// A captured stack trace from a Tokio task.
///
/// Contains the instruction pointer addresses for each frame in the trace,
/// along with the root and leaf addresses that bound the task's execution.
///
/// # Examples
///
/// ```
/// use tokio_taskdump::capture_trace;
/// # use core::ffi::c_void;
///
/// // Create an empty trace to be filled by capture_trace
/// let trace = tokio_taskdump::TaskTrace::empty();
/// assert!(trace.frames().is_empty());
/// ```
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TaskTrace {
    /// The instruction pointer addresses captured from the stack.
    frames: Vec<usize>,
    /// The root address of the task (entry point).
    root_addr: *const c_void,
    /// The leaf address of the task (yield point).
    leaf_addr: *const c_void,
}

impl TaskTrace {
    /// Creates a new empty `TaskTrace` with no frames and null addresses.
    ///
    /// # Examples
    ///
    /// ```
    /// let trace = tokio_taskdump::TaskTrace::empty();
    /// assert!(trace.frames().is_empty());
    /// ```
    pub fn empty() -> TaskTrace {
        Self {
            frames: vec![],
            root_addr: std::ptr::null_mut::<c_void>(),
            leaf_addr: std::ptr::null_mut::<c_void>(),
        }
    }

    /// Returns the unfiltered instruction pointer addresses captured during the stack unwind.
    pub fn frames(&self) -> &[usize] {
        &self.frames
    }

    /// Returns the root address of the task's stack.
    pub fn root_addr(&self) -> *const c_void {
        self.root_addr
    }

    /// Returns the leaf address of the task's stack.
    pub fn leaf_addr(&self) -> *const c_void {
        self.leaf_addr
    }
}

extern "C" fn callback(ctx: *mut _Unwind_Context, arg: *mut c_void) -> _Unwind_Reason_Code {
    // SAFETY: This function is only ever called by `_Unwind_Backtrace`, which guarantees:
    // 1. `ctx` is a valid, non-null pointer to an `_Unwind_Context` for the
    //    current frame being unwound.
    // 2. `arg` is the pointer we passed to `_Unwind_Backtrace`, which we know
    //    is a valid pointer to our `Vec<usize>`.
    // 3. This callback is invoked synchronously during the backtrace walk,
    //    so the pointed-to data has not been dropped or moved.
    unsafe {
        let data = &mut *(arg as *mut Vec<usize>);
        let ip = _Unwind_GetIP(ctx);

        // do this later
        // let symbol_addr = _Unwind_FindEnclosingFunction(ip as *mut c_void);
        // let below_root = !ptr::eq(symbol_addr, data.root_addr);

        data.push(ip);
        _URC_NO_REASON
    }
}

/// Captures a stack trace from a Tokio task and appends it to the provided `Vec`.
///
/// This function is meant to be called from within a [`trace_with`] callback.
/// It performs a stack unwind to collect instruction pointers, then pushes a
/// new [`TaskTrace`] (with root and leaf addresses from `meta`) onto `trace`.
///
/// # Examples
///
/// ```
/// use std::future::Future;
/// use std::task::Poll;
/// use tokio::runtime::dump::{Trace, trace_with};
/// use tokio_taskdump::{capture_trace, TaskTrace};
///
/// # #[tokio::main]
/// # async fn main() {
/// let mut fut = std::pin::pin!(async {
///     tokio::task::yield_now().await;
/// });
///
/// let mut traces = Vec::new();
///
/// Trace::root(std::future::poll_fn(|cx| {
///     trace_with(
///         || { let _ = fut.as_mut().poll(cx); },
///         |meta| { capture_trace(meta, &mut traces); },
///     );
///     Poll::Ready(())
/// }))
/// .await;
/// # }
/// ```
///
/// [`trace_with`]: tokio::runtime::dump::trace_with
pub fn capture_trace(meta: &TraceMeta, trace: &mut Vec<TaskTrace>) {
    let mut frames = vec![];

    let leaf_addr = meta.trace_leaf_addr as *mut c_void;
    let root_addr = meta.root_addr.unwrap_or(std::ptr::null()) as *mut c_void;

    unsafe {
        _Unwind_Backtrace(callback, &mut frames as *mut Vec<usize> as *mut c_void);
    }

    trace.push(TaskTrace {
        frames,
        root_addr,
        leaf_addr,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::Poll;
    use tokio::runtime::dump::{Trace, trace_with};

    // Create a chain of async functions to produce multiple frames
    async fn deep_c() {
        tokio::task::yield_now().await;
    }

    async fn deep_b() {
        deep_c().await;
    }

    async fn deep_a() {
        deep_b().await;
    }

    #[tokio::test]
    async fn test_capture_multiple_frames() {
        let mut fut = std::pin::pin!(async {
            deep_a().await;
        });

        let mut collected = Vec::new();

        Trace::root(std::future::poll_fn(|cx| {
            trace_with(
                || {
                    let _ = fut.as_mut().poll(cx);
                },
                |meta| {
                    capture_trace(meta, &mut collected);
                },
            );
            Poll::Ready(())
        }))
        .await;

        // Should have multiple frames: deep_a, deep_b, deep_c, etc.
        assert!(
            collected[0].frames().len() > 1,
            "expected multiple frames, got {}",
            collected[0].frames().len()
        );
    }

    #[tokio::test]
    async fn test_capture_single_yield() {
        let mut fut = std::pin::pin!(async {
            tokio::task::yield_now().await;
        });

        let mut collected = Vec::new();

        Trace::root(std::future::poll_fn(|cx| {
            trace_with(
                || {
                    let _ = fut.as_mut().poll(cx);
                },
                |meta| {
                    capture_trace(meta, &mut collected);
                },
            );
            Poll::Ready(())
        }))
        .await;

        assert!(!collected.is_empty(), "should capture at least one frame");
    }

    #[tokio::test]
    async fn test_no_yield_no_frames() {
        let mut fut = std::pin::pin!(async { 42 });

        let mut collected = Vec::new();

        Trace::root(std::future::poll_fn(|cx| {
            trace_with(
                || {
                    let _ = fut.as_mut().poll(cx);
                },
                |meta| {
                    capture_trace(meta, &mut collected);
                },
            );
            Poll::Ready(())
        }))
        .await;

        // No yield = trace_leaf never fires = no frames
        assert!(collected.is_empty(), "no yield should mean no frames");
    }
    #[tokio::test]
    async fn test_multiple_traces_per_poll() {
        let mut fut = std::pin::pin!(async {
            tokio::select! {
                _ = tokio::task::yield_now() => {}
                _ = tokio::task::yield_now() => {}
            }
        });

        let mut traces = Vec::new();

        Trace::root(std::future::poll_fn(|cx| {
            trace_with(
                || {
                    let _ = fut.as_mut().poll(cx);
                },
                |meta| {
                    capture_trace(meta, &mut traces);
                },
            );
            Poll::Ready(())
        }))
        .await;

        assert!(
            traces.len() == 2,
            "select! should produce multiple traces, got {}",
            traces.len()
        );
    }
}
