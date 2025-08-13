use std::sync::Arc;

use debug::{LogsManager, download_handler, handle_logs_request};
#[cfg(not(feature = "tplus"))]
use podman::handle_pods_allow;
use podman::{handle_pod_yml, handle_request_pods};

use tdx::{PodManager, handle_quote_request};
use tokio::sync::mpsc;
use tracing::{Level, info};
use warp::{Filter, reject::Reject};

mod debug;
mod podman;
mod tdx;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let (tx, rx) = mpsc::channel(10);

    let sender = Arc::new(tx.clone());
    let sender = warp::any().map(move || Arc::clone(&sender));

    let (logs_tx, logs_rx) = mpsc::channel(10);

    let logs_sender = Arc::new(logs_tx.clone());
    let logs_sender = warp::any().map(move || Arc::clone(&logs_sender));

    tokio::spawn(async {
        let mut manager = PodManager::new(rx);
        manager.worker().await;
    });

    tokio::spawn(async {
        let mut manager = LogsManager::new(logs_rx);
        manager.worker(logs_tx).await;
    });

    #[cfg(not(feature = "tplus"))]
    let post_allowed_pods = warp::post()
        .and(warp::path!("pods" / "allow"))
        .and(sender.clone())
        .and(warp::body::json())
        .and_then(handle_pods_allow);

    #[cfg(feature = "tplus")]
    #[cfg(feature = "tplus")]
    let post_allowed_pods =
        warp::any().and_then(|| async move { Ok::<_, warp::Rejection>("disabled") });

    let pods = warp::post()
        .and(warp::path!("pods" / "deploy"))
        .and(sender.clone())
        .and(warp::body::bytes())
        .and_then(handle_pod_yml);

    let list_pods = warp::get()
        .and(warp::path("pods"))
        .and(sender.clone())
        .and_then(handle_request_pods);

    let get_quote = warp::get()
        .and(warp::path!("quote" / String))
        .and(sender)
        .and_then(handle_quote_request);

    let status = warp::get()
        .and(warp::path("status"))
        .map(|| warp::reply::json(&serde_json::json!({"status": "ok"})));

    let download_logs = warp::path!("logs")
        .and(warp::get())
        .and_then(download_handler);

    let get_logs = warp::post()
        .and(warp::path!("logs" / "dump"))
        .and(logs_sender)
        .and_then(handle_logs_request);

    info!("Server running at http://0.0.0.0:3030");
    let routes = pods
        .or(post_allowed_pods)
        .or(status)
        .or(get_quote)
        .or(list_pods)
        .or(download_logs)
        .or(get_logs);
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

#[derive(Debug)]
pub struct Wrapper(pub String);

impl Reject for Wrapper {}
