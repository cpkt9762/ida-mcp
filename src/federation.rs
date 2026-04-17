use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationNodeConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_weight() -> u32 {
    1
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct FederationNodeStatus {
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub healthy: bool,
    pub ready: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteDispatchResult {
    pub node: String,
    pub url: String,
    pub response: Value,
}

pub fn load_nodes_from_env() -> Vec<FederationNodeConfig> {
    let path = match std::env::var("IDA_CLI_FEDERATION_CONFIG") {
        Ok(path) => path,
        Err(_) => return Vec::new(),
    };
    load_nodes(&path).unwrap_or_default()
}

pub fn load_nodes(path: &str) -> anyhow::Result<Vec<FederationNodeConfig>> {
    let data = fs::read_to_string(path)?;
    let nodes: Vec<FederationNodeConfig> = serde_json::from_str(&data)?;
    Ok(nodes)
}

pub fn probe_nodes(nodes: &[FederationNodeConfig]) -> Vec<FederationNodeStatus> {
    nodes.iter().map(probe_node).collect()
}

pub fn choose_ready_node(nodes: &[FederationNodeConfig]) -> Option<FederationNodeConfig> {
    let mut ready: Vec<FederationNodeConfig> = nodes
        .iter()
        .filter(|node| node.enabled)
        .filter_map(|node| {
            let status = probe_node(node);
            status.ready.then_some(node.clone())
        })
        .collect();
    ready.sort_by_key(|node| std::cmp::Reverse(node.weight));
    ready.into_iter().next()
}

pub fn submit_enqueue(
    node: &FederationNodeConfig,
    payload: &Value,
) -> anyhow::Result<RemoteDispatchResult> {
    let uri: hyper::Uri = node.url.parse()?;
    let host = uri
        .host()
        .ok_or_else(|| anyhow::anyhow!("missing host in node url"))?;
    let port = uri.port_u16().unwrap_or(80);
    let response = post_json(host, port, "/enqueuez", payload)?;
    Ok(RemoteDispatchResult {
        node: node.name.clone(),
        url: node.url.clone(),
        response,
    })
}

fn probe_node(node: &FederationNodeConfig) -> FederationNodeStatus {
    if !node.enabled {
        return FederationNodeStatus {
            name: node.name.clone(),
            url: node.url.clone(),
            enabled: false,
            healthy: false,
            ready: false,
            detail: "disabled".to_string(),
        };
    }

    let uri: hyper::Uri = match node.url.parse() {
        Ok(uri) => uri,
        Err(err) => {
            return FederationNodeStatus {
                name: node.name.clone(),
                url: node.url.clone(),
                enabled: true,
                healthy: false,
                ready: false,
                detail: format!("invalid url: {err}"),
            };
        }
    };

    if uri.scheme_str() != Some("http") {
        return FederationNodeStatus {
            name: node.name.clone(),
            url: node.url.clone(),
            enabled: true,
            healthy: false,
            ready: false,
            detail: "only http federation urls are currently supported".to_string(),
        };
    }

    let host = match uri.host() {
        Some(host) => host,
        None => {
            return FederationNodeStatus {
                name: node.name.clone(),
                url: node.url.clone(),
                enabled: true,
                healthy: false,
                ready: false,
                detail: "missing host".to_string(),
            };
        }
    };
    let port = uri.port_u16().unwrap_or(80);

    let healthy = fetch_json(host, port, "/healthz")
        .ok()
        .and_then(|v| v.get("ok").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let ready = fetch_json(host, port, "/readyz")
        .ok()
        .and_then(|v| v.get("ok").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    FederationNodeStatus {
        name: node.name.clone(),
        url: node.url.clone(),
        enabled: true,
        healthy,
        ready,
        detail: if healthy || ready {
            "ok".to_string()
        } else {
            "unreachable or unhealthy".to_string()
        },
    }
}

fn fetch_json(host: &str, port: u16, path: &str) -> anyhow::Result<serde_json::Value> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: application/json\r\n\r\n");
    stream.write_all(request.as_bytes())?;
    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    let body = buf
        .split("\r\n\r\n")
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("invalid http response"))?;
    Ok(serde_json::from_str(body)?)
}

fn post_json(host: &str, port: u16, path: &str, payload: &Value) -> anyhow::Result<Value> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let body = serde_json::to_vec(payload)?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes())?;
    stream.write_all(&body)?;
    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    let body = buf
        .split("\r\n\r\n")
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("invalid http response"))?;
    Ok(serde_json::from_str(body)?)
}
