use std::process::Command;

use crate::device_service::types::{Config, DeviceStatus, PrinterInfo, PrintConnection, PrintProtocol};

struct DiscoveredPrinter {
    name: String,
    status: DeviceStatus,
}

fn parse_cups_status(line: &str) -> DeviceStatus {
    if line.contains("is idle") || line.contains("now printing") || line.contains("enabled since")
    {
        DeviceStatus::Ready
    } else if line.contains("disabled") || line.contains("paused") {
        DeviceStatus::Offline
    } else if line.contains("stopped") {
        DeviceStatus::Error
    } else {
        DeviceStatus::Ready
    }
}

fn cups_printers() -> Vec<DiscoveredPrinter> {
    let output = match Command::new("lpstat").arg("-p").output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|line| line.starts_with("printer "))
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            parts.next()?; // "printer"
            let name = parts.next()?.to_string();
            Some(DiscoveredPrinter {
                name,
                status: parse_cups_status(line),
            })
        })
        .collect()
}

fn windows_printers() -> Vec<DiscoveredPrinter> {
    let output = match Command::new("wmic")
        .args(["printer", "get", "name"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .skip(1)
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("name") {
                return None;
            }
            Some(DiscoveredPrinter {
                name: trimmed.to_string(),
                status: DeviceStatus::Ready,
            })
        })
        .collect()
}

fn system_printers() -> Vec<DiscoveredPrinter> {
    if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        cups_printers()
    } else {
        windows_printers()
    }
}

pub fn discover_printers(config: &Config) -> Vec<PrinterInfo> {
    let mut out: Vec<PrinterInfo> = Vec::new();
    let system = system_printers();

    for sp in &system {
        let cfg = config.printers.get(&sp.name);
        out.push(PrinterInfo {
            name: sp.name.clone(),
            protocol: cfg.map(|c| c.protocol).unwrap_or(PrintProtocol::System),
            connection: cfg.map(|c| c.connection).unwrap_or(PrintConnection::System),
            status: sp.status,
        });
    }

    for (name, cfg) in &config.printers {
        if matches!(cfg.protocol, PrintProtocol::System) {
            continue;
        }
        if out.iter().any(|p| &p.name == name) {
            continue;
        }
        out.push(PrinterInfo {
            name: name.clone(),
            protocol: cfg.protocol,
            connection: cfg.connection,
            status: DeviceStatus::Ready,
        });
    }

    out
}
