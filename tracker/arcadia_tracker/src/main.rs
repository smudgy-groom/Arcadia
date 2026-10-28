use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    web::Data,
    App, Error, HttpServer,
};
use arcadia_tracker::{api_doc::ApiDoc, config::Config, routes::init, scheduler, Tracker};
use tracing_actix_web::{DefaultRootSpanBuilder, RootSpanBuilder, TracingLogger};
use tracing_actix_web::root_span_macro::private::tracing;

/// Root span builder that keeps every field of the default builder except
/// `http.target`.
///
/// BitTorrent clients put their passkey in the announce URL, so the request
/// target contains a long lived credential. Logging it (and exporting it to the
/// telemetry pipeline) leaks the passkeys of every member that announces.
struct RedactedRootSpanBuilder;

impl RootSpanBuilder for RedactedRootSpanBuilder {
    fn on_request_start(request: &ServiceRequest) -> tracing::Span {
        use tracing_actix_web::root_span_macro::private::{
            get_request_id, http_flavor, http_method_str, http_scheme,
        };

        let connection_info = request.connection_info();
        let http_method = http_method_str(request.method());
        let http_route = request
            .match_pattern()
            .unwrap_or_else(|| "default".to_string());
        let user_agent = request
            .headers()
            .get("User-Agent")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        let request_id = get_request_id(request);

        tracing::info_span!(
            "HTTP request",
            http.method = %http_method,
            http.route = %http_route,
            http.flavor = %http_flavor(request.version()),
            http.scheme = %http_scheme(connection_info.scheme()),
            http.host = %connection_info.host(),
            http.client_ip = %connection_info.realip_remote_addr().unwrap_or(""),
            http.user_agent = %user_agent,
            http.status_code = tracing::field::Empty,
            otel.name = %format!("{} {}", http_method, http_route),
            otel.kind = "server",
            otel.status_code = tracing::field::Empty,
            request_id = %request_id,
            exception.message = tracing::field::Empty,
            exception.details = tracing::field::Empty,
        )
    }

    fn on_request_end<B: MessageBody>(
        span: tracing::Span,
        outcome: &Result<ServiceResponse<B>, Error>,
    ) {
        DefaultRootSpanBuilder::on_request_end(span, outcome);
    }
}
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// Name reported to the telemetry collector for this service.
const OTEL_SERVICE_NAME: &str = "arcadia-tracker";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = arcadia_shared::config::load::<Config>();

    arcadia_shared::telemetry::init_telemetry(
        &config.telemetry,
        &config.tracker.log_level,
        OTEL_SERVICE_NAME,
    );

    let telemetry_enabled = config.telemetry.otlp_endpoint.is_some();
    let server_url = format!("{}:{}", config.tracker.host, config.tracker.port);
    println!("Server running at http://{server_url}");

    let arc = Data::new(Tracker::new(config).await);

    if telemetry_enabled {
        arcadia_tracker::metrics::register(&arc, OTEL_SERVICE_NAME);
    } else {
        log::info!("telemetry.otlp_endpoint is not set, skipping metrics registration");
    }

    // Starts scheduler to automate flushing updates
    // to database and inactive peer removal.
    let scheduler_handle = tokio::spawn({
        let arc = arc.clone();

        async move {
            scheduler::handle(&arc).await;
        }
    });

    let arc2 = arc.clone();
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::<RedactedRootSpanBuilder>::new())
            .app_data(arc2.clone())
            .configure(init) // Initialize routes
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/swagger-json/openapi.json", ApiDoc::openapi()),
            )
    })
    .bind(server_url)?
    .run();

    server.await?;

    // Flush all remaining updates before shutting down.
    let max_flushes = 1000;
    let mut flushes = 0;

    // stop the scheduler to avoid race conditions
    scheduler_handle.abort();

    while flushes < max_flushes
        && (!arc.peer_updates.lock().is_empty()
            || !arc.torrent_updates.lock().is_empty()
            || !arc.user_updates.lock().is_empty()
            || !arc.announce_error_updates.lock().is_empty())
    {
        scheduler::flush(&arc).await;
        flushes += 1;
    }

    if flushes == max_flushes {
        log::error!("Graceful shutdown failed");
    } else {
        log::info!("Graceful shutdown succeeded");
    }

    // Blocking flush, kept off the runtime workers that drive the gRPC exporters
    let _ = tokio::task::spawn_blocking(arcadia_shared::telemetry::shutdown_telemetry).await;

    Ok(())
}
