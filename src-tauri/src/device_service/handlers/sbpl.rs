use std::io::Write;
use std::process::{Command, Stdio};

use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::device_service::types::{CommandKind, PrintCommand, TextSize};

pub fn build_sbpl_commands(commands: &[PrintCommand]) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(256);
    buf.extend_from_slice(b"\x1bA");

    let mut v_pos: u32 = 8;
    let h_pos: u32 = 6;

    for cmd in commands {
        match cmd.kind {
            CommandKind::Text => {
                if let Some(value) = &cmd.value {
                    let v = format!("{:04}", v_pos);
                    let h = format!("{:04}", h_pos);
                    let font = match cmd.size {
                        Some(TextSize::Large) => "0102",
                        Some(TextSize::Small) => "0100",
                        _ => "0101",
                    };
                    buf.extend_from_slice(
                        format!("\x1bV{v}\x1bH{h}\x1bL{font}\x1bM{value}").as_bytes(),
                    );
                    v_pos += if matches!(cmd.size, Some(TextSize::Large)) {
                        40
                    } else {
                        30
                    };
                }
            }
            CommandKind::Barcode => {
                if let Some(value) = &cmd.value {
                    let v = format!("{:04}", v_pos);
                    let h = format!("{:04}", h_pos);
                    let len = format!("{:02}", value.len());
                    buf.extend_from_slice(
                        format!("\x1bV{v}\x1bH{h}\x1b2D30,L,03,1,0\x1bDN{len},{value}")
                            .as_bytes(),
                    );
                    v_pos += 60;
                }
            }
            CommandKind::Feed => {
                v_pos += cmd.lines.unwrap_or(1) * 30;
            }
            CommandKind::Raw => {
                if let Some(data) = &cmd.data {
                    if let Ok(decoded) = STANDARD.decode(data) {
                        buf.extend_from_slice(&decoded);
                    }
                }
            }
            _ => {}
        }
    }

    buf.extend_from_slice(b"\x1bQ1\x1bZ");
    buf
}

pub fn print_sbpl(data: &[u8], printer_name: &str) -> anyhow::Result<()> {
    if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        run_with_stdin("lpr", &["-P", printer_name, "-oraw"], data)
    } else {
        run_with_stdin(
            "lpr",
            &["-S", "localhost", "-P", printer_name, "-o", "l"],
            data,
        )
    }
}

fn run_with_stdin(program: &str, args: &[&str], stdin_bytes: &[u8]) -> anyhow::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_bytes)?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{program} failed: {err}");
    }
    Ok(())
}
