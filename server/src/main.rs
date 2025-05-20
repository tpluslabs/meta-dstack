use std::{io::Write, path::PathBuf};

use bytes::Bytes;
use tempfile::NamedTempFile;
use tokio::process::Command;
use warp::{reject::Reject, Filter};

#[tokio::main]
async fn main() {
    let pods = warp::post()
        .and(warp::path("pods"))
        .and(warp::body::bytes())
        .and_then(handle_pod_yml);

    println!("Server running at http://0.0.0.0:3030");
    warp::serve(pods)
        .run(([0, 0, 0, 0], 3030))
        .await;
}
#[derive(Debug)]
pub struct Wrapper(pub String);

impl Reject for Wrapper {}

async fn handle_pod_yml(body: Bytes) -> Result<impl warp::Reply, warp::Rejection> {
    let mut tmpfile = NamedTempFile::new().map_err(|e| {
        warp::reject::custom(Wrapper(format!("temp file creation failed {e}")))
    })?;
    tmpfile.write_all(&body).map_err(|e| {
        warp::reject::custom(Wrapper(format!("writing to temp file failed: {e}")))
    })?;

    let path: PathBuf = tmpfile.path().into();
    let status = Command::new("podman")
        .args(&["play", "kube", path.to_str().unwrap()])
        .status()
        .await
        .map_err(|e| {
            warp::reject::custom(Wrapper(format!("Failed to execute podman: {e}")))
        })?;

    if status.success() {
        Ok(warp::reply::with_status(
            "Pod started",
            warp::http::StatusCode::CREATED,
        ))
    } else {
        Ok(warp::reply::with_status(
            "Failed to start pod",
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
