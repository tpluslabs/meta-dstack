use podman::handle_pod_yml;
use tdx::handle_quote_request;
use tracing::{Level, info};
use warp::{Filter, reject::Reject};

mod podman;
mod tdx;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let pods = warp::post()
        .and(warp::path("pods"))
        .and(warp::body::bytes())
        .and_then(handle_pod_yml);

    let get_quote = warp::get()
        .and(warp::path!("quote" / String))
        .and_then(handle_quote_request);

    let status = warp::get()
        .and(warp::path("status"))
        .map(|| warp::reply::json(&serde_json::json!({"status": "ok"})));

    info!("Server running at http://0.0.0.0:3030");
    let routes = pods.or(status).or(get_quote);
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

#[derive(Debug)]
pub struct Wrapper(pub String);

impl Reject for Wrapper {}
