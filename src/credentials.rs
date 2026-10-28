use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use serde_json::Value;

use crate::regions::{normalize_region, Region};

#[derive(Clone, Debug)]
pub struct Credentials {
    pub uid: String,
    pub token: String,
    pub device: String,
    pub file: PathBuf,
    pub region: Option<&'static str>,
    pub mtime: u128,
    pub login_time: i64,
}

static YOSDK_CACHE: OnceLock<HashMap<String, Vec<Credentials>>> = OnceLock::new();

pub fn load_yosdk_credentials() -> &'static HashMap<String, Vec<Credentials>> {
    YOSDK_CACHE.get_or_init(|| {
        let files = find_yosdk_files();
        let mut users: Vec<Credentials> = files
            .iter()
            .filter(|file| {
                file.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| {
                        let n = name.to_ascii_lowercase();
                        // MODIFIED: also accept CN sdk_cn_official_userinfo
                        n.starts_with("yosdk_userinfo") || n == "sdk_cn_official_userinfo"
                    })
                    .unwrap_or(false)
            })
            .flat_map(|file| parse_yosdk_user_info(file).unwrap_or_default())
            .collect();

        users.sort_by(|a, b| {
            b.mtime
                .cmp(&a.mtime)
                .then_with(|| b.login_time.cmp(&a.login_time))
        });

        let mut by_region: HashMap<String, Vec<Credentials>> = HashMap::new();
        for user in users {
            let Some(region) = user.region else {
                continue;
            };
            let entries = by_region.entry(region.to_string()).or_default();
            if let Some(existing) = entries.iter_mut().find(|entry| entry.uid == user.uid) {
                let newer = user.mtime > existing.mtime
                    || (user.mtime == existing.mtime && user.login_time > existing.login_time);
                if newer {
                    *existing = user;
                }
            } else {
                entries.push(user);
            }
        }

        for entries in by_region.values_mut() {
            entries.sort_by(|a, b| compare_uids(&a.uid, &b.uid));
        }

        by_region
    })
}

pub fn find_yosdk_credentials(region_key: &str) -> Option<&'static [Credentials]> {
    load_yosdk_credentials().get(region_key).map(Vec::as_slice)
}

pub fn region_credentials(region: &Region) -> &'static [Credentials] {
    find_yosdk_credentials(region.key).unwrap_or(&[])
}

fn compare_uids(left: &str, right: &str) -> std::cmp::Ordering {
    match (left.parse::<u128>(), right.parse::<u128>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

// MODIFIED: removed yostar name filter so TW/CN company dirs are also matched
fn stella_sora_directories() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let local_low = home.join("AppData").join("LocalLow");
    let Ok(entries) = fs::read_dir(&local_low) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.path().join("StellaSora"))
        .filter(|dir| dir.is_dir())
        .collect()
}

fn find_yosdk_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in stella_sora_directories() {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            if entry.path().is_file()
                && entry
                    .file_name()
                    .to_str()
                    // MODIFIED: was starts_with("yosdk_"), narrowed to yosdk_userinfo
                    .map(|name| name.to_ascii_lowercase().starts_with("yosdk_userinfo"))
                    .unwrap_or(false)
            {
                files.push(entry.path());
            }
        }
    }

    // MODIFIED: added CN lookup under <company>/xtlr/CN_SDK_Cache/official/
    if let Some(home) = dirs::home_dir() {
        let local_low = home.join("AppData").join("LocalLow");
        if let Ok(entries) = fs::read_dir(&local_low) {
            for company in entries.filter_map(Result::ok) {
                if !company.path().is_dir() {
                    continue;
                }
                let cn = company
                    .path()
                    .join("xtlr")
                    .join("CN_SDK_Cache")
                    .join("official")
                    .join("sdk_cn_official_userinfo");
                if cn.is_file() {
                    files.push(cn);
                }
            }
        }
    }

    files
}

fn read_json_file(file: &Path) -> Result<Option<Value>> {
    let text = fs::read_to_string(file)?;
    let text = text.trim_start_matches('\u{feff}').trim();
    Ok(serde_json::from_str(text).ok())
}

fn parse_json_value(value: &Value) -> Option<Value> {
    if value.is_object() || value.is_array() {
        return Some(value.clone());
    }
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    serde_json::from_str(text).ok()
}

fn js_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn to_i64(value: &Value) -> i64 {
    if let Some(number) = value.as_i64() {
        return number;
    }
    if let Some(number) = value.as_f64() {
        return number as i64;
    }
    if let Some(text) = value.as_str() {
        return text.parse::<i64>().unwrap_or(0);
    }
    0
}

fn login_time(entry: &Value) -> i64 {
    entry.get("loginTime").map(to_i64).unwrap_or(0)
}

fn yosdk_region_from_file(file: &Path) -> Option<&'static str> {
    let name = file.file_name()?.to_str()?;
    let lower = name.to_ascii_lowercase();

    // MODIFIED: added CN branch returning lowercase "cn"
    if lower == "sdk_cn_official_userinfo" {
        return Some("cn");
    }
    if !lower.starts_with("yosdk_userinfo") {
        return None;
    }
    // MODIFIED: use get() instead of slicing to avoid panic on case variants
    let rest = name.get("YoSDK_UserInfo".len()..)?;
    let region = rest.trim_start_matches(['_', '-']);
    if region.is_empty() {
        return None;
    }
    normalize_region(region)
}

