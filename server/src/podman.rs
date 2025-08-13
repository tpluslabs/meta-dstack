use crate::tdx::{PodManager, PodManagerInstruction};
use bytes::Bytes;
#[cfg(not(feature = "tplus"))]
use serde::Deserialize;
use sha2::Digest;
use std::{io::Write, path::PathBuf, sync::Arc};
use tempfile::NamedTempFile;
use tokio::{
    process::Command,
    sync::{mpsc, oneshot},
};

const ALLOWED_PODS_HEX: [&str; 5] = [
    "c084f54924643800c2b2d1d53df9faa90134714eb2a45036ca59ec06cca1a06dcc227e1dfdd2f8558c51e67039766a63",
    "b1dd776d5773889d59791f089ec5dc118fd09deb0d1b7aa4a0fdd5a450805525b0aae04f556417497f057b3f1ae126d4",
    "05523adf63412be53a0adbc1d88a327efb13e88cee0e01fed047a52977c2c74c11905bfdd74fdbf5c4d8cb3c3dd28759",
    "e12ea8e5d2fdd07707a62a9fb13bae9bc3d4bbf4778ab7dae948a43aa87eeec4440703957c78b3d5435c1ed9b71940ed",
    "c379f138eaea0e2001cc1427ecd0125531925953f10ecc22a0386f94a50b858d2356c2bdd04e53ed20b509d7e94044da",
];

fn allowed_pods() -> anyhow::Result<Vec<[u8; 48]>> {
    ALLOWED_PODS_HEX
        .iter()
        .map(|hex| {
            let bytes = hex::decode(hex)?;
            <[u8; 48]>::try_from(bytes.as_slice())
                .map_err(|_| anyhow::anyhow!("invalid pod length"))
        })
        .collect()
}

fn is_allowed_pod(hash: &[u8; 48]) -> anyhow::Result<bool> {
    let allowed = allowed_pods()?;
    Ok(allowed.contains(hash))
}

impl PodManager {
    pub(crate) async fn handle_pod_yml(&mut self, pod_config: Bytes) -> anyhow::Result<()> {
        let pod_hash = sha2::Sha384::digest(&pod_config).try_into().unwrap();

        #[cfg(feature = "tplus")]
        if !is_allowed_pod(&pod_hash)? {
            anyhow::bail!("pod not allowed")
        }

        #[cfg(not(feature = "tplus"))]
        if !self.dynamic_allowed_pods.contains(&pod_hash) {
            anyhow::bail!("pod not allowed")
        }

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

#[cfg(not(feature = "tplus"))]
#[derive(Deserialize)]
pub struct AllowedPodsRequest {
    pods: Vec<String>,
}

#[cfg(not(feature = "tplus"))]
pub async fn handle_pods_allow(
    sender: Arc<mpsc::Sender<PodManagerInstruction>>,
    body: AllowedPodsRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let allowed = body
        .pods
        .iter()
        .map(|hash| hex::decode(hash).unwrap().try_into().unwrap())
        .collect();
    let _ = sender.send(PodManagerInstruction::AllowPods(allowed)).await;

    Ok(warp::reply::with_status(
        "forwared to internal pod manager worker, remember that allowing pods is a one-time operation per VM",
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
