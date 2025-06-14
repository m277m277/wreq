use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::future::BoxFuture;
use pin_project_lite::pin_project;
use tokio::time::Sleep;
use tower::{BoxError, Layer, Service};

/// This tower layer injects an arbitrary delay before calling downstream layers.
#[derive(Clone)]
pub struct DelayLayer {
    delay: Duration,
}

impl DelayLayer {
    #[allow(unused)]
    pub const fn new(delay: Duration) -> Self {
        DelayLayer { delay }
    }
}

impl<S> Layer<S> for DelayLayer {
    type Service = Delay<S>;
    fn layer(&self, service: S) -> Self::Service {
        Delay::new(service, self.delay)
    }
}

impl std::fmt::Debug for DelayLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("DelayLayer")
            .field("delay", &self.delay)
            .finish()
    }
}

/// This tower service injects an arbitrary delay before calling downstream layers.
#[derive(Debug, Clone)]
pub struct Delay<S> {
    inner: S,
    delay: Duration,
}
impl<S> Delay<S> {
    pub fn new(inner: S, delay: Duration) -> Self {
        Delay { inner, delay }
    }
}

impl<S, Request> Service<Request> for Delay<S>
where
    S: Service<Request>,
    S::Error: Into<BoxError>,
{
    type Response = S::Response;

    type Error = BoxError;

    type Future = ResponseFuture<S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        println!("Delay::poll_ready called");
        match self.inner.poll_ready(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => Poll::Ready(r.map_err(Into::into)),
        }
    }

    fn call(&mut self, req: Request) -> Self::Future {
        println!("Delay::call executed");
        let response = self.inner.call(req);
        let sleep = tokio::time::sleep(self.delay);

        ResponseFuture::new(response, sleep)
    }
}

// `Delay` response future
pin_project! {
    #[derive(Debug)]
    pub struct ResponseFuture<S> {
        #[pin]
        response: S,
        #[pin]
        sleep: Sleep,
    }
}

impl<S> ResponseFuture<S> {
    pub(crate) fn new(response: S, sleep: Sleep) -> Self {
        ResponseFuture { response, sleep }
    }
}

impl<F, S, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<S, E>>,
    E: Into<BoxError>,
{
    type Output = Result<S, BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        // First poll the sleep until complete
        match this.sleep.poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(_) => {}
        }

        // Then poll the inner future
        match this.response.poll(cx) {
            Poll::Ready(v) => Poll::Ready(v.map_err(Into::into)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Clone)]
pub struct PrintPollReadyLayer(pub &'static str);

impl<S> Layer<S> for PrintPollReadyLayer {
    type Service = PrintPollReady<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PrintPollReady::new(inner, self.0)
    }
}

#[derive(Clone)]
pub struct PrintPollReady<S> {
    inner: S,
    name: &'static str,
}

impl<S> PrintPollReady<S> {
    pub fn new(inner: S, name: &'static str) -> Self {
        Self { inner, name }
    }
}

impl<S, Request> Service<Request> for PrintPollReady<S>
where
    S: Service<Request> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        println!("[{}] poll_ready called", self.name);
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        println!("[{}] call executed", self.name);
        self.inner.call(req)
    }
}

#[derive(Clone)]
pub struct SharedConcurrencyLimitLayer {
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
}

impl SharedConcurrencyLimitLayer {
    #[allow(unused)]
    pub fn new(limit: usize) -> Self {
        Self {
            semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(limit)),
        }
    }
}

impl<S> tower::Layer<S> for SharedConcurrencyLimitLayer {
    type Service = SharedConcurrencyLimit<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SharedConcurrencyLimit {
            inner,
            semaphore: self.semaphore.clone(),
        }
    }
}

#[derive(Clone)]
pub struct SharedConcurrencyLimit<S> {
    inner: S,
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
}

impl<S, Req> tower::Service<Req> for SharedConcurrencyLimit<S>
where
    S: tower::Service<Req> + Clone + Send + 'static,
    S::Future: Send + 'static,
    Req: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        // always ready, we handle limits in call
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let semaphore = self.semaphore.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let _permit = semaphore.acquire_owned().await.unwrap();
            inner.call(req).await
        })
    }
}
