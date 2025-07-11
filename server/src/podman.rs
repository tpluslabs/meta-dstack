use crate::{
    Wrapper,
    tdx::{PodManager, PodManagerInstruction},
};
use bytes::Bytes;
use sha2::Digest;
use std::{io::Write, path::PathBuf, sync::Arc};
use tempfile::NamedTempFile;
use tokio::{
    process::Command,
    sync::{mpsc, oneshot},
};

impl PodManager {
    pub(crate) async fn handle_pod_yml(&mut self, pod_config: Bytes) -> anyhow::Result<()> {
        let pod_hash = sha2::Sha384::digest(&pod_config).try_into().unwrap();
        self.loaded_pods.push(pod_hash);
        tracing::info!("handling new pod config.");

        // nb: need to await for gcp to actually merge the kernel with support for tsm based rtmr interactions.
        //let client = tsm_client::make_client()?;
        //tsm_client::rtmr::extend_digest(&client, 3, &pod_hash)?;
        //tracing::info!("extended rmtr3 with digest {}", hex::encode(&pod_hash));

        let mut tmpfile = NamedTempFile::new()?;
        tmpfile.write_all(&pod_config)?;

        let path: PathBuf = tmpfile.path().into();
        tracing::info!("Executing podman with pod configuration");
        let status = Command::new("podman")
            .args(&["play", "kube", path.to_str().unwrap()])
            .status()
            .await?;

        if status.success() {
            tracing::info!("Pod started successfully");
        } else {
            tracing::error!("Failed to start pod with exit code: {:?}", status.code());
        }

        Ok(())
    }
}

pub async fn handle_pod_yml(
    sender: Arc<mpsc::Sender<PodManagerInstruction>>,
    body: Bytes,
) -> Result<impl warp::Reply, warp::Rejection> {
    let _ = sender.send(PodManagerInstruction::CreatePod(body)).await;

    Ok(warp::reply::with_status(
        "forwared to internal pod manager worker",
        warp::http::StatusCode::OK,
    ))
}

pub async fn handle_request_pods(
    sender: Arc<mpsc::Sender<PodManagerInstruction>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (one_sender, pods) = oneshot::channel();
    let _ = sender
        .send(PodManagerInstruction::RequestPods(one_sender))
        .await;

    let pods: Vec<[u8; 48]> = pods.await.unwrap();
    let pods_as_hex: Vec<String> = pods.into_iter().map(hex::encode).collect();

    Ok(warp::reply::json(&pods_as_hex))
}
