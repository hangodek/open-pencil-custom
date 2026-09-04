//! Writes `~/.openpencil/.op-mcp-port` so the Rust `op` CLI and local agent
//! subprocesses can discover this running editor's live MCP server.
//!
//! Content is JSON `{port, pid, writerPid, token, timestamp, transport: "json-rpc"}`.
//! We only ever remove a file we ourselves wrote (`writerPid` guard) so a
//! second instance can't delete the live instance's discovery file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ANTIGRAVITY_MCP_PERMISSION: &str = "mcp(openpencil/*)";

fn port_file_path() -> Option<PathBuf> {
    op_config_store::ConfigStore::user()
        .ok()?
        .path(op_config_store::well_known::LIVE_MCP_PORT)
        .ok()
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Publish the live MCP server on `port` (with identity `token`) under
/// `~/.openpencil/.op-mcp-port`. Best-effort: failures are non-fatal.
pub fn write(port: u16, token: &str) {
    let Some(path) = port_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let pid = std::process::id();
    let body = port_record_json(port, pid, token, now_millis());
    let _ = std::fs::write(&path, body);
}

/// Build the discovery-file JSON record.
pub fn port_record_json(port: u16, pid: u32, token: &str, timestamp: u64) -> String {
    serde_json::json!({
        "port": port,
        "pid": pid,
        "writerPid": pid,
        "token": token,
        "timestamp": timestamp,
        "transport": "json-rpc"
    })
    .to_string()
}

/// Remove the discovery file published by this process. Only removes the file
/// if `writerPid` matches this process id.
pub fn remove() {
    let Some(path) = port_file_path() else {
        return;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return;
    };
    if val.get("writerPid").and_then(|v| v.as_u64()) == Some(u64::from(std::process::id())) {
        let _ = std::fs::remove_file(&path);
    }
}

/// Read the live MCP discovery record and return the port if it belongs to the
/// current process and uses json-rpc transport.
pub fn read_live_mcp_port_for_current_process(host_home: &Path) -> io::Result<u16> {
    let path = host_home
        .join(op_config_store::OPENPENCIL_DIR_NAME)
        .join(op_config_store::well_known::LIVE_MCP_PORT);
    let record: serde_json::Value = serde_json::from_slice(&fs::read(path)?).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid OpenPencil MCP discovery record: {e}"),
        )
    })?;
    let same_process = record.get("writerPid").and_then(serde_json::Value::as_u64)
        == Some(u64::from(std::process::id()));
    let json_rpc = record.get("transport").and_then(serde_json::Value::as_str) == Some("json-rpc");
    let port = record
        .get("port")
        .and_then(serde_json::Value::as_u64)
        .map(|p| p as u16);
    if same_process && json_rpc {
        port.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing port in MCP record"))
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no live MCP record for current process",
        ))
    }
}

/// Auto-configure host `~/.gemini/config/mcp_config.json` and
/// `~/.gemini/antigravity-cli/settings.json` so Antigravity CLI can connect
/// to the running OpenPencil MCP server without manual user intervention.
pub fn auto_configure_antigravity_mcp(host_home: &Path, port: u16) -> io::Result<()> {
    let config_path = host_home.join(".gemini/config/mcp_config.json");
    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut root: serde_json::Value = fs::read(&config_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !root.is_object() {
        root = serde_json::json!({});
    }
    let servers = root
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    if !servers.is_object() {
        *servers = serde_json::json!({});
    }
    servers.as_object_mut().unwrap().insert(
        "openpencil".to_string(),
        serde_json::json!({
            "serverUrl": format!("http://127.0.0.1:{port}/mcp")
        }),
    );
    write_private_json(&config_path, &root)?;

    let settings_path = host_home.join(".gemini/antigravity-cli/settings.json");
    if let Some(parent) = settings_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut settings_root: serde_json::Value = fs::read(&settings_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !settings_root.is_object() {
        settings_root = serde_json::json!({});
    }
    let perms = settings_root
        .as_object_mut()
        .unwrap()
        .entry("permissions")
        .or_insert_with(|| serde_json::json!({}));
    if !perms.is_object() {
        *perms = serde_json::json!({});
    }
    let allow = perms
        .as_object_mut()
        .unwrap()
        .entry("allow")
        .or_insert_with(|| serde_json::json!([]));
    if let Some(arr) = allow.as_array_mut() {
        if !arr
            .iter()
            .any(|v| v.as_str() == Some(ANTIGRAVITY_MCP_PERMISSION))
        {
            arr.push(serde_json::json!(ANTIGRAVITY_MCP_PERMISSION));
        }
    }
    write_private_json(&settings_path, &settings_root)?;
    Ok(())
}

fn write_private_json(path: &Path, value: &serde_json::Value) -> io::Result<()> {
    fs::write(path, serde_json::to_vec(value).map_err(io::Error::other)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
