//! First-boot Wi-Fi provisioning over USB serial.
//!
//! The release image carries no credentials, so the first boot asks for the
//! SSID and password on the USB serial console, stores them in flash, and
//! reboots into station mode. A compile-time `wifi.env` skips the prompt.

use embassy_time::{Duration, Timer};
use esp_bootloader_esp_idf::partitions::FlashStorage;
use esp_hal::Blocking;
use esp_hal::usb::usb_serial_jtag::UsbSerialJtag;

use crate::config;

/// Asks for the credentials, stores them, and reboots.
///
/// Never returns: a successful save and a rejected pair both reboot, so the
/// next boot takes the normal station path.
pub async fn run(
    flash: &mut FlashStorage<'static>,
    mut usb: UsbSerialJtag<'static, Blocking>,
) -> ! {
    let _ = usb.write(b"\r\nwordclock setup\r\nSSID: ");

    let mut ssid = [0u8; 32];
    let ssid_len = read_line(&mut usb, &mut ssid).await;

    let _ = usb.write(b"\r\npassword: ");

    let mut password = [0u8; 64];
    let password_len = read_line(&mut usb, &mut password).await;

    let ssid = core::str::from_utf8(&ssid[..ssid_len]).unwrap_or("");
    let password = core::str::from_utf8(&password[..password_len]).unwrap_or("");

    match config::save(flash, ssid, password) {
        Ok(()) => {
            let _ = usb.write(b"\r\nsaved; rebooting\r\n");
        }
        Err(()) => {
            let _ = usb.write(b"\r\ninvalid credentials; rebooting\r\n");
        }
    }

    Timer::after(Duration::from_millis(500)).await;
    esp_hal::system::software_reset();
}

/// Reads one line, echoing it and handling backspace. Returns the length.
async fn read_line(usb: &mut UsbSerialJtag<'static, Blocking>, buffer: &mut [u8]) -> usize {
    let mut len = 0;

    loop {
        match usb.read_byte() {
            Ok(byte) => match byte {
                b'\r' | b'\n' => {
                    let _ = usb.write(b"\r\n");
                    return len;
                }
                0x08 | 0x7f => {
                    if len > 0 {
                        len -= 1;
                        let _ = usb.write(&[0x08, b' ', 0x08]);
                    }
                }
                byte if byte.is_ascii_graphic() || byte == b' ' => {
                    if len < buffer.len() {
                        buffer[len] = byte;
                        len += 1;
                        let _ = usb.write(&[byte]);
                    }
                }
                _ => {}
            },
            Err(nb::Error::WouldBlock) => Timer::after(Duration::from_millis(10)).await,
            Err(nb::Error::Other(_)) => return len,
        }
    }
}
