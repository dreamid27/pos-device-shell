// Minimal LLRP v1.1 client. Read-only inventory: opens TCP, deletes any prior
// ROSpec, adds a continuous "all antennas, immediate, no stop trigger" ROSpec,
// enables + starts it, then streams RO_ACCESS_REPORT messages, extracting EPCs.
//
// Spec reference: EPCglobal LLRP v1.1 (ISO/IEC 19762-3).
//
// What we DON'T implement (yet):
//   - Antenna power tuning
//   - Read frequency hopping config
//   - Tag write / kill / lock
//   - Custom parameter parsing (Impinj-specific)
//   - Re-connect on transport drop (caller restarts adapter)
//
// Wire format:
//   Header (10 bytes):
//     u16  [Rsvd:3 | Version:3 | MsgType:10]   (Version=1)
//     u32  message length (including header)
//     u32  message ID
//   Parameters: TLV
//     u16  [Rsvd:6 | ParamType:10]
//     u16  length (including 4-byte param header)
//     bytes payload
//   TV-encoded params (top bit = 1 in first byte): used inside TagReportData
//     u8   1xxxxxxx where x is type (0..127)
//     bytes payload (length determined by type)

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use chrono::Utc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::device_service::types::{RfidConfig, TagEvent};

use super::adapter::{RfidAdapter, TagReceiver, TagSender};

const CHANNEL_CAP: usize = 128;

// Message types
const MSG_ADD_ROSPEC: u16 = 20;
const MSG_DELETE_ROSPEC: u16 = 21;
const MSG_START_ROSPEC: u16 = 22;
const MSG_ENABLE_ROSPEC: u16 = 24;
const MSG_RO_ACCESS_REPORT: u16 = 61;
const MSG_KEEPALIVE: u16 = 62;
const MSG_KEEPALIVE_ACK: u16 = 72;

// Parameter types
const PARAM_RO_SPEC: u16 = 177;
const PARAM_RO_BOUNDARY_SPEC: u16 = 178;
const PARAM_RO_SPEC_START_TRIGGER: u16 = 179;
const PARAM_RO_SPEC_STOP_TRIGGER: u16 = 182;
const PARAM_AI_SPEC: u16 = 183;
const PARAM_AI_SPEC_STOP_TRIGGER: u16 = 184;
const PARAM_INVENTORY_PARAMETER_SPEC: u16 = 186;
const PARAM_TAG_REPORT_DATA: u16 = 240;
const PARAM_EPC_DATA: u16 = 241;

// TV (top-bit-set) parameter types — single byte type id (1..127).
const TV_EPC_96: u8 = 13;
const TV_ANTENNA_ID: u8 = 1;
const TV_PEAK_RSSI: u8 = 6;

const VERSION: u8 = 1;
const ROSPEC_ID: u32 = 1;

pub struct LlrpRfidAdapter {
    name: String,
    config: RfidConfig,
    tx: TagSender,
    state: Arc<StdMutex<LlrpState>>,
}

#[derive(Default)]
struct LlrpState {
    reader_task: Option<JoinHandle<()>>,
}

impl LlrpRfidAdapter {
    pub fn new(name: String, config: RfidConfig) -> anyhow::Result<Arc<Self>> {
        if config.host.is_none() {
            anyhow::bail!("RFID {name}: 'host' required for LLRP protocol");
        }
        let (tx, _rx) = broadcast::channel(CHANNEL_CAP);
        Ok(Arc::new(Self {
            name,
            config,
            tx,
            state: Arc::new(StdMutex::new(LlrpState::default())),
        }))
    }

    pub fn ensure_reader(self: &Arc<Self>) -> anyhow::Result<()> {
        let mut state = self.state.lock().expect("llrp state poisoned");
        if state.reader_task.is_some() {
            return Ok(());
        }
        let host = self
            .config
            .host
            .clone()
            .ok_or_else(|| anyhow::anyhow!("missing host"))?;
        let port = self.config.port.unwrap_or(5084);
        let me = Arc::clone(self);
        let handle = tokio::task::spawn_blocking(move || me.run(host, port));
        state.reader_task = Some(handle);
        Ok(())
    }

    fn run(self: Arc<Self>, host: String, port: u16) {
        let addr = format!("{host}:{port}");
        let mut stream = match TcpStream::connect_timeout(
            &match addr.parse() {
                Ok(a) => a,
                Err(e) => {
                    log::warn!("[rfid {}] bad address {addr}: {e}", self.name);
                    return;
                }
            },
            Duration::from_secs(5),
        ) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[rfid {}] connect {addr} failed: {e}", self.name);
                return;
            }
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_nodelay(true);

        // Reader sends READER_EVENT_NOTIFICATION on connect — read+discard one frame.
        let _ = read_message(&mut stream);

        // Best-effort cleanup any prior ROSpec.
        let mut msg_id: u32 = 1;
        let _ = send_message(&mut stream, MSG_DELETE_ROSPEC, msg_id, &u32_be(0));
        msg_id += 1;
        let _ = read_message(&mut stream);

