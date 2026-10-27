mod credentials;
mod crypto;
mod metadata;
mod net;
mod output;
mod proto;
mod regions;
mod sdk;
mod device;

use std::collections::BTreeSet;
use std::io::{IsTerminal, Write};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use prost::Message;

use crate::credentials::{find_yosdk_credentials, region_credentials, Credentials};
use crate::metadata::{build_catalog, load_metadata, OutputError, OutputErrorKey};
use crate::net::{do_ike_handshake, with_retry, IkeResult};
use crate::output::{
    build_region_output, load_existing_region, output_file_for_account, write_region_output,
    FetchedRegion, Pull,
};
use crate::proto::{GachaHistories, GachaInformationResp, PlayerInfo, Ui32};
use crate::regions::{region_by_key, Region, REGION_ORDER};
use crate::sdk::{
    do_player_login, get_account_login_token, send_game_request, MSG_GACHA_HISTORIES_REQ,
    MSG_GACHA_HISTORIES_SUCCEED_ACK, MSG_GACHA_INFORMATION_REQ, MSG_GACHA_INFORMATION_SUCCEED_ACK,
    MSG_PLAYER_DATA_REQ, MSG_PLAYER_DATA_SUCCEED_ACK,
};

async fn fetch_game_identity(
    client: &reqwest::Client,
    region: &Region,
    session_token: &str,
    cipher: i32,
    session_key: &[u8],
) -> Result<(String, Option<String>)> {
    let response = with_retry(
        || {
            send_game_request(
                client,
                region,
                session_token,
                cipher,
                session_key,
                MSG_PLAYER_DATA_REQ,
                &[],
                MSG_PLAYER_DATA_SUCCEED_ACK,
            )
        },
        3,
        800,
    )
    .await?;
    let info = PlayerInfo::decode(&response[..]).context("decoding player data")?;
    let acc = info.acc.context("Player data has no account info")?;
    let game_uid = acc
        .id
        .filter(|id| *id > 0)
        .context("Player data has no game UID")?;
    let game_name = acc.nickname.filter(|name| !name.is_empty());
    Ok((game_uid.to_string(), game_name))
}

async fn fetch_region_history(
    client: &reqwest::Client,
    region: &Region,
    credentials: &Credentials,
    metadata: &metadata::Metadata,
) -> Result<FetchedRegion> {
    let catalog = build_catalog(metadata)?;

    println!("Connecting to {}...", region.server_url);
    let ike: IkeResult = do_ike_handshake(client, region).await?;
    println!(
        "IKE ok ({})",
        if ike.cipher == 1 {
            "ChaCha20-Poly1305"
        } else {
            "AES-GCM"
        }
    );

    let quick = get_account_login_token(client, region, credentials).await?;
    let session_token = do_player_login(
        client,
        region,
        credentials,
        &ike.token,
        ike.cipher,
        &ike.session_key,
        &quick.account_login_token,
        &quick.account_uid,
    )
    .await?;
    if session_token.is_empty() {
        bail!("Player login returned an empty session token");
    }

    crypto::reset_http_seq(1);

    let (game_uid, game_name) =
        fetch_game_identity(client, region, &session_token, ike.cipher, &ike.session_key).await?;
    match &game_name {
        Some(name) => println!("Name: {name} (game UID {game_uid})"),
        None => println!("Game UID {game_uid}"),
    }

    let mut storage_ids: BTreeSet<u32> = catalog.storages.keys().copied().collect();
    for banner in &catalog.banners {
        storage_ids.insert(banner.storage_id);
    }
    storage_ids = storage_ids.into_iter().filter(|id| *id > 0).collect();

    let mut pulls = Vec::new();
    let mut errors = Vec::new();

    for storage_id in storage_ids {
        let body = Ui32 {
            value: Some(storage_id),
        }
        .encode_to_vec();
        let result = with_retry(
            || {
                send_game_request(
                    client,
                    region,
                    &session_token,
                    ike.cipher,
                    &ike.session_key,
                    MSG_GACHA_HISTORIES_REQ,
                    &body,
                    MSG_GACHA_HISTORIES_SUCCEED_ACK,
                )
            },
            3,
            800,
        )
        .await;

        match result.and_then(|response| Ok(GachaHistories::decode(&response[..])?)) {
            Ok(histories) => {
                for history in histories.list {
                    pulls.push(Pull {
                        banner_id: history.gid.unwrap_or(0),
                        storage_id,
                        time: history.time.unwrap_or(0),
                        ids: history.ids,
                    });
                }
            }
            Err(error) => {
                eprintln!("  storage {storage_id}: {error}");
                errors.push(OutputError {
                    storage_id: OutputErrorKey::Number(storage_id),
                    error: error.to_string(),
                });
            }
        }
    }

    let mut gacha_info = Vec::new();
    let information_result = with_retry(
        || {
            send_game_request(
                client,
                region,
                &session_token,
                ike.cipher,
                &ike.session_key,
                MSG_GACHA_INFORMATION_REQ,
                &[],
                MSG_GACHA_INFORMATION_SUCCEED_ACK,
            )
        },
        3,
        800,
    )
    .await;

    match information_result.and_then(|response| Ok(GachaInformationResp::decode(&response[..])?)) {
        Ok(response) => gacha_info = response.information,
        Err(error) => {
            eprintln!("Gacha information failed: {error}");
            errors.push(OutputError {
                storage_id: OutputErrorKey::Text("information".to_string()),
                error: error.to_string(),
            });
        }
    }

    Ok(FetchedRegion {
        uid: quick.account_uid,
        game_uid,
        game_name,
        catalog,
        pulls,
        gacha_info,
        errors,
    })
}

