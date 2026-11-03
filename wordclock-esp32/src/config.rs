//! Wi-Fi credentials stored in flash.
//!
//! The release image ships without credentials. On first boot the firmware
//! either uses a compile-time `wifi.env` pair or prompts over USB serial, then
//! stores the pair in a small record in the free flash gap between `phy_init`
//! and `ota_0`. Later OTA images read it from there, so updating the code does
//! not need the credentials.

use esp_bootloader_esp_idf::partitions::FlashStorage;

/// Magic for the credential record: "WCFG".
const MAGIC: u32 = 0x5743_4647;

/// The unallocated gap between `phy_init` (ends at 0x12000) and `ota_0`
/// (starts at 0x20000) in `partitions.csv`.
const OFFSET: u32 = 0x12000;

/// Magic + lengths + SSID + password + CRC.
const LEN: usize = 4 + 1 + 1 + 32 + 64 + 4;
const CRC_END: usize = 4 + 1 + 1 + 32 + 64;

/// A stored Wi-Fi credential pair.
pub struct Credentials {
    ssid: [u8; 32],
    ssid_len: usize,
    password: [u8; 64],
    password_len: usize,
}

impl Credentials {
    /// Builds a pair, rejecting anything that does not fit the record.
    pub fn new(ssid: &str, password: &str) -> Option<Self> {
        let ssid = ssid.as_bytes();
        let password = password.as_bytes();

        if ssid.len() > 32 || password.len() > 64 {
            return None;
        }

        let mut credentials = Self {
            ssid: [0; 32],
            ssid_len: ssid.len(),
            password: [0; 64],
            password_len: password.len(),
        };
        credentials.ssid[..ssid.len()].copy_from_slice(ssid);
        credentials.password[..password.len()].copy_from_slice(password);

        Some(credentials)
    }

    /// The SSID as a string.
    pub fn ssid(&self) -> &str {
        core::str::from_utf8(&self.ssid[..self.ssid_len]).unwrap_or("")
    }

    /// The password as a string.
    pub fn password(&self) -> &str {
        core::str::from_utf8(&self.password[..self.password_len]).unwrap_or("")
    }
}

/// Reads the stored pair, or `None` when the record is absent or invalid.
pub fn load(flash: &mut FlashStorage<'static>) -> Option<Credentials> {
    let mut record = [0u8; LEN];
    flash.read(OFFSET, &mut record).ok()?;

    if u32::from_le_bytes(record[0..4].try_into().ok()?) != MAGIC {
        return None;
    }

    let ssid_len = usize::from(record[4]);
    let password_len = usize::from(record[5]);
    if ssid_len > 32 || password_len > 64 {
        return None;
    }

    let crc = u32::from_le_bytes(record[CRC_END..LEN].try_into().ok()?);
    if crc != crc32(&record[..CRC_END]) {
        return None;
    }

    let mut credentials = Credentials {
        ssid: [0; 32],
        ssid_len,
        password: [0; 64],
        password_len,
    };
    credentials.ssid[..ssid_len].copy_from_slice(&record[6..6 + ssid_len]);
    credentials.password[..password_len].copy_from_slice(&record[38..38 + password_len]);

    Some(credentials)
}

/// Writes the pair to flash. The sector is erased by the flash layer.
pub fn save(flash: &mut FlashStorage<'static>, ssid: &str, password: &str) -> Result<(), ()> {
    let Some(credentials) = Credentials::new(ssid, password) else {
        return Err(());
    };

    let mut record = [0u8; LEN];
    record[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    record[4] = credentials.ssid_len as u8;
    record[5] = credentials.password_len as u8;
    record[6..38].copy_from_slice(&credentials.ssid);
    record[38..102].copy_from_slice(&credentials.password);
    let crc = crc32(&record[..CRC_END]);
    record[CRC_END..LEN].copy_from_slice(&crc.to_le_bytes());

    flash.write(OFFSET, &record).map_err(|_| ())?;

    Ok(())
}

/// CRC-32 (IEEE), bitwise, so the store needs no extra crate.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;

    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }

    !crc
}
