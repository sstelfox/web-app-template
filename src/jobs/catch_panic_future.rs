use std::fmt::{self, Debug, Display, Formatter};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use futures::{Future, FutureExt};

pub struct CatchPanicFuture<F: Future + Send + 'static> {
    inner: BoxFuture<'static, F::Output>,
}

impl<F: Future + Send + 'static> CatchPanicFuture<F> {
    pub fn wrap(f: F) -> Self {
        Self { inner: f.boxed() }
    }
}

impl<F: Future + Send + 'static> Future for CatchPanicFuture<F> {
    type Output = Result<F::Output, CaughtPanic>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = &mut self.inner;

        match local_catch_unwind(move || inner.poll_unpin(cx)) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(value)) => Poll::Ready(Ok(value)),
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

#[derive(Debug)]
pub struct CaughtPanic(String);

impl Display for CaughtPanic {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "panicked message: {}", self.0)
    }
}

impl std::error::Error for CaughtPanic {}

fn local_catch_unwind<F: FnOnce() -> R, R>(f: F) -> Result<R, CaughtPanic> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(res) => Ok(res),
        Err(panic_err) => {
            if let Some(msg) = panic_err.downcast_ref::<&'static str>() {
                return Err(CaughtPanic(msg.to_string()));
            }

            if let Some(msg) = panic_err.downcast_ref::<String>() {
                return Err(CaughtPanic(msg.to_string()));
            }

            Err(CaughtPanic("unknown panic message format".to_string()))
        }
    }
}
