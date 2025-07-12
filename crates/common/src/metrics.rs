//! Metrics initialization helpers.
//!
//! Provides [`init_metrics`] which configures the OTLP exporter when
//! the `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable is set.
//!
//! The following environment variables are used:
//! - `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP collector endpoint, e.g. `http://localhost:4317`.
//! - `OTEL_SERVICE_NAME`: optional service name reported with metrics (default `fleetingdns`).

use metrics_exporter_opentelemetry::Recorder;
use opentelemetry_otlp::MetricExporterBuilder;
use opentelemetry_sdk::metrics::MeterProviderBuilder;
use opentelemetry_sdk::resource::Resource;
use tracing::info;

/// Initialize metrics export via OpenTelemetry.
///
/// When the [`OTEL_EXPORTER_OTLP_ENDPOINT`] environment variable is set, this
/// function configures the [`metrics`] crate to send data to the endpoint using
/// the OTLP protocol. If the variable is not set, metrics are disabled.
///
/// # Panics
/// Panics if the exporter fails to start.
#[tracing::instrument]
pub fn init_metrics() {
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_err() {
        info!("OTEL_EXPORTER_OTLP_ENDPOINT not set; metrics disabled");
        return;
    }

    let service = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "fleetingdns".to_string());

    let exporter = MetricExporterBuilder::new()
        .with_tonic()
        .build()
        .expect("build otlp exporter");

    let resource = Resource::builder_empty()
        .with_service_name(service.clone())
        .build();

    Recorder::builder(service)
        .with_meter_provider(|b: MeterProviderBuilder| {
            b.with_resource(resource).with_periodic_exporter(exporter)
        })
        .install_global()
        .expect("install metrics recorder");

    info!("metrics exporter initialized");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::counter;
    use opentelemetry_proto::tonic::collector::metrics::v1::metrics_service_server::{
        MetricsService, MetricsServiceServer,
    };
    use opentelemetry_proto::tonic::collector::metrics::v1::{
        ExportMetricsServiceRequest, ExportMetricsServiceResponse,
    };
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::{Request, Response, Status};

    struct MockCollector {
        tx: Mutex<Option<oneshot::Sender<ExportMetricsServiceRequest>>>,
    }

    #[tonic::async_trait]
    impl MetricsService for MockCollector {
        async fn export(
            &self,
            request: Request<ExportMetricsServiceRequest>,
        ) -> Result<Response<ExportMetricsServiceResponse>, Status> {
            if let Some(tx) = self.tx.lock().unwrap().take() {
                let _ = tx.send(request.into_inner());
            }
            Ok(Response::new(ExportMetricsServiceResponse {
                partial_success: None,
            }))
        }
    }

    #[tokio::test]
    async fn counter_exported() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(MetricsServiceServer::new(MockCollector {
                    tx: Mutex::new(Some(tx)),
                }))
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        unsafe {
            std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", format!("http://{addr}"));
            std::env::set_var("OTEL_METRIC_EXPORT_INTERVAL", "50");
        }

        init_metrics();
        let c = counter!("test_counter");
        c.increment(1);

        let req = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .expect("export timed out")
            .expect("collector dropped");

        assert!(!req.resource_metrics.is_empty());
    }
}
