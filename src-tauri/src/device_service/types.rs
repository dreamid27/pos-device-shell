use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceStatus {
    Ready,
    Offline,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PrintProtocol {
    Escpos,
    Zpl,
    Sbpl,
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PrintConnection {
    Usb,
    Network,
    System,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrinterConfig {
    pub protocol: PrintProtocol,
    pub connection: PrintConnection,
    #[serde(rename = "vendorId")]
    pub vendor_id: Option<String>,
    #[serde(rename = "productId")]
    pub product_id: Option<String>,
    #[serde(rename = "paperWidth")]
    pub paper_width: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrinterInfo {
    pub name: String,
    pub protocol: PrintProtocol,
    pub connection: PrintConnection,
    pub status: DeviceStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommandKind {
    Text,
    Align,
    Style,
    Columns,
    Feed,
    Cut,
    Barcode,
    Image,
    Drawer,
    Raw,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextSize {
    Small,
    Normal,
    Large,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CutMode {
    Full,
    Partial,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BarcodeFormat {
    Code128,
    Ean13,
    Qr,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrintCommand {
    #[serde(rename = "type")]
    pub kind: CommandKind,
    pub value: Option<String>,
    pub values: Option<Vec<String>>,
    pub widths: Option<Vec<u32>>,
    pub bold: Option<bool>,
    pub underline: Option<bool>,
    pub size: Option<TextSize>,
    pub lines: Option<u32>,
    pub mode: Option<CutMode>,
    pub format: Option<BarcodeFormat>,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrintRequest {
    pub printer: String,
    pub protocol: PrintProtocol,
    pub data: PrintRequestData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrintRequestData {
    pub commands: Option<Vec<PrintCommand>>,
    pub pdf: Option<String>,
    pub raw: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ========================================================================
// Scales
// ========================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ScaleProtocol {
    Nci,
    Cas,
    Toledo,
    Avery,
    #[serde(rename = "hid-pos")]
    HidPos,
    Mock,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScaleTransport {
    Serial,
    #[serde(rename = "usb-hid")]
    UsbHid,
    Mock,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WeightUnit {
    Kg,
    G,
    Lb,
    Oz,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScaleConfig {
    pub protocol: ScaleProtocol,
    pub transport: ScaleTransport,
    pub path: Option<String>,
    pub baud: Option<u32>,
    #[serde(rename = "vendorId")]
    pub vendor_id: Option<String>,
    #[serde(rename = "productId")]
    pub product_id: Option<String>,
    pub unit: Option<WeightUnit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScaleInfo {
    pub name: String,
    pub protocol: ScaleProtocol,
    pub transport: ScaleTransport,
    pub status: DeviceStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeightReading {
    pub scale: String,
    pub value: f64,
    pub unit: WeightUnit,
    pub stable: bool,
    pub ts: String,
}

// ========================================================================
// RFID
// ========================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RfidProtocol {
    Llrp,
    #[serde(rename = "hid-keyboard")]
    HidKeyboard,
    #[serde(rename = "serial-proprietary")]
    SerialProprietary,
    Mock,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RfidTransport {
    Tcp,
    #[serde(rename = "usb-hid")]
    UsbHid,
    Serial,
    Mock,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RfidConfig {
    pub protocol: RfidProtocol,
    pub transport: RfidTransport,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: Option<String>,
    pub baud: Option<u32>,
    #[serde(rename = "vendorId")]
    pub vendor_id: Option<String>,
    #[serde(rename = "productId")]
    pub product_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RfidInfo {
    pub name: String,
    pub protocol: RfidProtocol,
    pub transport: RfidTransport,
    pub status: DeviceStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagEvent {
    pub reader: String,
    pub epc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rssi: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub antenna: Option<u32>,
    pub ts: String,
}

// ========================================================================
// Aggregate
// ========================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub printers: HashMap<String, PrinterConfig>,
    #[serde(default)]
    pub scales: HashMap<String, ScaleConfig>,
    #[serde(default)]
    pub rfid: HashMap<String, RfidConfig>,
}

fn default_port() -> u16 {
    3333
}

#[derive(Debug, Serialize)]
pub struct DeviceInventory {
    pub printers: Vec<PrinterInfo>,
    pub scales: Vec<ScaleInfo>,
    pub rfid: Vec<RfidInfo>,
}

#[derive(Debug, Serialize)]
pub struct SubsystemHealth {
    pub printers: SubsystemStatus,
    pub scales: SubsystemStatus,
    pub rfid: SubsystemStatus,
}

#[derive(Debug, Serialize)]
pub struct SubsystemStatus {
    pub ok: bool,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
}

impl ApiError {
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
        }
    }
}
