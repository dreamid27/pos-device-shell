use axum::{
    extract::Json as ExtractJson,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use uuid::Uuid;

use crate::device_service::config::load_config;
use crate::device_service::handlers::{
    escpos::build_escpos_commands,
    sbpl::{build_sbpl_commands, print_sbpl},
    system::{print_pdf, print_raw},
    zpl::build_zpl_commands,
};
use crate::device_service::types::{
    PrintConnection, PrintProtocol, PrintRequest, PrintResult,
};

pub fn router() -> Router {
    Router::new().route("/print", post(handler))
}

async fn handler(ExtractJson(req): ExtractJson<PrintRequest>) -> Response {
    if req.printer.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(PrintResult {
                success: false,
                job_id: None,
                message: Some("Printer name is required".into()),
                error: Some("MISSING_PRINTER".into()),
            }),
        )
            .into_response();
    }

    let job_id = format!("job_{}", &Uuid::new_v4().to_string()[..8]);
    let config = load_config();
    let printer_cfg = config.printers.get(&req.printer).cloned();

    let outcome = tokio::task::spawn_blocking({
        let printer = req.printer.clone();
        let job_id = job_id.clone();
        move || -> anyhow::Result<()> {
            match req.protocol {
                PrintProtocol::Escpos => {
                    let buffer: Vec<u8> = if let Some(raw) = req.data.raw.as_deref() {
                        STANDARD.decode(raw)?
                    } else if let Some(commands) = &req.data.commands {
                        let paper_width =
                            printer_cfg.as_ref().and_then(|c| c.paper_width).unwrap_or(42);
                        build_escpos_commands(commands, paper_width)
                    } else {
                        anyhow::bail!("Commands or raw data required for ESC/POS protocol");
                    };
                    let connection = printer_cfg
                        .as_ref()
                        .map(|c| c.connection)
                        .unwrap_or(PrintConnection::System);
                    if matches!(connection, PrintConnection::System) {
                        print_raw(&buffer, &printer)?;
                    }
                    log::info!(
                        "[print {job_id}] escpos printer={printer} bytes={}",
                        buffer.len()
                    );
                }
                PrintProtocol::Zpl => {
                    let commands = req
                        .data
                        .commands
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Commands required for ZPL protocol"))?;
                    let zpl = build_zpl_commands(commands);
                    log::info!(
                        "[print {job_id}] zpl printer={printer} chars={}",
                        zpl.len()
                    );
                }
                PrintProtocol::Sbpl => {
                    let buffer: Vec<u8> = if let Some(raw) = req.data.raw.as_deref() {
                        STANDARD.decode(raw)?
                    } else if let Some(commands) = &req.data.commands {
                        build_sbpl_commands(commands)
                    } else {
                        anyhow::bail!("Commands or raw data required for SBPL protocol");
                    };
                    print_sbpl(&buffer, &printer)?;
                    log::info!(
                        "[print {job_id}] sbpl printer={printer} bytes={}",
                        buffer.len()
                    );
                }
                PrintProtocol::System => {
                    let pdf = req
                        .data
                        .pdf
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("PDF data required for system protocol"))?;
                    print_pdf(pdf, &printer)?;
                    log::info!("[print {job_id}] system printer={printer}");
                }
            }
            Ok(())
        }
    })
    .await;

    match outcome {
        Ok(Ok(())) => Json(PrintResult {
            success: true,
            job_id: Some(job_id.clone()),
            message: Some(format!("Print job sent to {}", req.printer)),
            error: None,
        })
        .into_response(),
        Ok(Err(e)) => {
            log::error!("[print {job_id}] failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PrintResult {
                    success: false,
                    job_id: Some(job_id),
                    message: Some(e.to_string()),
                    error: Some("PRINT_FAILED".into()),
                }),
            )
                .into_response()
        }
        Err(join_err) => {
            log::error!("[print {job_id}] join error: {join_err}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PrintResult {
                    success: false,
                    job_id: Some(job_id),
                    message: Some(join_err.to_string()),
                    error: Some("PRINT_FAILED".into()),
                }),
            )
                .into_response()
        }
    }
}
