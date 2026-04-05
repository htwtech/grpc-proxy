mod config;
mod grpc;
mod protobuf;
mod proxy;
mod rules;
mod validator;

use clap::Parser;
use config::Config;
use pingora::apps::HttpServerOptions;
use pingora::prelude::*;
use proxy::SolanaGrpcProxy;

#[derive(Parser, Debug)]
#[command(name = "solana-grpc-proxy")]
#[command(about = "Pingora-based gRPC proxy for Yellowstone/Solana with per-IP filters")]
struct Args {
    /// Path to config.toml
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

fn main() {
    let args = Args::parse();

    let config = match Config::load(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load config: {e}");
            std::process::exit(1);
        }
    };

    // Init tracing
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();

    tracing::info!(
        listen = %config.listen,
        upstream = %config.upstream,
        rules_dir = ?config.rules_dir,
        "starting solana-grpc-proxy"
    );

    let proxy = match SolanaGrpcProxy::new(config.clone()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to init proxy: {e}");
            std::process::exit(1);
        }
    };

    let mut server = Server::new(None).expect("create pingora server");
    server.bootstrap();

    let listen_addr = config.listen.clone();
    let mut service = http_proxy_service(&server.configuration, proxy);

    // Enable h2c (HTTP/2 cleartext) — required for gRPC without TLS
    if let Some(http_logic) = service.app_logic_mut() {
        let mut http_server_options = HttpServerOptions::default();
        http_server_options.h2c = true;
        http_logic.server_options = Some(http_server_options);
    }

    service.add_tcp(&listen_addr);

    server.add_service(service);
    server.run_forever();
}
