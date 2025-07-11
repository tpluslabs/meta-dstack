use std::sync::Arc;

use podman::{handle_pod_yml, handle_request_pods};
use tdx::{PodManager, handle_quote_request};
use tokio::sync::mpsc;
use tracing::{Level, info};
use warp::{Filter, reject::Reject};

mod podman;
mod tdx;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let (tx, rx) = mpsc::channel(10);

    let sender = Arc::new(tx.clone());
    let sender = warp::any().map({
        let sender = Arc::clone(&sender);
        move || Arc::clone(&sender)
    });

    tokio::spawn(async {
        let mut manager = PodManager::new(rx);
        manager.worker().await;
    });

    let pods = warp::post()
        .and(warp::path("pods"))
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

    info!("Server running at http://0.0.0.0:3030");
    let routes = pods.or(status).or(get_quote).or(list_pods);
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

#[derive(Debug)]
pub struct Wrapper(pub String);

impl Reject for Wrapper {}
