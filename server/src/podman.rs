use crate::Wrapper;
use bytes::Bytes;
use sha2::Digest;
use std::{io::Write, path::PathBuf};
use tempfile::NamedTempFile;
use tokio::process::Command;

pub async fn handle_pod_yml(body: Bytes) -> Result<impl warp::Reply, warp::Rejection> {
    let pod_hash = sha2::Sha384::digest(&body).to_vec();
    let client = tsm_client::make_client().map_err(|e| {
        tracing::error!("failed to create tsm client: {}", e);
        warp::reject::custom(Wrapper(format!("failed to create tsm client: {}", e)))
    })?;

    if let Err(e) = tsm_client::rtmr::extend_digest(&client, 3, &pod_hash) {
        return Err(warp::reject::custom(Wrapper(format!(
            "failed to extend rtmr3: {}",
            e
        ))));
    }

    let mut tmpfile = NamedTempFile::new().map_err(|e| {
        tracing::error!("Temp file creation failed: {}", e);
        warp::reject::custom(Wrapper(format!("temp file creation failed {e}")))
    })?;
    tmpfile.write_all(&body).map_err(|e| {
        tracing::error!("Writing to temp file failed: {}", e);
        warp::reject::custom(Wrapper(format!("writing to temp file failed: {e}")))
    })?;

    let path: PathBuf = tmpfile.path().into();
    tracing::info!("Executing podman with pod configuration");
    let status = Command::new("podman")
        .args(&["play", "kube", path.to_str().unwrap()])
        .status()
        .await
        .map_err(|e| {
            tracing::error!("Failed to execute podman: {}", e);
            warp::reject::custom(Wrapper(format!("Failed to execute podman: {e}")))
        })?;

    if status.success() {
        tracing::info!("Pod started successfully");
        Ok(warp::reply::with_status(
            "Pod started",
            warp::http::StatusCode::CREATED,
        ))
    } else {
        tracing::error!("Failed to start pod with exit code: {:?}", status.code());
        Ok(warp::reply::with_status(
            "Failed to start pod",
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
