use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::device_service::types::{BarcodeFormat, CommandKind, PrintCommand};

pub fn build_zpl_commands(commands: &[PrintCommand]) -> String {
    let mut lines: Vec<String> = vec!["^XA".to_string()];
    let mut y_pos: u32 = 50;

    for cmd in commands {
        match cmd.kind {
            CommandKind::Text => {
                if let Some(value) = &cmd.value {
                    lines.push(format!("^FO50,{}^A0N,30,30^FD{}^FS", y_pos, value));
                    y_pos += 40;
                }
            }
            CommandKind::Barcode => {
                if let Some(value) = &cmd.value {
                    let format = cmd.format.unwrap_or(BarcodeFormat::Code128);
                    match format {
                        BarcodeFormat::Code128 => {
                            lines.push(format!("^FO50,{}^BY3", y_pos));
                            lines.push(format!("^BCN,100,Y,N,N^FD{}^FS", value));
                        }
                        BarcodeFormat::Qr => {
                            lines.push(format!("^FO50,{}", y_pos));
                            lines.push(format!("^BQN,2,5^FDQA,{}^FS", value));
                        }
                        BarcodeFormat::Ean13 => {
                            lines.push(format!("^FO50,{}^BY3", y_pos));
                            lines.push(format!("^BEN,100,Y,N^FD{}^FS", value));
                        }
                    }
                    y_pos += 120;
                }
            }
            CommandKind::Feed => {
                y_pos += cmd.lines.unwrap_or(1) * 30;
            }
            CommandKind::Columns => {
                if let Some(values) = &cmd.values {
                    let text = values.join(" ");
                    lines.push(format!("^FO50,{}^A0N,30,30^FD{}^FS", y_pos, text));
                    y_pos += 40;
                }
            }
            CommandKind::Raw => {
                if let Some(data) = &cmd.data {
                    if let Ok(decoded) = STANDARD.decode(data) {
                        if let Ok(s) = String::from_utf8(decoded) {
                            lines.push(s);
                        }
                    }
                }
            }
            CommandKind::Style
            | CommandKind::Align
            | CommandKind::Cut
            | CommandKind::Drawer
            | CommandKind::Image => {}
        }
    }
    lines.push("^XZ".to_string());
    lines.join("\n")
}
