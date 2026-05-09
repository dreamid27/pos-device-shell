use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use uuid::Uuid;

fn strip_data_uri(data: &str) -> &str {
    if let Some(rest) = data.strip_prefix("data:") {
        if let Some(idx) = rest.find(',') {
            return &rest[idx + 1..];
        }
    }
    data
}

fn temp_path(suffix: &str) -> PathBuf {
    let mut p = env::temp_dir();
    p.push(format!("print-{}.{suffix}", Uuid::new_v4()));
    p
}

pub fn print_raw(raw_data: &[u8], printer_name: &str) -> anyhow::Result<()> {
    let path = temp_path("bin");
    fs::write(&path, raw_data)?;
    let path_str = path.to_string_lossy().into_owned();
    let result = if cfg!(target_os = "linux") {
        run_cmd("lp", &["-d", printer_name, "-o", "raw", &path_str])
    } else if cfg!(target_os = "macos") {
        run_cmd("lpr", &["-P", printer_name, "-o", "raw", &path_str])
    } else {
        Err(anyhow::anyhow!("Raw printing not supported on this platform"))
    };
    let _ = fs::remove_file(&path);
    result
}

pub fn print_pdf(pdf_data: &str, printer_name: &str) -> anyhow::Result<()> {
    let base64 = strip_data_uri(pdf_data);
    let bytes = STANDARD.decode(base64)?;
    let path = temp_path("pdf");
    fs::write(&path, &bytes)?;
    let path_str = path.to_string_lossy().into_owned();
    let result = if cfg!(target_os = "linux") {
        run_cmd("lp", &["-d", printer_name, &path_str])
    } else if cfg!(target_os = "macos") {
        run_cmd("lpr", &["-P", printer_name, &path_str])
    } else {
        Err(anyhow::anyhow!(
            "PDF printing on Windows requires platform-specific implementation"
        ))
    };
    let _ = fs::remove_file(&path);
    result
}

fn run_cmd(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{program} failed: {err}");
    }
    Ok(())
}
