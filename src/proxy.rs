// ProxyHttp implementation for solana-grpc-proxy.

use crate::config::Config;
use crate::grpc::{
    build_grpc_error_header, send_grpc_error, GRPC_STATUS_INVALID_ARGUMENT,
    GRPC_STATUS_PERMISSION_DENIED, GRPC_STATUS_RESOURCE_EXHAUSTED,
};
use crate::protobuf::MAX_BODY_SIZE;
use crate::rules::{load_rules_from_dir, FilterRules};
use crate::validator::validate_subscribe_request;

use async_trait::async_trait;
use bytes::Bytes;
use pingora::prelude::*;
use pingora::protocols::l4::socket::SocketAddr;
use pingora::upstreams::peer::ALPN;
use pingora_limits::inflight::{Guard, Inflight};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SUBSCRIBE_PATH: &str = "/geyser.Geyser/Subscribe";

pub struct RequestCtx {
    pub client_ip: String,
    pub rules: Option<Arc<FilterRules>>,
    pub guard: Option<Guard>,
    /// True if we already wrote a response and the request should terminate
    pub response_sent: bool,
}

impl Default for RequestCtx {
    fn default() -> Self {
        Self {
            client_ip: String::new(),
            rules: None,
            guard: None,
            response_sent: false,
        }
    }
}

pub struct SolanaGrpcProxy {
    pub config: Config,
    rules: RwLock<HashMap<String, Arc<FilterRules>>>,
    rules_dir: PathBuf,
    reload_interval: Duration,
    last_reload: AtomicU64,
    inflight: Inflight,
}

impl SolanaGrpcProxy {
    pub fn new(config: Config) -> Result<Self, String> {
        let rules_dir = config.rules_dir.clone();
        let rules = load_rules_from_dir(&rules_dir)?;
        tracing::info!(
            dir = ?rules_dir,
            count = rules.len(),
            "loaded per-IP grpc filter rules"
        );
        Ok(Self {
            reload_interval: config.reload_interval,
            config,
            rules: RwLock::new(rules),
            rules_dir,
            last_reload: AtomicU64::new(now_secs()),
            inflight: Inflight::new(),
        })
    }

    fn maybe_reload(&self) {
        let now = now_secs();
        let last = self.last_reload.load(Ordering::Relaxed);
        if now.saturating_sub(last) < self.reload_interval.as_secs() {
            return;
        }
        if self
            .last_reload
            .compare_exchange(last, now, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        match load_rules_from_dir(&self.rules_dir) {
            Ok(ip_map) => {
                if let Ok(mut m) = self.rules.write() {
                    *m = ip_map;
                }
                tracing::debug!("reloaded grpc filter rules");
            }
            Err(e) => {
                self.last_reload.store(last, Ordering::Release);
                tracing::error!(error = %e, "failed to reload grpc filter rules");
            }
        }
    }

    fn get_rules_for_ip(&self, ip: &str) -> Option<Arc<FilterRules>> {
        self.rules.read().ok()?.get(ip).cloned()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Extract client IP from X-Forwarded-For, X-Real-IP, or socket address.
fn get_client_ip(session: &Session) -> String {
    let headers = &session.req_header().headers;
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(s) = xff.to_str() {
            if let Some(first) = s.split(',').next() {
                let trimmed = first.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    if let Some(xri) = headers.get("x-real-ip") {
        if let Ok(s) = xri.to_str() {
            return s.to_string();
        }
    }
    if let Some(addr) = session.client_addr() {
        if let SocketAddr::Inet(a) = addr {
            return a.ip().to_string();
        }
    }
    String::new()
}

#[async_trait]
impl ProxyHttp for SolanaGrpcProxy {
    type CTX = RequestCtx;

    fn new_ctx(&self) -> Self::CTX {
        RequestCtx::default()
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<bool> {
        let path = session.req_header().uri.path();
        if path != SUBSCRIBE_PATH {
            // Non-Subscribe requests pass through to upstream as-is
            return Ok(false);
        }

        let ip = get_client_ip(session);
        ctx.client_ip = ip.clone();

        self.maybe_reload();

        // Check if IP has rules configured
        let Some(rules) = self.get_rules_for_ip(&ip) else {
            tracing::warn!(client_ip = %ip, "no rules configured, rejecting");
            send_grpc_error(
                session,
                GRPC_STATUS_PERMISSION_DENIED,
                &format!("no subscription rules configured for {ip}"),
            )
            .await?;
            ctx.response_sent = true;
            return Ok(true);
        };

        // Check inflight connection limit
        if rules.max_connections > 0 {
            let (guard, current) = self.inflight.incr(&ip, 1);
            if current as i64 > rules.max_connections {
                tracing::warn!(
                    client_ip = %ip,
                    current,
                    max = rules.max_connections,
                    "too many concurrent connections"
                );
                send_grpc_error(
                    session,
                    GRPC_STATUS_RESOURCE_EXHAUSTED,
                    &format!(
                        "too many concurrent connections ({current} > {})",
                        rules.max_connections
                    ),
                )
                .await?;
                ctx.response_sent = true;
                return Ok(true);
            }
            ctx.guard = Some(guard);
        }

        ctx.rules = Some(rules);
        Ok(false)
    }

    async fn request_body_filter(
        &self,
        session: &mut Session,
        body: &mut Option<Bytes>,
        _end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let Some(rules) = &ctx.rules else {
            return Ok(());
        };
        let Some(buf) = body else {
            return Ok(());
        };
        if buf.len() < 5 || buf.len() > MAX_BODY_SIZE {
            return Ok(());
        }

        let proto_buf = &buf[5..];
        if let Err(msg) = validate_subscribe_request(rules, proto_buf) {
            tracing::warn!(
                client_ip = %ctx.client_ip,
                error = %msg,
                "grpc subscribe filter rejected"
            );
            // Send gRPC trailers-only error response directly to downstream.
            let header = build_grpc_error_header(GRPC_STATUS_INVALID_ARGUMENT, &msg)?;
            session.write_response_header(header, true).await?;
            ctx.response_sent = true;
            // Drop the body so upstream doesn't receive it, then return error
            // to stop the proxy pipeline.
            *body = None;
            session.set_keepalive(None);
            return Err(Error::explain(
                ErrorType::HTTPStatus(200),
                "grpc subscribe rejected",
            ));
        }
        Ok(())
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let mut peer = HttpPeer::new(&self.config.upstream, false, String::new());
        peer.options.alpn = ALPN::H2;
        if let Some(t) = self.config.upstream_idle_timeout {
            peer.options.idle_timeout = Some(t);
        }
        if let Some(t) = self.config.upstream_connection_timeout {
            peer.options.connection_timeout = Some(t);
        }
        Ok(Box::new(peer))
    }

    async fn fail_to_proxy(
        &self,
        session: &mut Session,
        e: &Error,
        ctx: &mut Self::CTX,
    ) -> FailToProxy
    where
        Self::CTX: Send + Sync,
    {
        // If we already sent a gRPC error response, don't try to write another
        if ctx.response_sent {
            return FailToProxy {
                error_code: 200,
                can_reuse_downstream: false,
            };
        }

        // Convert any proxy error to a gRPC internal error response
        let msg = format!("{}", e);
        let _ = send_grpc_error(session, crate::grpc::GRPC_STATUS_INTERNAL, &msg).await;
        FailToProxy {
            error_code: 500,
            can_reuse_downstream: false,
        }
    }
}
