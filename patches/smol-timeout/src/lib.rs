use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use async_io::Timer;

pub trait TimeoutExt: Future {
    fn timeout(self, duration: Duration) -> Timeout<Self>
    where
        Self: Sized,
    {
        Timeout {
            future: self,
            timer: Timer::after(duration),
        }
    }
}

impl<F: Future> TimeoutExt for F {}

pub struct Timeout<F> {
    future: F,
    timer: Timer,
}

impl<F: Future> Future for Timeout<F> {
    type Output = Option<F::Output>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // Safety: we never move the future or timer after they're pinned
        let this = unsafe { self.get_unchecked_mut() };
        let future = unsafe { Pin::new_unchecked(&mut this.future) };
        let timer = unsafe { Pin::new_unchecked(&mut this.timer) };

        if let std::task::Poll::Ready(val) = future.poll(cx) {
            return std::task::Poll::Ready(Some(val));
        }
        if let std::task::Poll::Ready(_) = timer.poll(cx) {
            return std::task::Poll::Ready(None);
        }
        std::task::Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use async_io::block_on;

    #[test]
    fn test_timeout_completes() {
        block_on(async {
            let future = Timer::after(Duration::from_millis(10));
            let result = future.timeout(Duration::from_millis(100)).await;
            assert!(result.is_some());
        });
    }

    #[test]
    fn test_timeout_expires() {
        block_on(async {
            let future = Timer::after(Duration::from_millis(100));
            let result = future.timeout(Duration::from_millis(10)).await;
            assert!(result.is_none());
        });
    }
}
