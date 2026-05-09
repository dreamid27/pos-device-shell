// Headless dev binary — runs the device service without spawning Tauri.
// Use for quick API smoke tests:
//   cargo run --bin standalone

use device_shell_lib::device_service;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init()
        .ok();
    device_service::serve(3333).await
}
