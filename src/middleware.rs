use std::{pin::Pin, sync::Arc, task::{Context, Poll}, time::Instant};

use crate::EventBus;
use http::{Request, Response};
use tokio::task::JoinHandle;
use tower::{Layer, Service};

use colored::{self, Color, Colorize};

/// Middleware for logging HTTP requests.  
/// 
/// It is started as a independent async task, automatically closing when the application is closed. 
/// 
/// Can be used together with [`li_logger::init()`]
/// # Example
/// ```rust
/// use li_logger::{middleware, default_formatter};
/// use axum::{Json, Router, routing::get};
/// 
/// #[tokio::main]
/// async fn main() {
/// 
///     let (handle, ware) = middleware::middleware(100, default_formatter);
/// 
///     let app = Router::new()
///         .route("/", get(|| async { "Hello, World!" }))
///         .layer(ware);
///     let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
///     axum::serve(listener, app)
///         .with_graceful_shutdown(async move {
///             tokio::signal::ctrl_c().await.ok();
///         })
///         .await.unwrap();
///     handle.await.unwrap();
/// }
/// ```
pub fn middleware(cap: usize, formatter: fn(&Event) -> String) -> (JoinHandle<()> ,EventLayer<Event>) {
    let bus = EventBus::<Event>::new(cap);
    let queue = Arc::downgrade(&bus.queue);
    let handle =  tokio::spawn(async move {
        loop {
            match queue.upgrade() {
                Some(queue) => {
                    if let Some(event) = queue.pop() {
                        let event = formatter(&event);
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

#[derive(Clone)]
pub struct Event {
    method: http::Method,
    uri: http::Uri,
    status: http::StatusCode,
    latency: std::time::Duration,
}

/// Default formatter for [`middleware()`]
/// 
/// Example: `2026-3-18 21:50:29 [200] GET /api/v1/users in 0.01s`
/// 
/// You may customize the formatter by implementing a `fn(event: &Event) -> String` and pass it to [`middleware()`]
pub fn default_formatter(event: &Event) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
        .color(Color::BrightBlack);
    let color = match event.status.as_u16() {
        200..=299 => Color::Green,
        400..=499 => Color::Red,
        _ => Color::Magenta
    };
    format!(
        "{} [{}] {} {} {}",
        timestamp,
        event.status.as_str().color(color).bold(),
        format!("{:>5}", event.method).bold(),
        event.uri.path(),
        format!("in {:.2}s", event.latency.as_secs_f32())
            .color(Color::BrightBlack),
    )
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