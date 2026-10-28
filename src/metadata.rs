use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::net::fetch_json_path;
use crate::regions::{
    banner_lang_path, banner_path, gacha_extra_path, item_path, storage_lang_path, storage_path,
    Region,
};

#[derive(Clone, Debug)]
pub struct Storage {
    pub id: u32,
    pub key: String,
    pub name: String,
    pub type_: String,
}

#[derive(Clone, Debug)]
pub struct Banner {
    pub id: u32,
    pub storage_id: u32,
    pub storage_key: Option<String>,
    pub storage_name: Option<String>,
    pub type_: Option<String>,
    pub name: String,
    pub icon: Option<String>,
    pub image: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub active: bool,
    pub rate_up5_star: Vec<Value>,
    pub rate_up4_star: Vec<Value>,
    pub unknown: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BannerOutput {
    pub id: u32,
    pub storage_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_name: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    pub active: bool,
    pub rate_up5_star: Vec<Value>,
    pub rate_up4_star: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unknown: Option<bool>,
    pub stats: Stats,
    pub pity: Option<Pity>,
    pub spark: Spark,
    pub history: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemInfo {
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rarity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_up: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub time: i64,
    #[serde(rename = "timeISO")]
    pub time_iso: String,
    pub ids: Vec<u32>,
    pub items: Vec<ItemInfo>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub pulls: u64,
    pub items: u64,
    pub rate_up_items: u64,
    #[serde(rename = "rateUpSSR")]
    pub rate_up_ssr: u64,
    pub by_rarity: BTreeMap<String, u64>,
    pub by_type: BTreeMap<String, u64>,
    pub first_pull_time: Option<i64>,
    pub last_pull_time: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pity {
    pub total_times: u32,
    pub gacha_total_times: u32,
    pub days_count: u32,
    pub aup_miss_times: u32,
    pub amiss_times: u32,
    pub aup_guarantee_times: u32,
    pub received_first_ten_reward: bool,
    pub received_guarantee_reward: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spark {
    pub sparkable: bool,
    pub threshold: u64,
    pub counted_pulls: u64,
    pub history_items: u64,
    pub banner_pulls: Option<u32>,
    pub sparked: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageOutput {
    pub id: u32,
    pub key: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub banner_ids: Vec<u32>,
    pub pulls: u64,
    pub items: u64,
    pub rate_up_items: u64,
    #[serde(rename = "rateUpSSR")]
    pub rate_up_ssr: u64,
    pub by_rarity: BTreeMap<String, u64>,
    pub by_type: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub pulls: u64,
    pub items: u64,
    pub rate_up_items: u64,
    #[serde(rename = "rateUpSSR")]
    pub rate_up_ssr: u64,
    pub by_rarity: BTreeMap<String, u64>,
    pub by_type: BTreeMap<String, u64>,
    pub banners: usize,
    pub active_banners: usize,
    pub banners_with_history: usize,
    pub sparks: usize,
    pub sparked_banner_ids: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlatHistoryEntry {
    pub banner_id: u32,
    pub banner_name: Option<String>,
    pub storage_id: u32,
    pub time: i64,
    #[serde(rename = "timeISO")]
    pub time_iso: String,
    pub count: usize,
    pub ids: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionOutput {
    pub region: String,
    pub uid: String,
    pub game_uid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_name: Option<String>,
    pub updated_at: String,
    pub totals: Totals,
    pub storages: Vec<StorageOutput>,
    pub banners: Vec<BannerOutput>,
    pub items: BTreeMap<u32, ItemInfo>,
    pub history: Vec<FlatHistoryEntry>,
    pub errors: Vec<OutputError>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputError {
    pub storage_id: OutputErrorKey,
    pub error: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum OutputErrorKey {
    Number(u32),
    Text(String),
}

#[derive(Clone, Debug)]
pub struct Catalog {
    pub banners: Vec<Banner>,
    pub banner_by_id: HashMap<u32, usize>,
    pub storages: BTreeMap<u32, Storage>,
}

pub struct Metadata {
    pub raw_banners: Value,
    pub raw_storages: Value,
    pub banner_lang: Value,
    pub storage_lang: Value,
    pub extra_banners: Value,
    pub item_data: Value,
}

pub async fn load_metadata(client: &reqwest::Client, region: &Region) -> Result<Metadata> {
    println!("Fetching stelladb data...");
    let banners_rel = banner_path(region);
    let storages_rel = storage_path(region);
    let banner_lang_rel = banner_lang_path(region);
    let storage_lang_rel = storage_lang_path(region);
    let extra_rel = gacha_extra_path();
    let items_rel = item_path();

    let banners_label = format!("{}/bin/Gacha.json", region.data_path);
    let storages_label = format!("{}/bin/GachaStorage.json", region.data_path);
    let banner_lang_label = format!(
        "{}/language/{}/Gacha.json",
        region.data_path, region.language
    );
    let storage_lang_label = format!(
        "{}/language/{}/GachaStorage.json",
        region.data_path, region.language
    );

    let banners = fetch_json_path(client, &banners_rel, &banners_label);
    let storages = fetch_json_path(client, &storages_rel, &storages_label);
    let banner_lang = fetch_json_path(client, &banner_lang_rel, &banner_lang_label);
    let storage_lang = fetch_json_path(client, &storage_lang_rel, &storage_lang_label);
    let extra = fetch_json_path(client, &extra_rel, "gacha.json");
    let items = fetch_json_path(client, &items_rel, "item.json");

    let (raw_banners, raw_storages, banner_lang, storage_lang, extra_banners, item_data) =
        tokio::try_join!(banners, storages, banner_lang, storage_lang, extra, items)?;

    Ok(Metadata {
        raw_banners,
        raw_storages,
        banner_lang,
        storage_lang,
        extra_banners,
        item_data,
    })
}

fn basename(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?;
    text.split('/')
        .last()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn storage_key_from_name(name: Option<&str>) -> String {
    let Some(name) = name else {
        return String::new();
    };
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() >= 2 {
        parts[1..parts.len() - 1].join(".")
    } else {
        name.to_string()
    }
}

fn json_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}

fn json_array(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn parse_time(text: &str) -> Option<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
        return Some(parsed.with_timezone(&Utc));
    }
    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .map(|naive| naive.and_utc())
}

fn is_banner_active(start_time: Option<&str>, end_time: Option<&str>) -> bool {
    let now = Utc::now();
    if let Some(start_time) = start_time {
        if let Some(start) = parse_time(start_time) {
            if now < start {
                return false;
            }
        }
    }
    if let Some(end_time) = end_time {
        if let Some(end) = parse_time(end_time) {
            if now > end {
                return false;
            }
        }
    }
    true
}

pub fn build_catalog(metadata: &Metadata) -> Result<Catalog> {
    let raw_storages = metadata
        .raw_storages
        .as_object()
        .context("GachaStorage.json is not an object")?;
    let mut storages = BTreeMap::new();

    for (id_text, raw) in raw_storages {
        let id: u32 = id_text
            .parse()
            .with_context(|| format!("Invalid storage id {id_text}"))?;
        let raw_name = raw.get("Name").and_then(Value::as_str);
        let key = storage_key_from_name(raw_name);
        let name = raw_name
            .and_then(|name| metadata.storage_lang.get(name))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .or_else(|| {
                if key.is_empty() {
                    None
                } else {
                    Some(key.clone())
                }
            })
            .unwrap_or_else(|| format!("Pool {id}"));
        let type_ = if key.to_lowercase().contains("disc") {
            "disc"
        } else {
            "character"
        };
        storages.insert(
            id,
            Storage {
                id,
                key,
                name,
                type_: type_.to_string(),
            },
        );
    }

    let raw_banners = metadata
        .raw_banners
        .as_object()
        .context("Gacha.json is not an object")?;
    let mut banners = Vec::new();
    let mut banner_by_id = HashMap::new();

    for (id_text, raw) in raw_banners {
        let id: u32 = id_text
            .parse()
            .with_context(|| format!("Invalid banner id {id_text}"))?;
        let extra = metadata.extra_banners.get(id_text);
        let storage_id = raw
            .get("StorageId")
            .and_then(Value::as_u64)
            .map(|value| value as u32)
            .with_context(|| format!("Banner {id} has no storage id"))?;
        let storage = storages
            .get(&storage_id)
            .with_context(|| format!("Banner {id} references missing storage {storage_id}"))?;

        let raw_name = raw.get("Name").and_then(Value::as_str);
        let name = raw_name
            .and_then(|name| metadata.banner_lang.get(name))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .or_else(|| {
                json_string(extra.and_then(|extra| extra.get("name")))
                    .filter(|name| !name.is_empty())
            })
            .unwrap_or_else(|| format!("Banner {id}"));
        let type_ = json_string(extra.and_then(|extra| extra.get("type")))
            .filter(|type_| !type_.is_empty())
            .or_else(|| Some(storage.type_.clone()));
        let icon = json_string(extra.and_then(|extra| extra.get("icon")))
            .filter(|icon| !icon.is_empty())
            .or_else(|| basename(raw.get("Icon")));
        let image = json_string(raw.get("Image"));
        let start_time = json_string(raw.get("StartTime"));
        let end_time = json_string(raw.get("EndTime"));
        let active = is_banner_active(start_time.as_deref(), end_time.as_deref());
        let rate_up5_star = json_array(extra.and_then(|extra| extra.get("rateUp5Star")));
        let rate_up4_star = json_array(extra.and_then(|extra| extra.get("rateUp4Star")));

        let banner = Banner {
            id,
            storage_id,
            storage_key: Some(storage.key.clone()),
            storage_name: Some(storage.name.clone()),
            type_,
            name,
            icon,
            image,
            start_time,
            end_time,
            active,
            rate_up5_star,
            rate_up4_star,
            unknown: None,
        };
        banner_by_id.insert(id, banners.len());
        banners.push(banner);
    }

    Ok(Catalog {
        banners,
        banner_by_id,
        storages,
    })
}
