#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginField {
    OfficialOverseas,
    Official,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMode {
    Normal,
    Cn,
}

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub key: &'static str,
    pub label: &'static str,
    pub sdk_url: &'static str,
    pub server_url: &'static str,
    pub garble_key: &'static [u8],
    pub language: &'static str,
    pub auth_lang: &'static str,
    pub pid: &'static str,
    pub data_path: &'static str,
    pub sdk_version: &'static str,
    pub login_field: LoginField,
    pub auth_mode: AuthMode,
}

pub const CLIENT_VERSION: &str = "727.727.727.7272727";
pub const SDK_VERSION: &str = "4.16.0";
pub const SDK_VERSION_CN: &str = "1.20.0";
pub const DEVICE_MODEL: &str = "AutumnVN";
pub const SPARK_THRESHOLD: u64 = 120;

pub const DATA_BASE: &str = "https://raw.githubusercontent.com/AutumnVN/ss-data/refs/heads/main";

pub const REGION_ORDER: [&str; 5] = ["en", "jp", "kr", "tw", "cn"];

pub const REGIONS: [Region; 5] = [
    Region {
        key: "en",
        label: "EN",
        sdk_url: "https://en-sdk-api.yostarplat.com",
        server_url: "https://nova.stellasora.global",
        garble_key: b"xNdVF^XTa6T3HCUATMQ@sKMLzAw&%L!3",
        language: "en_US",
        auth_lang: "en",
        pid: "US-NOVA",
        data_path: "EN",
        sdk_version: SDK_VERSION,
        login_field: LoginField::OfficialOverseas,
        auth_mode: AuthMode::Normal,
    },
    Region {
        key: "jp",
        label: "JP",
        sdk_url: "https://jp-sdk-api.yostarplat.com",
        server_url: "https://nova.stellasora.jp",
        garble_key: b"yX5Gt64PVvVH6$qwBXaPJC*LZKoK5mYh",
        language: "ja_JP",
        auth_lang: "ja",
        pid: "JP-NOVA",
        data_path: "JP",
        sdk_version: SDK_VERSION,
        login_field: LoginField::OfficialOverseas,
        auth_mode: AuthMode::Normal,
    },
    Region {
        key: "kr",
        label: "KR",
        sdk_url: "https://jp-sdk-api.yostarplat.com",
        server_url: "https://nova.stellasora.kr",
        garble_key: b"25hdume9H#*6hHn@d9hSF7tekTwN#JYj",
        language: "ko_KR",
        auth_lang: "ko",
        pid: "KR-NOVA",
        data_path: "KR",
        sdk_version: SDK_VERSION,
        login_field: LoginField::OfficialOverseas,
        auth_mode: AuthMode::Normal,
    },
    Region {
        key: "tw",
        label: "TW",
        sdk_url: "https://jp-sdk-api.yostarplat.com",
        server_url: "https://nova.stargazer-games.com",
        garble_key: b"N&mfco452ZH5!nE3s&o5uxB57UGPENVo",
        language: "zh_TW",
        auth_lang: "zh-Hant",
        pid: "TW-NOVA",
        data_path: "TW",
        sdk_version: SDK_VERSION,
        login_field: LoginField::OfficialOverseas,
        auth_mode: AuthMode::Normal,
    },
    Region {
        key: "cn",
        label: "CN",
        sdk_url: "https://sdk-api.yostar.cn",
        server_url: "https://nova.yostar.cn",
        garble_key: b"QW*Wi7fKjLk!T82Qf2nEGZA%nSC!D9qV",
        language: "zh_CN",
        auth_lang: "ChineseSimplified",
        pid: "CN-NOVA",
        data_path: "CN",
        sdk_version: SDK_VERSION_CN,
        login_field: LoginField::Official,
        auth_mode: AuthMode::Cn,
    },
];

pub fn region_by_key(key: &str) -> Option<&'static Region> {
    REGIONS.iter().find(|region| region.key == key)
}

pub fn normalize_region(value: &str) -> Option<&'static str> {
    let lower = value.trim().to_lowercase().replace('_', "-");
    Some(match lower.as_str() {
        "en" | "us" | "na" | "eu" => "en",
        "jp" | "ja" => "jp",
        "kr" | "ko" => "kr",
        "tw" | "zh-tw" => "tw",
        "cn" | "zh-cn" | "zh" => "cn",
        _ => return None,
    })
}

pub fn banner_url(region: &Region) -> String {
    format!("{DATA_BASE}/{}/bin/Gacha.json", region.data_path)
}

pub fn storage_url(region: &Region) -> String {
    format!("{DATA_BASE}/{}/bin/GachaStorage.json", region.data_path)
}

pub fn banner_lang_url(region: &Region) -> String {
    format!(
        "{DATA_BASE}/{}/language/{}/Gacha.json",
        region.data_path, region.language
    )
}

pub fn storage_lang_url(region: &Region) -> String {
    format!(
        "{DATA_BASE}/{}/language/{}/GachaStorage.json",
        region.data_path, region.language
    )
}

pub fn gacha_extra_url() -> String {
    format!("{DATA_BASE}/gacha.json")
}

pub fn item_url() -> String {
    format!("{DATA_BASE}/item.json")
}
