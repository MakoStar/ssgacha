use std::sync::OnceLock;
use std::time::Duration;

const REPO: &str = "AutumnVN/ss-data";
const RAW_BASE: &str = "https://raw.githubusercontent.com/AutumnVN/ss-data/refs/heads/main";

#[derive(Clone, Copy, Debug)]
pub enum Mirror {
    Raw,
    JsDelivr(&'static str),
    Proxy(&'static str),
}

impl Mirror {
    pub fn build_url(&self, path: &str) -> String {
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        match self {
            Mirror::Raw => format!("{RAW_BASE}{path}"),
            Mirror::Proxy(proxy) => format!("{proxy}{RAW_BASE}{path}"),
            Mirror::JsDelivr(host) => {
                let clean = path.trim_start_matches('/');
                format!("https://{host}/gh/{REPO}@main/{clean}")
            }
        }
    }

    fn probe_url(&self) -> String {
        match self {
            Mirror::Raw => "https://raw.githubusercontent.com/".to_string(),
            Mirror::JsDelivr(host) => format!("https://{host}/"),
            Mirror::Proxy(proxy) => proxy.to_string(),
        }
    }
}

pub const MIRRORS: &[Mirror] = &[
    Mirror::Raw,
    Mirror::Proxy("https://githubproxy.cc/"),
    Mirror::Proxy("https://ghfast.top/"),
    Mirror::JsDelivr("jsd.onmicrosoft.cn"),
    Mirror::JsDelivr("fastly.jsdelivr.net"),
    Mirror::JsDelivr("gcore.jsdelivr.net"),
];

static LOCKED: OnceLock<Mirror> = OnceLock::new();

pub fn current() -> Mirror {
    LOCKED.get().copied().unwrap_or(MIRRORS[0])
}

#[allow(dead_code)]
pub fn build_url(path: &str) -> String {
    current().build_url(path)
}

fn same(a: &Mirror, b: &Mirror) -> bool {
    match (a, b) {
        (Mirror::Raw, Mirror::Raw) => true,
        (Mirror::JsDelivr(x), Mirror::JsDelivr(y)) => x == y,
        (Mirror::Proxy(x), Mirror::Proxy(y)) => x == y,
        _ => false,
    }
}

pub fn candidates() -> Vec<Mirror> {
    let cur = current();
    let mut out = vec![cur];
    for &m in MIRRORS {
        if !same(&m, &cur) {
            out.push(m);
        }
    }
    out
}

fn parse_env_mirror(value: &str) -> Option<Mirror> {
    match value.trim().to_ascii_lowercase().as_str() {
        "raw" => Some(Mirror::Raw),
        "jsdelivr" | "jsd" => Some(Mirror::JsDelivr("jsd.onmicrosoft.cn")),
        "fastly" => Some(Mirror::JsDelivr("fastly.jsdelivr.net")),
        "gcore" => Some(Mirror::JsDelivr("gcore.jsdelivr.net")),
        "proxy" | "githubproxy" => Some(Mirror::Proxy("https://githubproxy.cc/")),
        "ghfast" => Some(Mirror::Proxy("https://ghfast.top/")),
        _ => None,
    }
}

async fn probe(client: &reqwest::Client) -> Mirror {
    let timeout = probe_timeout();
    let mut set = tokio::task::JoinSet::new();
    for &mirror in MIRRORS {
        let client = client.clone();
        set.spawn(async move {
            let url = mirror.probe_url();
            let ok = matches!(
                tokio::time::timeout(timeout, client.head(&url).send()).await,
                Ok(Ok(_))
            );
            (mirror, ok)
        });
    }

    let mut results: Vec<(Mirror, bool)> = Vec::with_capacity(MIRRORS.len());
    while let Some(res) = set.join_next().await {
        if let Ok(r) = res {
            results.push(r);
        }
    }

    for &mirror in MIRRORS {
        if results.iter().any(|(m, ok)| *ok && same(m, &mirror)) {
            return mirror;
        }
    }
    Mirror::Raw
}

fn probe_timeout() -> Duration {
    std::env::var("SS_DATA_PROBE_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(2))
}

fn log_enabled() -> bool {
    match std::env::var("SS_MIRROR_LOG") {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

macro_rules! repolog {
    ($($arg:tt)*) => {
        if log_enabled() {
            eprintln!("{}", format_args!($($arg)*));
        }
    };
}

pub async fn init(client: &reqwest::Client) {
    if let Ok(forced) = std::env::var("SS_DATA_MIRROR") {
        match parse_env_mirror(&forced) {
            Some(mirror) => {
                let _ = LOCKED.set(mirror);
                repolog!("Repo mirror: {mirror:?} (forced by SS_DATA_MIRROR)");
                return;
            }
            None => repolog!("[mirror] unknown SS_DATA_MIRROR={forced}, probing repo mirrors"),
        }
    }

    let probed = probe(client).await;
    repolog!("Repo mirror: {probed:?}");
    let _ = LOCKED.set(probed);
}
