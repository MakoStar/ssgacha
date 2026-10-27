use sha1_smol::Sha1;
use std::process::Command;
use std::sync::OnceLock;

static UNITY_DEVICE_ID: OnceLock<Option<String>> = OnceLock::new();

// Get cached device identifier
pub fn get_unity_device_unique_identifier() -> Option<&'static str> {
    UNITY_DEVICE_ID.get_or_init(compute).as_deref()
}

// Compute device identifier from WMI hash falling back to registry
fn compute() -> Option<String> {
    let mut concat = String::new();

    for (class, field) in [
        ("Win32_BaseBoard", "SerialNumber"),
        ("Win32_BIOS", "SerialNumber"),
        ("Win32_OperatingSystem", "SerialNumber"),
    ] {
        let v = query_powershell(class, field).or_else(|| query_wmic(class, field));
        if let Some(s) = v {
            concat.push_str(&s);
        }
    }

    if concat.is_empty() {
        return query_machine_guid();
    }

    let mut hasher = Sha1::new();
    hasher.update(concat.as_bytes());
    Some(hasher.digest().to_string())
}

// Query WMI field via PowerShell
fn query_powershell(class: &str, field: &str) -> Option<String> {
    let cmd = format!("(Get-CimInstance {}).{}", class, field);
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

// Query WMI field via wmic
fn query_wmic(class: &str, field: &str) -> Option<String> {
    let arg = format!("{} get {} /value", class, field);
    let out = Command::new("wmic").arg(&arg).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let prefix = format!("{}=", field);
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix(&prefix) {
            let v = rest.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

// Read MachineGuid from registry
fn query_machine_guid() -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey(r"SOFTWARE\Microsoft\Cryptography").ok()?;
    let guid: String = key.get_value("MachineGuid").ok()?;
    let guid = guid.trim().to_string();
    if guid.is_empty() { None } else { Some(guid) }
}
