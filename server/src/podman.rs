use crate::tdx::{PodManager, PodManagerInstruction};
use bytes::Bytes;
use serde::Deserialize;
#[cfg(not(feature = "tplus"))]
use serde::Deserialize;
use sha2::Digest;
use std::{io::Write, path::PathBuf, sync::Arc};
use tempfile::NamedTempFile;
use tokio::{
    process::Command,
    sync::{mpsc, oneshot},
};

use anyhow::Result;
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::BTreeMap;

// NB: not sure if we need this, in general I don't like the approach of extracting away non measured data from the pod file
// might want to switch over to a more formal configs loader that is either handled by the app or in a generic way by the server.
fn sort_json(v: JsonValue) -> JsonValue {
    match v {
        JsonValue::Object(map) => {
            let mut sorted: BTreeMap<String, JsonValue> = BTreeMap::new();
            for (k, val) in map {
                sorted.insert(k, sort_json(val));
            }
            let obj: JsonMap<String, JsonValue> = sorted.into_iter().collect();
            JsonValue::Object(obj)
        }
        JsonValue::Array(arr) => {
            if arr
                .iter()
                .all(|e| e.get("name").and_then(|n| n.as_str()).is_some())
            {
                let mut arr = arr.into_iter().map(sort_json).collect::<Vec<_>>();
                arr.sort_by(|a, b| {
                    a.get("name")
                        .and_then(|n| n.as_str())
                        .cmp(&b.get("name").and_then(|n| n.as_str()))
                });
                JsonValue::Array(arr)
            } else {
                JsonValue::Array(arr.into_iter().map(sort_json).collect())
            }
        }
        other => other,
    }
}

fn get_pod_hash(pod_config: &Bytes) -> Result<[u8; 48]> {
    let yaml_docs: Vec<serde_yaml::Value> = serde_yaml::Deserializer::from_slice(&pod_config)
        .map(|doc| serde_yaml::Value::deserialize(doc))
        .collect::<Result<_, _>>()?;

    let mut json_docs: Vec<JsonValue> = yaml_docs
        .into_iter()
        .map(|d| serde_json::to_value(d))
        .collect::<Result<_, _>>()?;

    for doc in &mut json_docs {
        if let Some(kind) = doc.get("kind").and_then(|k| k.as_str()) {
            if kind == "ConfigMap" {
                if let Some(obj) = doc.as_object_mut() {
                    obj.remove("data");
                    obj.remove("binaryData");
                }
            }
        }
    }

    let canon = sort_json(JsonValue::Array(json_docs));
    let payload = serde_json::to_string(&canon)?;
    let pod_hash: [u8; 48] = sha2::Sha384::digest(payload.as_bytes())
        .as_slice()
        .try_into()
        .expect("Sha384 output is 48 bytes");

    Ok(pod_hash)
}

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
        //let pod_hash = sha2::Sha384::digest(&pod_config).try_into().unwrap();
        let pod_hash = get_pod_hash(&pod_config)?;

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

#[cfg(test)]
mod tests {
    use super::get_pod_hash;
    use bytes::Bytes;

    const YAML_1: &str = r#"apiVersion: v1
kind: Pod
metadata:
  name: clearing-engine-pod
spec:
  hostNetwork: true
  containers:
    - name: clearing-engine-container
      image: docker.io/xycloo/clearing-engine-test:latest
      volumeMounts:
        - name: config-volume
          mountPath: /app/config/ce.toml
          subPath: ce.toml
  volumes:
    - name: config-volume
      configMap:
        name: ce-config
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: ce-config
data:
  ce.toml: |
    metrics_port = 8080

    [overlay]
    listen_port = 7000
    peers = [
        #"/ip4/34.162.88.71/udp/7002/quic-v1",
        #"/ip4/34.162.88.71/udp/7003/quic-v1"
    ]

    [books]
    max_orderbook_count = 16
"#;

    const YAML_2: &str = r#"apiVersion: v1
kind: Pod
metadata:
  name: clearing-engine-pod
spec:
  hostNetwork: true
  containers:
    - name: clearing-engine-container
      image: docker.io/xycloo/clearing-engine-test:latest
      volumeMounts:
        - name: config-volume
          mountPath: /app/config/ce.toml
          subPath: ce.toml
  volumes:
    - name: config-volume
      configMap:
        name: ce-config
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: ce-config
data:
  ce.toml: |
    metrics_port = 8080

    [overlay]
    listen_port = 7010
    peers = [
        #"/ip4/34.162.88.71/udp/7002/quic-v1",
        #"/ip4/34.162.88.71/udp/7003/quic-v1"
    ]

    [books]
    max_orderbook_count = 32
"#;

    #[test]
    fn config_ignored_in_measurement() {
        let h1 = get_pod_hash(&Bytes::from_static(YAML_1.as_bytes())).expect("hash 1");
        let h2 = get_pod_hash(&Bytes::from_static(YAML_2.as_bytes())).expect("hash 2");
        assert_eq!(h1, h2);
    }

    #[test]
    fn non_config_change_measurement() {
        let yaml_changed_pod = r#"apiVersion: v1
kind: Pod
metadata:
  name: clearing-engine-pod
spec:
  hostNetwork: true
  containers:
    - name: clearing-engine-container
      image: docker.io/xycloo/clearing-engine-test@sha256:deadbeef
      volumeMounts:
        - name: config-volume
          mountPath: /app/config/ce.toml
          subPath: ce.toml
  volumes:
    - name: config-volume
      configMap:
        name: ce-config
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: ce-config
data:
  ce.toml: |
    metrics_port = 8080

    [overlay]
    listen_port = 7000
    peers = [
        #"/ip4/34.162.88.71/udp/7002/quic-v1",
        #"/ip4/34.162.88.71/udp/7003/quic-v1"
    ]

    [books]
    max_orderbook_count = 16
"#;

        let h_base = get_pod_hash(&Bytes::from_static(YAML_1.as_bytes())).expect("hash base");
        let h_changed =
            get_pod_hash(&Bytes::from_static(yaml_changed_pod.as_bytes())).expect("hash changed");
        assert_ne!(h_base, h_changed);
    }
}