async fn run() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let detected: Vec<&Region> = REGION_ORDER
        .iter()
        .filter_map(|key| region_by_key(key))
        .filter(|region| find_yosdk_credentials(region.key).is_some())
        .collect();
    let labels = detected
        .iter()
        .map(|region| region.label)
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "Detected {} region{}:{}",
        detected.len(),
        if detected.len() == 1 { "" } else { "s" },
        if labels.is_empty() {
            String::new()
        } else {
            format!(" {labels}")
        }
    );

    let mut success_count = 0usize;
    let mut total_pulls = 0u64;

    for region in detected {
        let accounts = region_credentials(region);
        let metadata = match load_metadata(&client, region).await {
            Ok(metadata) => metadata,
            Err(error) => {
                eprintln!("{} failed: {error:#}", region.label);
                continue;
            }
        };

        for credentials in accounts {
            println!(
                "Using {} ({}, Yostar UID {})",
                credentials.file.display(),
                region.label,
                credentials.uid
            );

            let region_result: Result<u64> = async {
                let fetched = fetch_region_history(&client, region, credentials, &metadata).await?;
                let output_file = output_file_for_account(region.key, &fetched.game_uid)?;
                let existing = load_existing_region(&output_file);
                let region_output = build_region_output(
                    region.key,
                    &fetched,
                    existing.as_ref(),
                    &metadata.item_data,
                )?;
                write_region_output(&output_file, &region_output)?;
                println!("{}: {} pulls", region.label, region_output.totals.pulls);
                println!("Wrote {}", output_file.display());
                Ok(region_output.totals.pulls)
            }
            .await;

            match region_result {
                Ok(pulls) => {
                    success_count += 1;
                    total_pulls += pulls;
                }
                Err(error) => eprintln!("{} failed: {error:#}", region.label),
            }
        }
    }

    if success_count == 0 {
        bail!("No region could be fetched");
    }
    if total_pulls == 0 {
        eprintln!("Warning: the fetched history is empty. The metadata was still written.");
    }

    Ok(())
}

fn wait_for_exit() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    if !stdin.is_terminal() || !stdout.is_terminal() {
        return;
    }

    print!("\nPress any key to close...");
    let _ = stdout.flush();

    if crossterm::terminal::enable_raw_mode().is_ok() {
        loop {
            match crossterm::event::read() {
                Ok(crossterm::event::Event::Key(_)) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        let _ = crossterm::terminal::disable_raw_mode();
    } else {
        let mut line = String::new();
        let _ = stdin.read_line(&mut line);
    }
    println!();
}

#[tokio::main]
async fn main() {
    crypto::init_clock();
    let result = run().await;
    if let Err(error) = &result {
        eprintln!("ERROR: {error:#}");
    }
    wait_for_exit();
    if result.is_err() {
        std::process::exit(1);
    }
}
