// Minimal Prometheus-compatible metrics + health endpoint.
//
// Only exposes build info — no counters, no gauges touching the hot path.
// The metrics body is a compile-time `const &str` assembled via `concat!` +
// `env!`, so the response has zero runtime formatting overhead.

use async_trait::async_trait;
use http::Response;
use pingora::apps::http_app::ServeHttp;
use pingora::protocols::http::ServerSession;

pub struct MetricsApp;

/// Pre-rendered Prometheus metrics body with build information.
///
/// Assembled at compile time from Cargo version and git short hash
/// (set by build.rs).
const METRICS_BODY: &str = concat!(
    "# HELP proxy_info Build information\n",
    "# TYPE proxy_info gauge\n",
    "proxy_info{version=\"",
    env!("CARGO_PKG_VERSION"),
    "\",build=\"",
    env!("BUILD_GIT_HASH"),
    "\"} 1\n",
);

#[async_trait]
impl ServeHttp for MetricsApp {
    async fn response(&self, session: &mut ServerSession) -> Response<Vec<u8>> {
        let path = session.req_header().uri.path();
        match path {
            "/metrics" => Response::builder()
                .status(200)
                .header(
                    "content-type",
                    "text/plain; version=0.0.4; charset=utf-8",
                )
                .body(METRICS_BODY.as_bytes().to_vec())
                .unwrap(),
            "/health" => Response::builder()
                .status(200)
                .header("content-type", "text/plain")
                .body(b"ok\n".to_vec())
                .unwrap(),
            _ => Response::builder()
                .status(404)
                .header("content-type", "text/plain")
                .body(b"not found\n".to_vec())
                .unwrap(),
        }
    }
}
