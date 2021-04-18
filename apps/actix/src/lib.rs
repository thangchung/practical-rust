pub mod config;
pub mod handlers;
mod errors;
mod logger;
mod middlewares;
mod routes;

extern crate chrono;
extern crate serde;

#[macro_use]
extern crate log;

use crate::config::Config;
use actix_cors::Cors;
use actix_web::middleware::{errhandlers::ErrorHandlers, Logger};
use actix_web::{http, App, HttpServer};
use actix_web_prom::PrometheusMetrics;
use color_eyre::Result;
use sqlx::{Postgres, Pool};

#[derive(Debug, Clone)]
pub struct AppState {
    pub jwt_secret_key: String,
    pub jwt_lifetime: i64,
}

pub async fn run(settings: Config, db_pool: Pool<Postgres>) -> Result<()> {
    // Logger
    // ------
    logger::init(settings.rust_log);

    // Init application state
    // ----------------------
    let data = AppState {
        jwt_secret_key: settings.jwt_secret_key.clone(),
        jwt_lifetime: settings.jwt_lifetime,
    };

    // Prometheus
    // ----------
    let prometheus = PrometheusMetrics::new("api", Some("/metrics"), None);

    // Start server
    // ------------
    HttpServer::new(move || {
        App::new()
            .data(db_pool.clone())
            .data(data.clone())
            .wrap(middlewares::request_id::RequestIdService)
            .wrap(middlewares::timer::Timer)
            .wrap(Logger::new("%s | %r | %Ts | %{User-Agent}i | %a | %{x-request-id}o"))
            .wrap(prometheus.clone())
            .wrap(
                ErrorHandlers::new()
                    .handler(http::StatusCode::UNAUTHORIZED, handlers::errors::render_401)
                    .handler(http::StatusCode::FORBIDDEN, handlers::errors::render_403)
                    .handler(http::StatusCode::REQUEST_TIMEOUT, handlers::errors::render_408)
                    .handler(http::StatusCode::BAD_GATEWAY, handlers::errors::render_502)
                    .handler(http::StatusCode::SERVICE_UNAVAILABLE, handlers::errors::render_503)
                    .handler(http::StatusCode::GATEWAY_TIMEOUT, handlers::errors::render_504),
            )
            .wrap(
                Cors::default()
                    // .allowed_origin("*")
                    .allowed_methods(vec!["GET", "POST", "PATCH", "PUT", "DELETE", "HEAD", "OPTIONS"])
                    .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
                    .allowed_header(http::header::CONTENT_TYPE)
                    .supports_credentials()
                    .max_age(3600),
            )
            .configure(routes::web)
            .configure(routes::api)
    })
    .bind(format!("{}:{}", settings.server_url, settings.server_port))?
    .run()
    .await?;

    Ok(())
}