        if let Err(e) = send_message(
            &mut stream,
            MSG_ADD_ROSPEC,
            msg_id,
            &build_inventory_rospec(),
        ) {
            log::warn!("[rfid {}] ADD_ROSPEC failed: {e}", self.name);
            return;
        }
        msg_id += 1;
        let _ = read_message(&mut stream);

        let _ = send_message(&mut stream, MSG_ENABLE_ROSPEC, msg_id, &u32_be(ROSPEC_ID));
        msg_id += 1;
        let _ = read_message(&mut stream);

        let _ = send_message(&mut stream, MSG_START_ROSPEC, msg_id, &u32_be(ROSPEC_ID));
        let _ = read_message(&mut stream);

        // Read reports until disconnect.
        loop {
            match read_message(&mut stream) {
                Ok((msg_type, payload)) => {
                    if msg_type == MSG_RO_ACCESS_REPORT {
                        for tag in decode_ro_access_report(&payload) {
                            let event = TagEvent {
                                reader: self.name.clone(),
                                epc: tag.epc,
                                tid: None,
                                rssi: tag.rssi,
                                antenna: tag.antenna.map(u32::from),
                                ts: Utc::now().to_rfc3339(),
                            };
                            let _ = self.tx.send(event);
                        }
                    } else if msg_type == MSG_KEEPALIVE {
                        // Reply so reader doesn't drop us.
                        let _ = send_message(&mut stream, MSG_KEEPALIVE_ACK, 0, &[]);
                    }
                }
                Err(e) => {
                    log::warn!("[rfid {}] disconnected: {e}", self.name);
                    break;
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl RfidAdapter for LlrpRfidAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn subscribe(&self) -> TagReceiver {
        self.tx.subscribe()
    }

    async fn close(&self) {
        if let Ok(mut s) = self.state.lock() {
            if let Some(h) = s.reader_task.take() {
                h.abort();
            }
        }
    }
}

// ===== framing =====

fn header_word(version: u8, msg_type: u16) -> u16 {
    // [Rsvd:3=0 | Version:3 | Type:10]
    ((u16::from(version & 0x07)) << 10) | (msg_type & 0x03ff)
}

fn send_message(
    stream: &mut TcpStream,
    msg_type: u16,
    msg_id: u32,
    payload: &[u8],
) -> std::io::Result<()> {
    let length: u32 = (10 + payload.len()) as u32;
    let mut buf = Vec::with_capacity(length as usize);
    buf.extend_from_slice(&header_word(VERSION, msg_type).to_be_bytes());
    buf.extend_from_slice(&length.to_be_bytes());
    buf.extend_from_slice(&msg_id.to_be_bytes());
    buf.extend_from_slice(payload);
    stream.write_all(&buf)
}

fn read_message(stream: &mut TcpStream) -> std::io::Result<(u16, Vec<u8>)> {
    let mut header = [0u8; 10];
    stream.read_exact(&mut header)?;
    let word = u16::from_be_bytes([header[0], header[1]]);
    let msg_type = word & 0x03ff;
    let length = u32::from_be_bytes([header[2], header[3], header[4], header[5]]);
    if (length as usize) < 10 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "llrp length < 10",
        ));
    }
    let payload_len = (length as usize) - 10;
    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        stream.read_exact(&mut payload)?;
    }
    Ok((msg_type, payload))
}

// ===== ROSpec builders =====

