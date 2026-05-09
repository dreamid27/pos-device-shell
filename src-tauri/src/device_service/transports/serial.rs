use std::time::Duration;

use serialport::SerialPort;

pub struct SerialOptions {
    pub path: String,
    pub baud: u32,
}

pub fn open_serial(options: &SerialOptions) -> serialport::Result<Box<dyn SerialPort>> {
    serialport::new(&options.path, options.baud)
        .data_bits(serialport::DataBits::Eight)
        .stop_bits(serialport::StopBits::One)
        .parity(serialport::Parity::None)
        .timeout(Duration::from_millis(50))
        .open()
}
