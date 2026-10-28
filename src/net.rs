use std::future::Future;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use p256::ecdh::EphemeralSecret;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use prost::Message;
use rand::rngs::OsRng;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::Value;

use crate::crypto::{
    decrypt_basic, decrypt_gcm, encrypt_basic, encrypt_gcm, generate_session_key, mark_server_time,
};
use crate::proto::{IkeReq, IkeResp};
use crate::regions::Region;
use crate::mirror;

pub struct IkeResult {
    pub token: String,
    pub cipher: i32,
    pub session_key: Vec<u8>,
}

pub async fn post_buffer(
    client: &reqwest::Client,
    url: &str,
    body: Vec<u8>,
    headers: HeaderMap,
) -> Result<Vec<u8>> {
    let mut headers = headers;
    headers
        .entry(CONTENT_TYPE)
        .or_insert(HeaderValue::from_static("application/octet-stream"));

    let response = client
        .post(url)
        .headers(headers)
        .body(body)
        .send()
        .await
        .with_context(|| format!("POST {url} failed"))?;
    let status = response.status();

    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        let reason = status.canonical_reason().unwrap_or("");
        bail!(
            "POST {url} failed with {} {}{}",
            status.as_u16(),
            reason,
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        );
    }

    Ok(response.bytes().await?.to_vec())
}

pub async fn do_ike_handshake(client: &reqwest::Client, region: &Region) -> Result<IkeResult> {
    let secret = EphemeralSecret::random(&mut OsRng);
    let client_pub = secret
        .public_key()
        .to_encoded_point(false)
        .as_bytes()
        .to_vec();
    let client_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let request = IkeReq {
        proto_version: Some(1),
        client_ts: Some(client_ts),
        pub_key: Some(client_pub.clone()),
    };
    let request_bytes = request.encode_to_vec();

    let mut packet = Vec::with_capacity(2 + request_bytes.len());
    packet.extend_from_slice(&1u16.to_be_bytes());
    packet.extend_from_slice(&request_bytes);

    let encrypted = encrypt_gcm(&packet, region.garble_key)?;
    let body = encrypt_basic(&encrypted, region.garble_key);

    let response = post_buffer(
        client,
        &format!("{}/agent-zone-1/", region.server_url),
        body,
        HeaderMap::new(),
    )
    .await?;
    let plain = decrypt_gcm(
        &decrypt_basic(&response, region.garble_key),
        region.garble_key,
    )?;
    if plain.len() < 2 {
        bail!("IKE response is too short");
    }
    let msg_id = u16::from_be_bytes([plain[0], plain[1]]);
    if msg_id != 2 {
        bail!("Unexpected IKE response msgId: {msg_id}");
    }

    let response = IkeResp::decode(&plain[2..])?;
    if let Some(server_ts) = response.server_ts {
        mark_server_time(server_ts);
    }
    let server_pub = response.pub_key.unwrap_or_default();
    if server_pub.is_empty() {
        bail!("IKE response has no server public key");
    }

    let server_public = p256::PublicKey::from_sec1_bytes(&server_pub)
        .context("IKE response has an invalid server public key")?;
    let shared = secret.diffie_hellman(&server_public);
    let session_key = generate_session_key(
        shared.raw_secret_bytes().as_slice(),
        &server_pub,
        &client_pub,
    )?;

    Ok(IkeResult {
        token: response.token.unwrap_or_default(),
        cipher: response.cipher.unwrap_or(0),
        session_key,
    })
}

async fn fetch_json_once(client: &reqwest::Client, url: &str) -> Result<Value> {
    let response = client.get(url).send().await?;
    let status = response.status();
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        bail!(
            "{} {}{}",
            status.as_u16(),
            status.canonical_reason().unwrap_or(""),
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        );
    }
    Ok(response.json::<Value>().await?)
}

pub async fn fetch_json(client: &reqwest::Client, url: &str, label: &str) -> Result<Value> {
    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 1..=3 {
        match fetch_json_once(client, url).await {
            Ok(value) => return Ok(value),
            Err(error) => {
                last_error = Some(error);
                if attempt < 3 {
                    tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
                }
            }
        }
    }
    Err(anyhow!(
        "Failed to fetch {label} ({url}): {}",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "unknown error".to_string())
    ))
}

pub async fn with_retry<F, Fut, T>(mut operation: F, attempts: usize, delay_ms: u64) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 1..=attempts {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                last_error = Some(error);
                if attempt < attempts {
                    tokio::time::sleep(Duration::from_millis(delay_ms * attempt as u64)).await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("retry operation failed")))
}

pub async fn fetch_json_path(
    client: &reqwest::Client,
    path: &str,
    label: &str,
) -> Result<Value> {
    let mut last_err = None;
    for mirror in mirror::candidates() {
        let url = mirror.build_url(path);
        match fetch_json(client, &url, label).await {
            Ok(value) => return Ok(value),
            Err(err) => {
                eprintln!("[net] {} failed on {:?}: {}", label, mirror, err);
                last_err = Some(err);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("all mirrors failed for {label}")))
}