fn u16_be(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

fn u32_be(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

fn tlv(param_type: u16, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&u16_be(param_type & 0x03ff));
    let length: u16 = (4 + payload.len()) as u16;
    buf.extend_from_slice(&u16_be(length));
    buf.extend_from_slice(payload);
    buf
}

fn build_inventory_rospec() -> Vec<u8> {
    // ROSpecStartTrigger { Type=1 (Immediate) }
    let start_trigger = tlv(PARAM_RO_SPEC_START_TRIGGER, &[1u8]);
    // ROSpecStopTrigger { Type=0 (Null), DurationTrigger=0 }
    let stop_trigger = tlv(PARAM_RO_SPEC_STOP_TRIGGER, &[0u8, 0, 0, 0, 0]);
    // ROBoundarySpec { StartTrigger, StopTrigger }
    let mut boundary = Vec::new();
    boundary.extend_from_slice(&start_trigger);
    boundary.extend_from_slice(&stop_trigger);
    let boundary = tlv(PARAM_RO_BOUNDARY_SPEC, &boundary);

    // AISpecStopTrigger { Type=0 (Null), DurationTrigger=0 }
    let ai_stop = tlv(PARAM_AI_SPEC_STOP_TRIGGER, &[0u8, 0, 0, 0, 0]);
    // InventoryParameterSpec { Spec ID=1, Protocol=1 (EPCGlobalClass1Gen2) }
    let inventory_param = tlv(
        PARAM_INVENTORY_PARAMETER_SPEC,
        &[
            0, 1, // SpecID=1
            1, // ProtocolID=EPC C1G2
        ],
    );
    // AISpec { AntennaCount=1, AntennaIDs=[0] (all), StopTrigger, InventoryParameterSpec }
    let mut ai = Vec::new();
    ai.extend_from_slice(&u16_be(1)); // AntennaCount
    ai.extend_from_slice(&u16_be(0)); // AntennaIDs[0] = 0 = all
    ai.extend_from_slice(&ai_stop);
    ai.extend_from_slice(&inventory_param);
    let ai = tlv(PARAM_AI_SPEC, &ai);

    // ROSpec { ROSpecID, Priority=0, CurrentState=0 (Disabled), ROBoundarySpec, AISpec }
    let mut rospec = Vec::new();
    rospec.extend_from_slice(&u32_be(ROSPEC_ID));
    rospec.push(0); // Priority
    rospec.push(0); // CurrentState=Disabled — we ENABLE_ROSPEC explicitly after add
    rospec.extend_from_slice(&boundary);
    rospec.extend_from_slice(&ai);
    tlv(PARAM_RO_SPEC, &rospec)
}

// ===== RO_ACCESS_REPORT decoder =====

#[derive(Debug, Default)]
struct DecodedTag {
    epc: String,
    rssi: Option<i16>,
    antenna: Option<u16>,
}

fn decode_ro_access_report(payload: &[u8]) -> Vec<DecodedTag> {
    let mut tags = Vec::new();
    let mut i = 0usize;
    while i + 4 <= payload.len() {
        let raw_type = u16::from_be_bytes([payload[i], payload[i + 1]]) & 0x03ff;
        let length = u16::from_be_bytes([payload[i + 2], payload[i + 3]]) as usize;
        if length < 4 || i + length > payload.len() {
            break;
        }
        let body = &payload[i + 4..i + length];
        if raw_type == PARAM_TAG_REPORT_DATA {
            tags.push(decode_tag_report_data(body));
        }
        i += length;
    }
    tags
}

fn decode_tag_report_data(payload: &[u8]) -> DecodedTag {
    let mut tag = DecodedTag::default();
    let mut i = 0usize;
    while i < payload.len() {
        let first = payload[i];
        if first & 0x80 != 0 {
            // TV-encoded
            let tv_type = first & 0x7f;
            let consumed = match tv_type {
                TV_EPC_96 => {
                    if i + 1 + 12 <= payload.len() {
                        tag.epc = hex_upper(&payload[i + 1..i + 1 + 12]);
                    }
                    13
                }
                TV_ANTENNA_ID => {
                    if i + 1 + 2 <= payload.len() {
                        tag.antenna =
                            Some(u16::from_be_bytes([payload[i + 1], payload[i + 2]]));
                    }
                    3
                }
                TV_PEAK_RSSI => {
                    if i + 1 + 1 <= payload.len() {
                        tag.rssi = Some(payload[i + 1] as i8 as i16);
                    }
                    2
                }
                _ => {
                    // Unknown TV — without the spec table we can't know the size.
                    // Bail out of this report to avoid mis-aligned reads.
                    break;
                }
            };
            i += consumed;
        } else {
            if i + 4 > payload.len() {
                break;
            }
            let raw_type = u16::from_be_bytes([payload[i], payload[i + 1]]) & 0x03ff;
            let length = u16::from_be_bytes([payload[i + 2], payload[i + 3]]) as usize;
            if length < 4 || i + length > payload.len() {
                break;
            }
            let body = &payload[i + 4..i + length];
            if raw_type == PARAM_EPC_DATA && tag.epc.is_empty() {
                // EPCData: { LengthBits: u16, EPC: bits ... }
                if body.len() >= 2 {
                    let length_bits = u16::from_be_bytes([body[0], body[1]]) as usize;
                    let bytes = (length_bits + 7) / 8;
                    if 2 + bytes <= body.len() {
                        tag.epc = hex_upper(&body[2..2 + bytes]);
                    }
                }
            }
            i += length;
        }
    }
    tag
}

fn hex_upper(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02X}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_word_packs_version_and_type() {
        // Version=1, Type=20 (ADD_ROSPEC) → 0x0414
        assert_eq!(header_word(1, 20), 0x0414);
    }

    #[test]
    fn tlv_encodes_length() {
        let bytes = tlv(177, &[0xAA, 0xBB]);
        // type=177 (0x00B1), length=4+2=6, payload AA BB
        assert_eq!(bytes, vec![0x00, 0xB1, 0x00, 0x06, 0xAA, 0xBB]);
    }

    #[test]
    fn decode_tag_report_extracts_epc96_and_rssi() {
        let mut payload = Vec::new();
        // EPC-96 TV (type=13), 12 bytes EPC
        payload.push(0x80 | TV_EPC_96);
        payload.extend_from_slice(&[
            0xE2, 0x00, 0x34, 0x12, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF,
        ]);
        // AntennaID TV (type=1), u16
        payload.push(0x80 | TV_ANTENNA_ID);
        payload.extend_from_slice(&[0x00, 0x02]);
        // PeakRSSI TV (type=6), i8
        payload.push(0x80 | TV_PEAK_RSSI);
        payload.push((-55i8) as u8);

        let tag = decode_tag_report_data(&payload);
        assert_eq!(tag.epc, "E20034120123456789ABCDEF");
        assert_eq!(tag.antenna, Some(2));
        assert_eq!(tag.rssi, Some(-55));
    }
}
