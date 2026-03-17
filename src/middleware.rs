use std::{pin::Pin, sync::Arc, task::{Context, Poll}, time::Instant};

use crossbeam::queue::ArrayQueue;
use http::{Request, Response};
use tokio::task::JoinHandle;
use tower::{Layer, Service};

use colored::{Color, Colorize};

pub fn middleware(cap: usize) -> (JoinHandle<()> ,EventLayer<Event>) {
    let bus: EventBus<Event> = EventBus::new(cap);
    let queue = Arc::downgrade(&bus.queue);
    let handle =  tokio::spawn(async move {
        loop {
            match queue.upgrade() {
                Some(queue) => {
                    if let Some(event) = queue.pop() {
                        let event = event.format();
                        println!("{}", event);
                    } else {
                        tokio::task::yield_now().await
                    }
                }
                None => break,
            }
        }
    });
    (handle, EventLayer { bus })
}

pub struct EventBus<E> {
    queue: Arc<ArrayQueue<E>>
}

impl<E> Clone for EventBus<E> {
    fn clone(&self) -> Self {
        EventBus {
            queue: self.queue.clone()
        }
    }
}

impl<E> EventBus<E> {
    pub fn new(cap: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(cap)),
        }
    }

    pub fn push(&self, event: E) {
        let _ = self.queue.push(event);
    }

    pub fn queue(&self) -> Arc<ArrayQueue<E>> {
        self.queue.clone()
    }
}

#[derive(Clone)]
pub struct Event {
    method: http::Method,
    uri: http::Uri,
    status: http::StatusCode,
    latency: std::time::Duration,
}

impl Event {
    pub fn format(&self) -> String {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string()
            .color(Color::BrightBlack);
        let color = match self.status.as_u16() {
            200..=299 => Color::Green,
            400..=499 => Color::Red,
            _ => Color::Magenta
        };
        format!(
            "{} {} [{}] {} {}",
            timestamp,
            format!("{:5}", self.method).bold(),
            self.status.as_str().color(color).bold(),
            self.uri.path(),
            format!("(in {:.2}s)", self.latency.as_secs_f32())
                .color(Color::BrightBlack),
        )
    }
}

#[derive(Clone)]
pub struct EventLayer<E> {
    bus: EventBus<E>
}

impl<S, E> Layer<S> for EventLayer<E>
where E: Clone
{
    type Service = EventService<S, E>;

    fn layer(&self, inner: S) -> Self::Service {
        EventService {
            inner: inner,
            bus: self.bus.clone(),
        }
    }
}

#[derive(Clone)]
pub struct EventService<S, E> {
    pub(crate) inner: S,
    pub(crate) bus: EventBus<E>,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for EventService<S, Event>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = Response<ResBody>;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let method = req.method().clone();
        let uri = req.uri().clone();
        let start = Instant::now();

        let fut = self.inner.call(req);

        ResponseFuture {
            fut,
            method,
            uri,
            start,
            bus: self.bus.clone(),
        }
    }
}

#[pin_project::pin_project]
pub struct ResponseFuture<F> {
    #[pin]
    fut: F,
    method: http::Method,
    uri: http::Uri,
    start: Instant,
    bus: EventBus<Event>
}

impl<F, ResBody, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<ResBody>, E>>,
{
    type Output = Result<Response<ResBody>, E>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let this = self.project();

        match this.fut.poll(cx) {
            Poll::Ready(Ok(response)) => {
                let latency = this.start.elapsed();
                let status = response.status();

                let event = Event {
                    method: this.method.clone(),
                    uri: this.uri.clone(),
                    status,
                    latency,
                };
                this.bus.push(event);

                Poll::Ready(Ok(response))
            }

            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}