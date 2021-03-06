//! Timer middleware module

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::SystemTime;

use actix_http::http::header;
use actix_service::{Service, Transform};
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use color_eyre::Result;
use futures::future::{ok, Ready};
use futures::Future;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct Timer;

// Middleware factory is `Transform` trait from actix-service crate
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S> for Timer
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = TimerMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(TimerMiddleware { service })
    }
}

pub struct TimerMiddleware<S> {
    service: S,
}

impl<S, B> Service for TimerMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let now = SystemTime::now();
        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;

            let elapsed_result = now.elapsed();
            if let Ok(elapsed) = elapsed_result {
                if let Ok(name) = header::HeaderName::from_lowercase(b"x-process-time-s") {
                    let elapsed_sec = elapsed.as_micros() as f32 / 1_000_000f32;
                    if let Ok(value) = header::HeaderValue::from_str(&format!("{}", elapsed_sec)) {
                        res.headers_mut().insert(name, value);
                    }
                }
            }

            Ok(res)
        })
    }
}
