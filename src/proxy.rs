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
use pingora::proxy::FailToProxy;
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
    #[allow(dead_code)]
    pub guard: Option<Guard>,
    /// True if we already wrote a response and the request should terminate
    pub response_sent: bool,
    /// Buffer for partial gRPC frames that span multiple DATA frames.
    /// We accumulate bytes until we have a complete gRPC message, then
    /// validate it. Every SubscribeRequest in the stream is validated —
    /// no early-exit.
    pub pending_frame: bytes::BytesMut,
    /// Per-stream churn counter: number of SubscribeRequest updates
    /// received within the current churn window.
    pub churn_count: u32,
    /// When the current churn window started (None = no updates yet).
    pub churn_window_start: Option<std::time::Instant>,
}

impl Default for RequestCtx {
    fn default() -> Self {
        Self {
            client_ip: String::new(),
            rules: None,
            guard: None,
            response_sent: false,
            pending_frame: bytes::BytesMut::new(),
            churn_count: 0,
            churn_window_start: None,
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

/// Send a gRPC error response and return an error that aborts the request.
/// Used from within request_body_filter. The RST_STREAM to upstream is
/// emitted by our patched proxy_h2.rs when it sees the returned Err.
async fn reject_body(
    session: &mut Session,
    ctx: &mut RequestCtx,
    body: &mut Option<Bytes>,
    grpc_status: u8,
    msg: &str,
) -> Result<()> {
    let header = build_grpc_error_header(grpc_status, msg)?;
    let _ = session.write_response_header(header, true).await;
    ctx.response_sent = true;
    *body = None;
    session.set_keepalive(None);
    Err(Error::because(
        ErrorType::ConnectionClosed,
        "grpc subscribe rejected",
        Error::new(ErrorType::HTTPStatus(200)),
    ))
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

    /// Hook added by our pingora fork: called after upstream TCP connect but
    /// BEFORE request headers are forwarded upstream.
    ///
    /// We used to read the body here (for grpcurl-style clients that send
    /// DATA immediately), but calling `read_request_body()` with a timeout
    /// appears to put h2::RecvStream into a state where subsequent reads
    /// in the main proxy loop don't fire, breaking streaming clients.
    ///
    /// Now this hook is a no-op stub. All validation happens in
    /// `request_body_filter` when the client actually sends DATA frames —
    /// which works for both grpcurl (sends immediately) and tonic (sends
    /// after receiving server response headers).
    async fn pre_upstream_body_filter(
        &self,
        _session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<bool>
    where
        Self::CTX: Send + Sync,
    {
        Ok(false)
    }

    async fn request_body_filter(
        &self,
        session: &mut Session,
        body: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let Some(rules) = ctx.rules.clone() else {
            return Ok(());
        };
        let Some(buf) = body.as_ref() else {
            return Ok(());
        };
        if buf.is_empty() {
            return Ok(());
        }

        // Append the incoming chunk to the pending frame buffer. This handles
        // the case where a single SubscribeRequest is split across multiple
        // DATA frames, or multiple small SubscribeRequests arrive in one frame.
        ctx.pending_frame.extend_from_slice(buf);

        // Enforce a hard upper bound on buffered data to prevent memory abuse.
        if ctx.pending_frame.len() > MAX_BODY_SIZE * 4 {
            tracing::warn!(
                client_ip = %ctx.client_ip,
                size = ctx.pending_frame.len(),
                "pending frame buffer exceeded limit"
            );
            return reject_body(
                session,
                ctx,
                body,
                GRPC_STATUS_RESOURCE_EXHAUSTED,
                "grpc frame too large",
            )
            .await;
        }

        // Parse and validate every complete gRPC frame in the buffer.
        // gRPC frame format: [1 byte compressed][4 bytes length BE][N bytes protobuf]
        loop {
            if ctx.pending_frame.len() < 5 {
                break; // Incomplete header
            }
            let len = u32::from_be_bytes([
                ctx.pending_frame[1],
                ctx.pending_frame[2],
                ctx.pending_frame[3],
                ctx.pending_frame[4],
            ]) as usize;
            let total_frame_size = 5 + len;
            if ctx.pending_frame.len() < total_frame_size {
                break; // Incomplete body
            }
            if len > MAX_BODY_SIZE {
                return reject_body(
                    session,
                    ctx,
                    body,
                    GRPC_STATUS_RESOURCE_EXHAUSTED,
                    "grpc message too large",
                )
                .await;
            }

            let compressed = ctx.pending_frame[0];
            let proto_buf = ctx.pending_frame[5..total_frame_size].to_vec();

            // Reject compressed frames — we can't parse them
            if compressed != 0 {
                return reject_body(
                    session,
                    ctx,
                    body,
                    GRPC_STATUS_INVALID_ARGUMENT,
                    "compressed gRPC frames not supported",
                )
                .await;
            }

            tracing::debug!(
                client_ip = %ctx.client_ip,
                frame_len = len,
                end_of_stream,
                churn_max = rules.max_updates_per_window,
                churn_count = ctx.churn_count,
                "validating SubscribeRequest frame"
            );

            if let Err(msg) = validate_subscribe_request(&rules, &proto_buf) {
                tracing::warn!(
                    client_ip = %ctx.client_ip,
                    error = %msg,
                    "grpc subscribe filter rejected"
                );
                return reject_body(
                    session,
                    ctx,
                    body,
                    GRPC_STATUS_INVALID_ARGUMENT,
                    &msg,
                )
                .await;
            }

            // Per-stream churn protection: limit how often a client can
            // update their subscription within a sliding window.
            if rules.max_updates_per_window > 0 {
                let now = std::time::Instant::now();
                let window = std::time::Duration::from_secs(rules.churn_window_secs.max(1) as u64);
                match ctx.churn_window_start {
                    None => {
                        ctx.churn_window_start = Some(now);
                        ctx.churn_count = 1;
                    }
                    Some(start) if now.duration_since(start) >= window => {
                        // Reset window
                        ctx.churn_window_start = Some(now);
                        ctx.churn_count = 1;
                    }
                    Some(_) => {
                        ctx.churn_count += 1;
                        if ctx.churn_count as i64 > rules.max_updates_per_window {
                            tracing::warn!(
                                client_ip = %ctx.client_ip,
                                count = ctx.churn_count,
                                max = rules.max_updates_per_window,
                                window_secs = rules.churn_window_secs,
                                "subscription churn limit exceeded"
                            );
                            return reject_body(
                                session,
                                ctx,
                                body,
                                GRPC_STATUS_RESOURCE_EXHAUSTED,
                                &format!(
                                    "subscription churn limit: {} updates in {}s",
                                    ctx.churn_count, rules.churn_window_secs
                                ),
                            )
                            .await;
                        }
                    }
                }
            }

            // Consume the validated frame from the buffer
            let _ = ctx.pending_frame.split_to(total_frame_size);
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
        // If we already sent a gRPC error response (from request_body_filter),
        // do nothing — the response is already on the wire.
        if ctx.response_sent {
            return FailToProxy {
                error_code: 0, // 0 = connection dead, don't try to write
                can_reuse_downstream: false,
            };
        }

        // For real proxy errors (upstream down, etc), send a gRPC internal error.
        let msg = format!("{}", e);
        let _ = send_grpc_error(session, crate::grpc::GRPC_STATUS_INTERNAL, &msg).await;
        FailToProxy {
            error_code: 500,
            can_reuse_downstream: false,
        }
    }
}
