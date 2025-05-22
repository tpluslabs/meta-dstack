use crate::Wrapper;
use bytes::Bytes;
use std::sync::Arc;
use tokio::{
    process::Command,
    sync::{mpsc, oneshot},
};

mod quote;

pub struct PodManager {
    rx: mpsc::Receiver<PodManagerInstruction>,
    pub(crate) loaded_pods: Vec<[u8; 48]>,
}

pub enum PodManagerInstruction {
    CreatePod(Bytes),
    RequestQuote((String, oneshot::Sender<Result<String, warp::Rejection>>)),
}

impl PodManager {
    pub fn new(rx: mpsc::Receiver<PodManagerInstruction>) -> Self {
        Self {
            rx,
            loaded_pods: Vec::new(),
        }
    }

    fn aggregate_digest(&self) -> Vec<u8> {
        self.loaded_pods.concat().to_vec()
    }

    pub async fn worker(&mut self) {
        let _ = Command::new("podman")
            .args(&["pull", "k8s.gcr.io/pause:3.8"])
            .status()
            .await;

        let _ = Command::new("sed")
            .args(&[
                "-i",
                "/^\\[engine\\]/a infra_image = \"k8s.gcr.io/pause:3.8\"",
                "/etc/containers/containers.conf",
            ])
            .status()
            .await;

        while let Some(task) = self.rx.recv().await {
            match task {
                PodManagerInstruction::CreatePod(pod_config) => {
                    let _ = self.handle_pod_yml(pod_config).await;
                }

                PodManagerInstruction::RequestQuote((report_data, sender)) => {
                    let resp = self.get_quote(report_data).await;
                    let _ = sender.send(resp);
                }
            }
        }
    }

    async fn get_quote(&self, report_data: String) -> Result<String, warp::Rejection> {
        let report_data = [
            self.aggregate_digest(),
            hex::decode(report_data).map_err(|_| Wrapper("invalid hex".into()))?,
        ]
        .concat();

        let quote = quote::get_quote(&report_data).await;

        match quote {
            Ok(quote) => {
                tracing::info!("successfully obtained quote");
                Ok(quote)
            }

            Err(e) => {
                tracing::error!("failed to obtain quote: {:?}", e);
                Ok("failed to get quote".into())
            }
        }
    }
}

pub async fn handle_quote_request(
    report_data: String,
    sender: Arc<mpsc::Sender<PodManagerInstruction>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (one_sender, quote) = oneshot::channel();
    let _ = sender
        .send(PodManagerInstruction::RequestQuote((
            report_data,
            one_sender,
        )))
        .await;

    let quote = quote
        .await
        .map_err(|_| Wrapper("oneshot sender dropped unexpectedly".into()))??;
    Ok(warp::reply::with_status(quote, warp::http::StatusCode::OK))
}
