# tokio-taskdump

[![CI](https://github.com/tokio-rs/tokio-taskdump/actions/workflows/ci.yml/badge.svg)](https://github.com/tokio-rs/tokio-taskdump/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/tokio-taskdump.svg)](https://crates.io/crates/tokio-taskdump)
[![docs.rs](https://docs.rs/tokio-taskdump/badge.svg)](https://docs.rs/tokio-taskdump)

Capture stack traces from Tokio tasks at their yield points.

This crate provides utilities for collecting instruction-pointer-level stack
traces from async tasks while they are suspended. It hooks into Tokio's
[`taskdump`] infrastructure to walk the stack of a task when it yields,
producing a list of frame addresses that can be symbolicated with tools like
`addr2line` or the [`backtrace`] crate.

[`taskdump`]: https://docs.rs/tokio/latest/tokio/runtime/dump/index.html
[`backtrace`]: https://docs.rs/backtrace

## Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
tokio-taskdump = "0.1"
tokio = { version = "1", features = ["rt", "taskdump"] }
```

### Quick example

```rust
use std::future::Future;
use std::task::Poll;
use tokio::runtime::dump::{Trace, trace_with};
use tokio_taskdump::{capture_trace, TaskTrace};

#[tokio::main]
async fn main() {
    let mut fut = std::pin::pin!(async {
        tokio::task::yield_now().await;
    });

    let mut traces = Vec::new();

    Trace::root(std::future::poll_fn(|cx| {
        trace_with(
            || { let _ = fut.as_mut().poll(cx); },
            |meta| { capture_trace(meta, &mut traces); },
        );
        Poll::Ready(())
    }))
    .await;

    for trace in &traces {
        println!("captured {} frames", trace.frames().len());
        for (i, addr) in trace.frames().iter().enumerate() {
            println!("  frame {i}: {addr:#x}");
        }
    }
}
```

See the [`examples/`](examples/) directory for a more complete demonstration.

## How it works

1. You wrap your future in [`Trace::root`] so Tokio knows where the task's
   stack begins.
2. Inside a poll, you call [`trace_with`] which invokes your closure and, if
   the task yields, calls your callback with a [`TraceMeta`] containing the
   root and leaf addresses.
3. [`capture_trace`] performs a stack unwind (via `_Unwind_Backtrace`) and
   appends a new [`TaskTrace`] containing the collected instruction pointer
   addresses to the provided `Vec<TaskTrace>`. If the task has multiple yield
   points (e.g., branches of a `tokio::select!`), the callback is invoked once
   per yield point, producing multiple traces in a single poll.

The resulting frame addresses span from the unwind origin up through the
task's call stack. You can then resolve them to symbols offline or at runtime.

[`Trace::root`]: https://docs.rs/tokio/latest/tokio/runtime/dump/struct.Trace.html#method.root
[`trace_with`]: https://docs.rs/tokio/latest/tokio/runtime/dump/fn.trace_with.html
[`TraceMeta`]: https://docs.rs/tokio/latest/tokio/runtime/dump/struct.TraceMeta.html
[`capture_trace`]: https://docs.rs/tokio-taskdump/latest/tokio_taskdump/fn.capture_trace.html
[`TaskTrace`]: https://docs.rs/tokio-taskdump/latest/tokio_taskdump/struct.TaskTrace.html

## Requirements

- Tokio 1.52.3+ with the `taskdump` feature enabled
- Linux (stack unwinding uses platform-specific APIs)

## License

This project is licensed under the [MIT license].

[MIT license]: https://github.com/tokio-rs/tokio-taskdump/blob/main/LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tokio by you shall be licensed as MIT, without any additional
terms or conditions.
