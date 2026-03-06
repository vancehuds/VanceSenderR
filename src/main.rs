//! VanceSender — FiveM text sender with AI generation.
//!
//! Entry point: CLI parsing, config loading, HTTP server + native GUI.

mod api;
mod app_meta;
mod config;
mod core;
mod desktop;
mod error;
mod gui;
mod state;

use std::sync::Arc;

use clap::Parser;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

use crate::app_meta::{APP_NAME, APP_VERSION};
use crate::state::AppState;

#[derive(Parser)]
#[command(name = "vancesender", about = "VanceSender — FiveM text sender")]
struct Cli {
    /// Port to listen on
    #[arg(short, long, default_value_t = 0)]
    port: u16,

    /// Listen on all interfaces (LAN access)
    #[arg(long)]
    lan: bool,

    /// Run without native GUI (headless server mode)
    #[arg(long)]
    no_gui: bool,
}

fn main() {
    // Tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // Load config
    let cfg = config::load_config();

    // Resolve host/port
    let cfg_host = config::get_str(&cfg, "server", "host").to_string();
    let cfg_port = config::get_i64(&cfg, "server", "port", 8730) as u16;
    let cfg_lan = config::get_bool(&cfg, "server", "lan_access");

    let host = if cli.lan || cfg_lan {
        "0.0.0.0".to_string()
    } else if cfg_host.is_empty() {
        "127.0.0.1".to_string()
    } else {
        cfg_host
    };
    let port = if cli.port > 0 { cli.port } else { cfg_port };
    let lan_access = cli.lan || cfg_lan;

    // Shared state
    let app_state = Arc::new(AppState::new());
    {
        *app_state.runtime_host.write() = host.clone();
        *app_state.runtime_port.write() = port;
        *app_state.runtime_lan_access.write() = lan_access;

        if lan_access {
            *app_state.runtime_lan_ips.write() =
                crate::core::network::get_lan_ipv4_addresses();
        }
    }

    // Ensure data directories exist
    let _ = std::fs::create_dir_all(config::data_dir());
    let _ = std::fs::create_dir_all(config::presets_dir());
    let _ = std::fs::create_dir_all(config::ai_history_dir());

    // Banner
    print_banner(&host, port, lan_access, &app_state.runtime_lan_ips.read());

    // Build tokio runtime for HTTP server
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    let tokio_handle = rt.handle().clone();
    let state_for_server = app_state.clone();
    let host_for_server = host.clone();

    // Port guard — check if port is available
    match crate::core::port_guard::ensure_startup_port_available(&host, port) {
        Ok(()) => {}
        Err(e) => {
            tracing::error!("端口检查失败: {e}");
            eprintln!("❌ {e}");
            std::process::exit(1);
        }
    }

    // Start HTTP server in background
    rt.spawn(async move {
        run_http_server(state_for_server, &host_for_server, port).await;
    });

    if cli.no_gui {
        // Headless mode — block on runtime
        tracing::info!("Running in headless mode (no GUI)");
        rt.block_on(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen for ctrl-c");
            tracing::info!("Shutting down...");
        });
    } else {
        // Launch native GUI on main thread (pass tokio handle for async bridging)
        tracing::info!("Starting native GUI...");
        if let Err(e) = gui::run_gui(app_state.clone(), tokio_handle) {
            tracing::error!("GUI error: {e}");
        }
    }

    // Flush stats on exit
    app_state.stats.write().flush();
    tracing::info!("VanceSender exited.");
}

async fn run_http_server(state: state::SharedState, host: &str, port: u16) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Static file serving for WebUI
    let web_dir = config::exe_dir_public().join("app").join("web");
    let static_service = ServeDir::new(&web_dir);

    let app = api::build_router(state)
        .fallback_service(static_service)
        .layer(cors);

    let addr: std::net::SocketAddr = format!("{host}:{port}")
        .parse()
        .expect("invalid bind address");

    tracing::info!("HTTP server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app)
        .await
        .expect("server error");
}

fn print_banner(host: &str, port: u16, lan_access: bool, lan_ips: &[String]) {
    println!();
    println!("  ╔══════════════════════════════════════════╗");
    println!("  ║  ⚡ {APP_NAME} v{APP_VERSION}               ║");
    println!("  ║  FiveM /me /do 文本发送器 + AI 生成       ║");
    println!("  ╚══════════════════════════════════════════╝");
    println!();
    println!("  🌐 WebUI: http://{host}:{port}");
    if lan_access && !lan_ips.is_empty() {
        for ip in lan_ips {
            println!("  📱 LAN:   http://{ip}:{port}");
        }
    }
    println!();
}