fn parse_yosdk_user_info(file: &Path) -> Result<Vec<Credentials>> {
    let Some(data) = read_json_file(file)? else {
        return Ok(Vec::new());
    };

    let mtime = fs::metadata(file)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    let region = yosdk_region_from_file(file);
    let mut users = Vec::new();

    if let Some(history_value) = data.get("all_user_login_history") {
        if let Some(history) = parse_json_value(history_value) {
            if let Some(history) = history.as_array() {
                let mut entries: Vec<Value> = history.iter().filter_map(parse_json_value).collect();
                entries.sort_by(|a, b| {
                    login_time(b)
                        .partial_cmp(&login_time(a))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let current_uid = non_empty(data.get("ID").and_then(js_string));
                let current_token = non_empty(data.get("Token").and_then(js_string));
                let fallback_device = entries
                    .iter()
                    .find_map(|entry| non_empty(entry.get("DeviceID").and_then(js_string)));

                for entry in entries {
                    let Some(uid) = non_empty(entry.get("ID").and_then(js_string)) else {
                        continue;
                    };
                    let token = non_empty(entry.get("Token").and_then(js_string)).or_else(|| {
                        if current_uid.as_deref() == Some(uid.as_str()) {
                            current_token.clone()
                        } else {
                            None
                        }
                    });
                    let Some(token) = token else { continue };
                    // MODIFIED: final fallback to Unity deviceUniqueIdentifier hash
                    let device = non_empty(entry.get("DeviceID").and_then(js_string))
                        .or_else(|| fallback_device.clone())
                        .or_else(|| {
                            crate::device::get_unity_device_unique_identifier().map(str::to_string)
                        });
                    let Some(device) = device else { continue };

                    users.push(Credentials {
                        uid,
                        token,
                        device,
                        file: file.to_path_buf(),
                        region,
                        mtime,
                        login_time: login_time(&entry),
                    });
                }
            }
        }
    }

    // MODIFIED: new format B for CN files (UserInfo + GameRole, strings inside JSON)
    if let Some(user_info) = data.get("UserInfo").and_then(parse_json_value) {
        let uid = non_empty(user_info.get("ID").and_then(js_string))
            .or_else(|| non_empty(user_info.get("PassId").and_then(js_string)));
        let token = non_empty(user_info.get("Token").and_then(js_string));
        let device = non_empty(user_info.get("DeviceID").and_then(js_string))
            .or_else(|| crate::device::get_unity_device_unique_identifier().map(str::to_string));

        if let (Some(uid), Some(token), Some(device)) = (uid, token, device) {
            users.push(Credentials {
                uid,
                token,
                device,
                file: file.to_path_buf(),
                region,
                mtime,
                login_time: 0,
            });
        }
    }

    Ok(users)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_file(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stella_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(name);
        fs::write(&file, body).unwrap();
        file
    }

    #[test]
    fn parse_both_formats_and_regions() {
        // MODIFIED: region assertions are lowercase to match normalize_region output
        assert_eq!(yosdk_region_from_file(Path::new("YoSDK_UserInfo_en")), Some("en"));
        assert_eq!(yosdk_region_from_file(Path::new("YoSDK_UserInfo_jp")), Some("jp"));
        assert_eq!(yosdk_region_from_file(Path::new("YoSDK_UserInfo_kr")), Some("kr"));
        assert_eq!(yosdk_region_from_file(Path::new("YoSDK_UserInfo_tw")), Some("tw"));
        assert_eq!(yosdk_region_from_file(Path::new("sdk_cn_official_userinfo")), Some("cn"));
        assert_eq!(yosdk_region_from_file(Path::new("other.bin")), None);

        // Format A: top-level all_user_login_history
        let fa = temp_file(
            "YoSDK_UserInfo_en",
            &json!({
                "ID": "1001",
                "Token": "tok_top",
                "all_user_login_history": [
                    { "ID": "1001", "Token": "tok_1001", "DeviceID": "dev_1001", "loginTime": 1700000000i64 },
                    { "ID": "1002", "Token": "tok_1002", "DeviceID": "dev_1002", "loginTime": 1700000100i64 }
                ]
            })
            .to_string(),
        );
        let users = parse_yosdk_user_info(&fa).unwrap();
        assert_eq!(users.len(), 2);
        assert!(users.iter().all(|u| u.region == Some("en")));
        assert_eq!(users[0].uid, "1001");
        assert_eq!(users[0].token, "tok_1001");

        // Format B: UserInfo / GameRole strings inside JSON
        let fb = temp_file(
            "sdk_cn_official_userinfo",
            &json!({
                "UserInfo": json!({
                    "ID": "2001",
                    "Token": "tok_cn",
                    "DeviceID": "dev_cn"
                }).to_string()
            })
            .to_string(),
        );
        let users = parse_yosdk_user_info(&fb).unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].uid, "2001");
        assert_eq!(users[0].token, "tok_cn");
        assert_eq!(users[0].region, Some("cn"));
    }

    #[test]
    #[ignore = "touches real LocalLow"]
    fn find_yosdk_files_on_real_machine() {
        let files = find_yosdk_files();
        println!("[real] {} candidate file(s):", files.len());
        for f in &files {
            println!("  - {}", f.display());
        }
        println!(
            "[real] unity_device_id = {:?}",
            crate::device::get_unity_device_unique_identifier()
        );
        for f in &files {
            for u in parse_yosdk_user_info(f).unwrap_or_default() {
                println!(
                    "[real] file={} uid={} device={} (len={})",
                    f.display(),
                    u.uid,
                    u.device,
                    u.device.len()
                );
            }
        }
    }
}
