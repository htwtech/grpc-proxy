// gRPC response helpers for trailers-only error responses.
//
// gRPC error semantics over HTTP/2:
//   - HTTP status 200
//   - content-type: application/grpc
//   - grpc-status: <numeric code> (in HEADERS frame with END_STREAM flag)
//   - grpc-message: <error text>
//
// This is a "trailers-only" response (RFC: gRPC over HTTP2 spec).

use http::header::HeaderValue;
use pingora::http::ResponseHeader;
use pingora::proxy::Session;
use pingora::Result;

// gRPC status codes (https://grpc.io/docs/guides/status-codes/)
#[allow(dead_code)]
pub const GRPC_STATUS_OK: u8 = 0;
pub const GRPC_STATUS_INVALID_ARGUMENT: u8 = 3;
pub const GRPC_STATUS_PERMISSION_DENIED: u8 = 7;
pub const GRPC_STATUS_RESOURCE_EXHAUSTED: u8 = 8;
pub const GRPC_STATUS_INTERNAL: u8 = 13;

/// Build a trailers-only gRPC error response header.
/// Returns ResponseHeader ready to be written with end_of_stream=true.
pub fn build_grpc_error_header(
    grpc_status: u8,
    message: &str,
) -> Result<Box<ResponseHeader>> {
    let mut header = ResponseHeader::build(200, Some(3))?;
    header.insert_header("content-type", "application/grpc")?;
    header.insert_header("grpc-status", grpc_status.to_string())?;
    // Sanitize message: strip non-printable chars to keep header valid
    let sanitized: String = message
        .chars()
        .filter(|c| c.is_ascii() && !c.is_ascii_control())
        .collect();
    header.insert_header(
        "grpc-message",
        HeaderValue::from_str(&sanitized)
            .unwrap_or_else(|_| HeaderValue::from_static("rejected")),
    )?;
    Ok(Box::new(header))
}

/// Send a complete trailers-only gRPC error response to the downstream.
/// This writes the response header with end_of_stream=true, which sends a
/// HEADERS frame with the END_STREAM flag — the canonical gRPC "trailers-only"
/// error response format.
pub async fn send_grpc_error(
    session: &mut Session,
    grpc_status: u8,
    message: &str,
) -> Result<()> {
    let header = build_grpc_error_header(grpc_status, message)?;
    session.write_response_header(header, true).await?;
    Ok(())
}
