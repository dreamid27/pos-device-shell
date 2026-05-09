use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::device_service::types::{CommandKind, CutMode, PrintCommand, TextSize};

const ESC: u8 = 0x1B;
const GS: u8 = 0x1D;
const LF: u8 = 0x0A;

const ALIGN_LEFT: [u8; 3] = [ESC, 0x61, 0x00];
const ALIGN_CENTER: [u8; 3] = [ESC, 0x61, 0x01];
const ALIGN_RIGHT: [u8; 3] = [ESC, 0x61, 0x02];

const BOLD_ON: [u8; 3] = [ESC, 0x45, 0x01];
const BOLD_OFF: [u8; 3] = [ESC, 0x45, 0x00];
const UNDERLINE_ON: [u8; 3] = [ESC, 0x2D, 0x01];
const UNDERLINE_OFF: [u8; 3] = [ESC, 0x2D, 0x00];

const SIZE_NORMAL: [u8; 3] = [GS, 0x21, 0x00];
const SIZE_LARGE: [u8; 3] = [GS, 0x21, 0x11];
const SIZE_SMALL: [u8; 3] = [GS, 0x21, 0x00];

const CUT_FULL: [u8; 3] = [GS, 0x56, 0x00];
const CUT_PARTIAL: [u8; 3] = [GS, 0x56, 0x01];

const OPEN_DRAWER: [u8; 5] = [ESC, 0x70, 0x00, 0x19, 0xFA];
const INIT: [u8; 2] = [ESC, 0x40];

pub fn build_escpos_commands(commands: &[PrintCommand], paper_width: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(256);
    bytes.extend_from_slice(&INIT);

    for cmd in commands {
        match cmd.kind {
            CommandKind::Text => {
                if let Some(value) = &cmd.value {
                    bytes.extend_from_slice(value.as_bytes());
                    bytes.push(LF);
                }
            }
            CommandKind::Align => match cmd.value.as_deref() {
                Some("center") => bytes.extend_from_slice(&ALIGN_CENTER),
                Some("right") => bytes.extend_from_slice(&ALIGN_RIGHT),
                _ => bytes.extend_from_slice(&ALIGN_LEFT),
            },
            CommandKind::Style => {
                match cmd.bold {
                    Some(true) => bytes.extend_from_slice(&BOLD_ON),
                    Some(false) => bytes.extend_from_slice(&BOLD_OFF),
                    None => {}
                }
                match cmd.underline {
                    Some(true) => bytes.extend_from_slice(&UNDERLINE_ON),
                    Some(false) => bytes.extend_from_slice(&UNDERLINE_OFF),
                    None => {}
                }
                match cmd.size {
                    Some(TextSize::Large) => bytes.extend_from_slice(&SIZE_LARGE),
                    Some(TextSize::Small) => bytes.extend_from_slice(&SIZE_SMALL),
                    Some(TextSize::Normal) => bytes.extend_from_slice(&SIZE_NORMAL),
                    None => {}
                }
            }
            CommandKind::Columns => {
                if let (Some(values), Some(widths)) = (&cmd.values, &cmd.widths) {
                    let line = format_columns(values, widths, paper_width);
                    bytes.extend_from_slice(line.as_bytes());
                    bytes.push(LF);
                }
            }
            CommandKind::Feed => {
                let lines = cmd.lines.unwrap_or(1);
                for _ in 0..lines {
                    bytes.push(LF);
                }
            }
            CommandKind::Cut => match cmd.mode {
                Some(CutMode::Partial) => bytes.extend_from_slice(&CUT_PARTIAL),
                _ => bytes.extend_from_slice(&CUT_FULL),
            },
            CommandKind::Drawer => bytes.extend_from_slice(&OPEN_DRAWER),
            CommandKind::Raw => {
                if let Some(data) = &cmd.data {
                    if let Ok(decoded) = STANDARD.decode(data) {
                        bytes.extend_from_slice(&decoded);
                    }
                }
            }
            CommandKind::Barcode => {
                if let Some(value) = &cmd.value {
                    bytes.extend_from_slice(&ALIGN_CENTER);
                    bytes.extend_from_slice(format!("[{}]\n", value).as_bytes());
                    bytes.extend_from_slice(&ALIGN_LEFT);
                }
            }
            CommandKind::Image => {}
        }
    }
    bytes
}

fn format_columns(values: &[String], widths: &[u32], total_width: u32) -> String {
    let mut result = String::new();
    let last = values.len().saturating_sub(1);
    for (i, value) in values.iter().enumerate() {
        let width = widths.get(i).copied().unwrap_or(10) as usize;
        if i == last {
            result.push_str(&pad_start(value, width));
        } else {
            result.push_str(&pad_end(&truncate(value, width), width));
        }
    }
    let total = total_width as usize;
    if result.chars().count() > total {
        result = result.chars().take(total).collect();
    }
    result
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn pad_start(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count >= width {
        return s.to_string();
    }
    format!("{}{s}", " ".repeat(width - count))
}

fn pad_end(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count >= width {
        return s.to_string();
    }
    format!("{s}{}", " ".repeat(width - count))
}
