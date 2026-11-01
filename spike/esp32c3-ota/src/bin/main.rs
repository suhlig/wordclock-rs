//! Over-the-air update spike for the ESP32-C3.
//!
//! The finished clock runs from a single 5 V supply, so USB is not available
//! for updates. This image provides the replacement: it joins Wi-Fi, then
//! serves a tiny HTTP endpoint that accepts a firmware image and writes it to
//! the inactive OTA slot.
//!
//! - `GET /` returns a plain-text status document (revision, uptime, slot).
//! - `PUT /ota` takes the raw `espflash save-image` output as the request body.
//!
//! Power it from the `5V` pin (header pin 19), never from USB and the 5 V rail
//! at the same time. See the crate README and design decision D9.

#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use core::fmt::Write as _;
#[cfg(feature = "strip-check")]
use core::sync::atomic::{AtomicBool, Ordering};

use alloc::string::String;

use embassy_executor::Spawner;
use embassy_net::tcp::{State, TcpSocket};
use embassy_net::{Ipv4Address, Ipv4Cidr, Runner, Stack, StackResources, StaticConfigV4};
use embassy_time::{Duration, Instant, Timer};
use esp_backtrace as _;
use esp_bootloader_esp_idf::ota::OtaImageState;
use esp_bootloader_esp_idf::ota_updater::OtaUpdater;
use esp_bootloader_esp_idf::partitions::{
    AppPartitionSubType, DataPartitionSubType, FlashRegion, FlashStorage, PARTITION_TABLE_MAX_LEN,
    PartitionType, read_partition_table,
};
use esp_hal::clock::CpuClock;
use esp_hal::rmt::Rmt;
use esp_hal::rng::Rng;
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Config as SpiConfig, Spi, SpiDma};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal_smartled::{RmtSmartLeds, buffer_size, color_order};
use esp_println::println;
use esp_radio::wifi::{
    AuthenticationMethodConfig, Config as WifiConfig, ControllerConfig, Interface, WifiController,
    sta::StationConfig,
};
use log::{error, info};
use smart_leds::{RGB8, SmartLedsWrite};
use static_cell::StaticCell;

use Outcome::{KeepRunning, Restart};

extern crate alloc;

// This creates the app descriptor the esp-idf bootloader validates.
esp_bootloader_esp_idf::esp_app_desc!();

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}

/// Bump this before every OTA experiment. The on-board LED blinks it at boot
/// and `GET /` reports it, which is how an update is verified without USB.
/// It sits at 12: the strip check, the boot-order work, the walking check for
/// the extended strip, and the move from RMT to SPI with DMA.
const REVISION: u32 = 12;

const OTA_PORT: u16 = 3232;
const HEADER_MAX: usize = 1024;
const PAGE_SIZE: usize = 4096;
const SOCKET_BUFFER: usize = 4096;

const SSID: &str = env!("WORDCLOCK_WIFI_SSID");
const PASSWORD: &str = env!("WORDCLOCK_WIFI_PASSWORD");
const STATIC_IP: &str = env!("WORDCLOCK_OTA_IP");
const STATIC_GATEWAY: &str = env!("WORDCLOCK_OTA_GATEWAY");

// Status colours, kept dim: this is the on-board WS2812, not the clock. They
// were halved after a night session, and the strip pattern went to a fifth.
const LED_BOOT: RGB8 = RGB8::new(0, 0, 16);
const LED_CONNECTING: RGB8 = RGB8::new(16, 8, 0);
const LED_READY: RGB8 = RGB8::new(0, 12, 0);
const LED_ERROR: RGB8 = RGB8::new(16, 0, 0);
const LED_OFF: RGB8 = RGB8::new(0, 0, 0);
const LED_BLINK: RGB8 = RGB8::new(12, 12, 12);

/// The first byte of every ESP application image.
const ESP_IMAGE_MAGIC: u8 = 0xE9;

type Leds<'a> =
    RmtSmartLeds<'a, { buffer_size::<RGB8>(1) }, esp_hal::Blocking, RGB8, color_order::Grb>;

/// The chain is the 35-pixel offcut plus the 300 LEDs added to it, so 335.
/// A frame must always cover the whole strip: a frame that stops early leaves
/// the remaining pixels at whatever they latched when the rail came up.
#[cfg(feature = "strip-check")]
const STRIP_LENGTH: usize = 335;

/// Dwell time on each pixel of the walk, in milliseconds. With 335 pixels one
/// sweep takes about 100 s.
#[cfg(feature = "strip-check")]
const WALK_STEP_MS: u64 = 300;

/// The walk colour: dim enough for a dark room, bright enough to see. Every
/// tenth pixel is green instead of white, so a failure can be counted to.
#[cfg(feature = "strip-check")]
const WALK_PIXEL: RGB8 = RGB8::new(16, 16, 16);
#[cfg(feature = "strip-check")]
const WALK_MARKER: RGB8 = RGB8::new(0, 16, 0);
#[cfg(feature = "strip-check")]
const WALK_MARKER_EVERY: usize = 10;

/// WS2812 through SPI: three SPI bits per LED bit at 2.5 MHz, so a `0` bit is
/// 100 (0.4 us high, 0.8 us low) and a `1` bit is 110 (0.8 us high, 0.4 us
/// low), both inside the WS2812B tolerance and exact at 80 MHz / 32.
///
/// The point of SPI here is DMA: the peripheral shifts the bits out of memory
/// with no CPU involvement, which is what RMT cannot do on this chip.
#[cfg(feature = "strip-check")]
const STRIP_BIT_BYTES: usize = STRIP_LENGTH * 9;

/// Trailing zero bytes hold the data line low for a reset. At 2.5 MHz, 20 bytes
/// are 160 bit times, 64 us, comfortably past the 50 us the strip needs.
#[cfg(feature = "strip-check")]
const STRIP_RESET_BYTES: usize = 20;

#[cfg(feature = "strip-check")]
type Strip = SpiDma<'static, esp_hal::Blocking>;

/// Whether the strip driver was created, so the status page can tell a wiring
/// problem apart from a firmware that does not drive the strip at all.
#[cfg(feature = "strip-check")]
static STRIP_READY: AtomicBool = AtomicBool::new(false);

/// The SPI+DMA strip driver lives in a static so the task only holds a reference.
#[cfg(feature = "strip-check")]
static STRIP: StaticCell<Strip> = StaticCell::new();

/// The frame being walked, about a kilobyte, so also a static.
#[cfg(feature = "strip-check")]
static STRIP_FRAME: StaticCell<[RGB8; STRIP_LENGTH]> = StaticCell::new();

/// The encoded bitstream handed to DMA: about 3 KiB for 335 pixels.
#[cfg(feature = "strip-check")]
static STRIP_BITS: StaticCell<[u8; STRIP_BIT_BYTES + STRIP_RESET_BYTES]> = StaticCell::new();

/// A byte buffer with flash-friendly alignment, so the flash layer never has
/// to stage a copy through its own sector buffer.
#[repr(align(4))]
struct Aligned<const N: usize>([u8; N]);

static FLASH: StaticCell<FlashStorage<'static>> = StaticCell::new();
static PARTITION_TABLE_BUFFER: StaticCell<Aligned<PARTITION_TABLE_MAX_LEN>> = StaticCell::new();
static PAGE: StaticCell<Aligned<PAGE_SIZE>> = StaticCell::new();
static HEADER: StaticCell<Aligned<HEADER_MAX>> = StaticCell::new();
static SOCKET_RX: StaticCell<Aligned<SOCKET_BUFFER>> = StaticCell::new();
static SOCKET_TX: StaticCell<Aligned<SOCKET_BUFFER>> = StaticCell::new();

fn set_led(led: &mut Leds<'_>, color: RGB8) {
    led.write([color]).expect("RMT write failed");
}

fn now_secs() -> u64 {
    Instant::now().as_secs()
}

/// The request line and the headers we care about.
struct Request {
    is_get: bool,
    is_put: bool,
    targets_ota: bool,
    content_length: Option<usize>,
    expect_continue: bool,
}

fn parse_request(header: &str) -> Result<Request, &'static str> {
    let mut lines = header.split("\r\n");
    let request_line = lines.next().ok_or("empty request")?;

    let mut parts = request_line.split(' ');
    let method = parts.next().ok_or("malformed request line")?;
    let target = parts.next().ok_or("malformed request line")?;

    let mut request = Request {
        is_get: method.eq_ignore_ascii_case("GET"),
        is_put: method.eq_ignore_ascii_case("PUT") || method.eq_ignore_ascii_case("POST"),
        targets_ota: target == "/ota",
        content_length: None,
        expect_continue: false,
    };

    for line in lines {
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();

        if name.eq_ignore_ascii_case("content-length") {
            request.content_length = value.parse().ok();
        } else if name.eq_ignore_ascii_case("expect") && value.eq_ignore_ascii_case("100-continue")
        {
            request.expect_continue = true;
        }
    }

    Ok(request)
}

fn find_headers_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

/// Reads the request head and returns it together with the body bytes that
/// arrived in the same segment.
async fn read_headers<'a>(
    socket: &mut TcpSocket<'_>,
    buffer: &'a mut Aligned<HEADER_MAX>,
) -> Result<(&'a str, &'a [u8]), &'static str> {
    let mut filled = 0;

    loop {
        if let Some(end) = find_headers_end(&buffer.0[..filled]) {
            let header = core::str::from_utf8(&buffer.0[..end])
                .map_err(|_| "headers are not valid UTF-8")?;
            return Ok((header, &buffer.0[end + 4..filled]));
        }

        if filled == buffer.0.len() {
            return Err("request headers are too large");
        }

        let read = socket
            .read(&mut buffer.0[filled..])
            .await
            .map_err(|_| "socket read failed")?;
        if read == 0 {
            return Err("connection closed before the request head was complete");
        }
        filled += read;
    }
}

async fn write_all(socket: &mut TcpSocket<'_>, mut bytes: &[u8]) -> Result<(), &'static str> {
    while !bytes.is_empty() {
        let written = socket
            .write(bytes)
            .await
            .map_err(|_| "socket write failed")?;
        if written == 0 {
            return Err("connection closed while writing");
        }
        bytes = &bytes[written..];
    }
    Ok(())
}

async fn respond(socket: &mut TcpSocket<'_>, status: &str, body: &str) -> Result<(), &'static str> {
    let mut head = String::new();
    let _ = write!(
        head,
        "HTTP/1.1 {status}\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );

    write_all(socket, head.as_bytes()).await?;
    write_all(socket, body.as_bytes()).await?;
    socket.flush().await.map_err(|_| "socket flush failed")
}

/// Waits until the response has left the device and the connection has closed,
/// so the client sees a clean end of response. Dropping the socket frees the
/// TCB for the next `accept`.
async fn close_gracefully(socket: &mut TcpSocket<'_>) {
    socket.close();

    // `TimeWait` means the peer has everything and only the lingering state is
    // left, which dropping the socket clears.
    for _ in 0..50 {
        if matches!(socket.state(), State::Closed | State::TimeWait) {
            return;
        }
        Timer::after(Duration::from_millis(20)).await;
    }

    socket.abort();
}

/// Buffers the incoming body into flash pages. `esp_storage` erases the
/// affected sector before writing, so no explicit erase is needed.
struct Upload {
    offset: u32,
    filled: usize,
}

impl Upload {
    fn flush(
        &mut self,
        region: &mut FlashRegion<'_, '_>,
        page: &mut Aligned<PAGE_SIZE>,
    ) -> Result<(), &'static str> {
        if self.filled == 0 {
            return Ok(());
        }

        let length = self.filled;
        if self.offset as usize + length > region.capacity() {
            return Err("image is larger than the OTA slot");
        }

        region
            .write(self.offset, &page.0[..length])
            .map_err(|_| "flash write failed")?;

        self.offset += length as u32;
        self.filled = 0;
        Ok(())
    }
}

/// Streams the request body into the inactive OTA slot and returns how many
/// bytes were written.
async fn receive_image(
    socket: &mut TcpSocket<'_>,
    region: &mut FlashRegion<'_, '_>,
    page: &mut Aligned<PAGE_SIZE>,
    leftover: &[u8],
    total: usize,
    led: &mut Leds<'_>,
) -> Result<u32, &'static str> {
    if !(1024..=region.capacity()).contains(&total) {
        return Err("refusing a body that is shorter than 1 KiB or larger than the slot");
    }
    if leftover.len() > total {
        return Err("more body bytes than announced");
    }

    let mut upload = Upload {
        offset: 0,
        filled: leftover.len(),
    };
    page.0[..leftover.len()].copy_from_slice(leftover);
    let mut received = leftover.len();

    while received < total {
        if upload.filled == PAGE_SIZE {
            upload.flush(region, page)?;
        }

        let room = (PAGE_SIZE - upload.filled).min(total - received);
        let read = socket
            .read(&mut page.0[upload.filled..upload.filled + room])
            .await
            .map_err(|_| "socket read failed")?;
        if read == 0 {
            return Err("connection closed before the body was complete");
        }

        upload.filled += read;
        received += read;

        let progress = ((received * 24) / total) as u8;
        set_led(led, RGB8::new(32 - progress, progress, 0));
    }

    upload.flush(region, page)?;

    // A wrong file is the most likely mistake here, and the magic byte is the
    // cheapest way to catch it before the slot is activated.
    let mut magic = [0u8; 4];
    region
        .read(0, &mut magic)
        .map_err(|_| "cannot read the image back")?;
    if magic[0] != ESP_IMAGE_MAGIC {
        return Err("the body is not an ESP application image");
    }

    Ok(upload.offset)
}

#[allow(
    clippy::large_stack_frames,
    reason = "a handful of write! calls and their format temporaries, on the server \
    path where the stack is not the constraint"
)]
fn status_document(
    flash: &mut FlashStorage<'static>,
    partition_buffer: &mut Aligned<PARTITION_TABLE_MAX_LEN>,
) -> String {
    let mut document = String::new();
    let _ = writeln!(document, "revision={REVISION}");
    let _ = writeln!(document, "uptime_s={}", now_secs());

    let stats = esp_alloc::HEAP.stats();
    let _ = writeln!(
        document,
        "heap_used={} heap_max={} heap_size={}",
        stats.current_usage, stats.max_usage, stats.size
    );
    let _ = writeln!(document, "strip_check={}", strip_state());
    let _ = writeln!(document, "strip_pixels={}", strip_pixels());

    match running_slot(flash, partition_buffer) {
        Ok(slot) => {
            let _ = writeln!(document, "running={slot:?}");
        }
        Err(reason) => {
            let _ = writeln!(document, "running=unknown reason={reason}");
        }
    }

    match table_summary(flash, partition_buffer) {
        Ok(summary) => {
            let _ = writeln!(document, "table={summary}");
        }
        Err(reason) => {
            let _ = writeln!(document, "table=unknown reason={reason}");
        }
    }

    match read_slot(flash, partition_buffer) {
        Ok((slot, state)) => {
            let _ = writeln!(document, "selected={}", describe(slot));
            let _ = writeln!(document, "slot_state={}", describe(state));
        }
        Err(reason) => {
            let _ = writeln!(
                document,
                "selected=unknown slot_state=unknown reason={reason}"
            );
        }
    }

    document
}

fn describe<T: core::fmt::Debug>(value: Option<T>) -> String {
    match value {
        // `otadata` is empty until the first update, which is a normal state
        // and not an error.
        None => "unset".into(),
        Some(value) => alloc::format!("{value:?}"),
    }
}

/// `off` when this image has no strip check, `failed` when the RMT channel could
/// not be set up, `on` when it is walking.
fn strip_state() -> &'static str {
    #[cfg(feature = "strip-check")]
    {
        if STRIP_READY.load(Ordering::Relaxed) {
            "on"
        } else {
            "failed"
        }
    }

    #[cfg(not(feature = "strip-check"))]
    {
        "off"
    }
}

/// How many pixels the walk covers, or zero when the feature is off.
fn strip_pixels() -> usize {
    #[cfg(feature = "strip-check")]
    {
        STRIP_LENGTH
    }

    #[cfg(not(feature = "strip-check"))]
    {
        0
    }
}

/// Counts what the OTA layer counts, so a mismatch between the flashed table
/// and what the updater expects is visible over the network instead of only in
/// a debugger.
fn table_summary(
    flash: &mut FlashStorage<'static>,
    partition_buffer: &mut Aligned<PARTITION_TABLE_MAX_LEN>,
) -> Result<String, &'static str> {
    let table = read_partition_table(&mut *flash, &mut partition_buffer.0)
        .map_err(|_| "cannot read the partition table")?;

    let mut entries = 0;
    let mut apps = 0;
    let mut ota_data = 0;
    let mut labels = String::new();

    for entry in table.iter() {
        entries += 1;
        match entry.partition_type() {
            PartitionType::App(subtype)
                if subtype != AppPartitionSubType::Factory
                    && subtype != AppPartitionSubType::Test =>
            {
                apps += 1;
            }
            PartitionType::Data(DataPartitionSubType::Ota) => ota_data += 1,
            _ => {}
        }
        let _ = write!(
            labels,
            " {}(0x{:x},0x{:x})",
            entry.label_as_str(),
            entry.offset(),
            entry.len()
        );
    }

    Ok(alloc::format!(
        "entries={entries} apps={apps} ota_data={ota_data} labels:{labels}"
    ))
}

/// The slot the running image was booted from, read from the MMU mapping. This
/// is the honest answer to "did the update take effect?": the selection in
/// otadata can point somewhere else than the image that actually booted.
fn running_slot(
    flash: &mut FlashStorage<'static>,
    partition_buffer: &mut Aligned<PARTITION_TABLE_MAX_LEN>,
) -> Result<AppPartitionSubType, &'static str> {
    let table = read_partition_table(&mut *flash, &mut partition_buffer.0)
        .map_err(|_| "cannot read the partition table")?;
    let entry = table
        .booted_partition()
        .map_err(|_| "cannot read the boot mapping")?
        .ok_or("the running partition is not in the table")?;

    match entry.partition_type() {
        PartitionType::App(subtype)
            if subtype != AppPartitionSubType::Factory && subtype != AppPartitionSubType::Test =>
        {
            Ok(subtype)
        }
        _ => Err("the running partition is not an OTA slot"),
    }
}

/// Makes `otadata` usable before serving requests.
///
/// A board that ran a different partition layout before keeps stale bytes in
/// what is now the second `otadata` slot, and the library rejects the whole
/// partition when either entry fails its CRC check. That is exactly what a
/// first flash over an old firmware produces, so scrub the slots and select the
/// running one; without this the first update fails with `Error::Invalid`.
fn prepare_ota_data(
    flash: &mut FlashStorage<'static>,
    partition_buffer: &mut Aligned<PARTITION_TABLE_MAX_LEN>,
) -> Result<AppPartitionSubType, &'static str> {
    let running = running_slot(flash, partition_buffer)?;

    let mut updater = OtaUpdater::new(flash, &mut partition_buffer.0)
        .map_err(|_| "cannot read the partition table")?;

    if updater.selected_partition().is_err() {
        info!("otadata is unusable; scrubbing both slots");
        updater
            .ota_data()
            .and_then(|mut ota| ota.set_current_app_partition(AppPartitionSubType::Factory))
            .map_err(|_| "cannot scrub otadata")?;
    }

    if let Ok(AppPartitionSubType::Factory) = updater.selected_partition() {
        updater
            .ota_data()
            .and_then(|mut ota| ota.set_current_app_partition(running))
            .map_err(|_| "cannot select the running slot")?;
        updater
            .set_current_ota_state(OtaImageState::Valid)
            .map_err(|_| "cannot mark the running image valid")?;
        info!("otadata selected {running:?}");
    }

    Ok(running)
}

fn read_slot(
    flash: &mut FlashStorage<'static>,
    partition_buffer: &mut Aligned<PARTITION_TABLE_MAX_LEN>,
) -> Result<(Option<AppPartitionSubType>, Option<OtaImageState>), &'static str> {
    let mut updater = OtaUpdater::new(flash, &mut partition_buffer.0)
        .map_err(|_| "cannot read the partition table")?;

    let selected = match updater.selected_partition() {
        Ok(value) => Some(value),
        Err(error) => {
            info!("selected_partition failed: {error:?}");
            None
        }
    };
    let state = match updater.current_ota_state() {
        Ok(value) => Some(value),
        Err(error) => {
            info!("current_ota_state failed: {error:?}");
            None
        }
    };

    Ok((selected, state))
}

/// Marks a freshly booted image as working. With rollback enabled the
/// bootloader would otherwise fall back to the previous slot; without it this
/// is a harmless bookkeeping write.
fn confirm_running_image(
    flash: &mut FlashStorage<'static>,
    partition_buffer: &mut Aligned<PARTITION_TABLE_MAX_LEN>,
) {
    let Ok(mut updater) = OtaUpdater::new(flash, &mut partition_buffer.0) else {
        return;
    };

    match updater.current_ota_state() {
        Ok(OtaImageState::Valid) => {}
        Ok(state) => {
            if updater.set_current_ota_state(OtaImageState::Valid).is_ok() {
                info!("confirmed the running image: {state:?} -> Valid");
            }
        }
        Err(reason) => info!("not confirming the running image: {reason:?}"),
    }
}

enum Outcome {
    Restart,
    KeepRunning,
}

#[allow(
    clippy::large_stack_frames,
    reason = "one request is handled at a time and the composed future is about \
    two kilobytes against a much larger main stack"
)]
async fn handle(
    socket: &mut TcpSocket<'_>,
    page: &mut Aligned<PAGE_SIZE>,
    header_buffer: &mut Aligned<HEADER_MAX>,
    flash: &mut FlashStorage<'static>,
    partition_buffer: &mut Aligned<PARTITION_TABLE_MAX_LEN>,
    led: &mut Leds<'_>,
) -> Result<Outcome, &'static str> {
    let (header, leftover) = read_headers(socket, header_buffer).await?;
    let request = parse_request(header)?;

    if request.is_get && !request.targets_ota {
        let body = status_document(flash, partition_buffer);
        respond(socket, "200 OK", &body).await?;
        return Ok(KeepRunning);
    }

    if !request.is_put || !request.targets_ota {
        respond(
            socket,
            "404 Not Found",
            "GET / for status, PUT /ota with an app image\n",
        )
        .await?;
        return Ok(KeepRunning);
    }

    let total = request
        .content_length
        .ok_or("PUT /ota needs a Content-Length header")?;

    if request.expect_continue {
        write_all(socket, b"HTTP/1.1 100 Continue\r\n\r\n").await?;
    }

    let mut updater =
        OtaUpdater::new(flash, &mut partition_buffer.0).map_err(|_| "cannot read the table")?;
    let (mut region, slot) = match updater.next_partition() {
        Ok(value) => value,
        Err(error) => {
            error!("next_partition failed: {error:?}");
            return Err("no inactive OTA slot available");
        }
    };

    if total > region.capacity() {
        respond(
            socket,
            "413 Payload Too Large",
            "image exceeds the OTA slot\n",
        )
        .await?;
        return Ok(KeepRunning);
    }

    info!("receiving {total} bytes into {slot:?}");

    let written = receive_image(socket, &mut region, page, leftover, total, led).await;

    let written = match written {
        Ok(written) => written,
        Err(reason) => {
            respond(socket, "500 Internal Server Error", reason).await?;
            return Ok(KeepRunning);
        }
    };

    respond(socket, "200 OK", &describe_update(written)).await?;

    updater
        .activate_next_partition()
        .map_err(|_| "cannot switch to the new OTA slot")?;
    updater
        .set_current_ota_state(OtaImageState::New)
        .map_err(|_| "cannot mark the new image as new")?;

    Ok(Restart)
}

fn describe_update(written: u32) -> String {
    let mut body = String::new();
    let _ = writeln!(body, "wrote {written} bytes");
    let _ = writeln!(body, "revision before the update: {REVISION}");
    let _ = writeln!(body, "restarting into the new image");
    body
}

#[allow(
    clippy::large_stack_frames,
    reason = "the buffers are constructed once at startup and moved into statics, so \
    they never live on the stack afterwards"
)]
async fn serve(
    stack: Stack<'static>,
    led: &mut Leds<'_>,
    flash: &mut FlashStorage<'static>,
    partition_buffer: &mut Aligned<PARTITION_TABLE_MAX_LEN>,
) -> ! {
    let page = PAGE.init(Aligned([0; PAGE_SIZE]));
    let header = HEADER.init(Aligned([0; HEADER_MAX]));
    let rx = SOCKET_RX.init(Aligned([0; SOCKET_BUFFER]));
    let tx = SOCKET_TX.init(Aligned([0; SOCKET_BUFFER]));

    loop {
        set_led(led, LED_READY);

        let mut socket = TcpSocket::new(stack, &mut rx.0[..], &mut tx.0[..]);
        if let Err(reason) = socket.accept(OTA_PORT).await {
            error!("cannot listen on port {OTA_PORT}: {reason:?}");
            Timer::after(Duration::from_secs(1)).await;
            continue;
        }
        socket.set_timeout(Some(Duration::from_secs(15)));

        match handle(
            &mut socket,
            &mut *page,
            &mut *header,
            &mut *flash,
            &mut *partition_buffer,
            &mut *led,
        )
        .await
        {
            Ok(Restart) => {
                info!("OTA image installed, restarting");
                close_gracefully(&mut socket).await;
                Timer::after(Duration::from_millis(200)).await;
                esp_hal::system::software_reset();
            }
            Ok(KeepRunning) => close_gracefully(&mut socket).await,
            Err(reason) => {
                error!("request failed: {reason}");
                set_led(led, LED_ERROR);
                Timer::after(Duration::from_secs(2)).await;
                close_gracefully(&mut socket).await;
            }
        }
    }
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);
    esp_alloc::heap_allocator!(size: 36 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    let frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, frequency).expect("Failed to initialize RMT");
    let mut led =
        RmtSmartLeds::<{ buffer_size::<RGB8>(1) }, _, RGB8, color_order::Grb>::new_with_memsize(
            esp_hal_smartled::WS2812_TIMING,
            rmt.channel0,
            peripherals.GPIO2,
            1,
            frequency,
        )
        .expect("Failed to initialize the status LED");

    // The strip runs on SPI with DMA, not RMT. An 8,042-code frame does not fit
    // the RMT's 48-code memory, so RMT has to refill it every 30 us from the
    // CPU, and Wi-Fi interrupts make it miss that deadline; the hardware then
    // re-sends the block and shifts the rest of the frame. SPI shifts the bits
    // out of memory with no CPU involvement at all. A failure here must not
    // take the OTA server down with it, because the only way back in is over
    // the air.
    #[cfg(feature = "strip-check")]
    let strip = match Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_khz(2_500))
            .with_mode(Mode::_0),
    ) {
        Ok(spi) => {
            STRIP_READY.store(true, Ordering::Relaxed);
            Some(
                spi.with_mosi(peripherals.GPIO3)
                    .with_dma(peripherals.DMA_CH0),
            )
        }
        Err(error) => {
            error!("strip check unavailable: {error:?}");
            None
        }
    };

    set_led(&mut led, LED_BOOT);

    // The first frames go out as soon as the driver exists, and three times: the
    // far end of a strip can still be finishing its power-on reset when the
    // first one arrives, and the clock redraws only once a minute, so a pixel
    // that misses the first frame stays wrong for a long time. Black, so a strip
    // that latched something while the rail was rising comes up dark.
    #[cfg(feature = "strip-check")]
    if let Some(strip) = strip {
        let strip = STRIP.init(strip);
        let frame = STRIP_FRAME.init([RGB8::new(0, 0, 0); STRIP_LENGTH]);
        let bits = STRIP_BITS.init([0u8; STRIP_BIT_BYTES + STRIP_RESET_BYTES]);

        for _ in 0..3 {
            write_frame(strip, frame, bits);
            Timer::after(Duration::from_millis(100)).await;
        }

        spawner.spawn(strip_check(strip, frame, bits).expect("failed to spawn the strip check"));
    }

    // The revision blink is the only update indicator when the board runs from
    // the 5 V rail with no USB and therefore no serial console.
    for _ in 0..REVISION {
        set_led(&mut led, LED_BLINK);
        Timer::after(Duration::from_millis(150)).await;
        set_led(&mut led, LED_OFF);
        Timer::after(Duration::from_millis(250)).await;
    }

    println!(
        "OTA revision={REVISION} port={OTA_PORT} ip={}",
        if STATIC_IP.is_empty() {
            "dhcp"
        } else {
            STATIC_IP
        }
    );

    if SSID.is_empty() || PASSWORD.is_empty() {
        panic!("set WORDCLOCK_WIFI_SSID and WORDCLOCK_WIFI_PASSWORD in wifi.env");
    }

    let flash = FLASH.init(FlashStorage::new(peripherals.FLASH));
    let partition_buffer = PARTITION_TABLE_BUFFER.init(Aligned([0; PARTITION_TABLE_MAX_LEN]));

    // Prepare `otadata` before Wi-Fi starts: the writes are short, and a failure
    // here must not cost the network path that reports it.
    match prepare_ota_data(&mut *flash, &mut *partition_buffer) {
        Ok(slot) => info!("running from {slot:?}; OTA ready"),
        Err(reason) => error!("cannot prepare otadata ({reason}); updates will not work"),
    }

    let station_config = WifiConfig::Station(
        StationConfig::default()
            .with_ssid(SSID.try_into().expect("SSID is too long"))
            .with_authentication(AuthenticationMethodConfig::Wpa2Personal(
                PASSWORD.try_into().expect("password is too long"),
            )),
    );

    let wifi_interface = Interface::station();
    let controller = WifiController::new(
        peripherals.WIFI,
        ControllerConfig::default().with_initial_config(station_config),
    )
    .expect("Failed to initialize the Wi-Fi controller");

    let net_config = network_config();
    let rng = Rng::new();
    let net_seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let (stack, runner) = embassy_net::new(
        wifi_interface,
        net_config,
        mk_static!(StackResources<4>, StackResources::<4>::new()),
        net_seed,
    );

    spawner.spawn(connection(controller).expect("failed to spawn the Wi-Fi task"));
    spawner.spawn(net_task(runner).expect("failed to spawn the network task"));
    spawner.spawn(heartbeat(stack).expect("failed to spawn the heartbeat"));

    set_led(&mut led, LED_CONNECTING);
    stack.wait_config_up().await;

    if let Some(config) = stack.config_v4() {
        println!(
            "OTA ready on http://{}:{OTA_PORT}/",
            config.address.address()
        );
    }

    // The image booted and the network came up, so it is good enough to keep.
    confirm_running_image(&mut *flash, &mut *partition_buffer);

    serve(stack, &mut led, flash, partition_buffer).await
}

fn network_config() -> embassy_net::Config {
    if STATIC_IP.is_empty() {
        return embassy_net::Config::dhcpv4(Default::default());
    }

    let address = parse_cidr(STATIC_IP).expect("WORDCLOCK_OTA_IP must look like 192.168.1.50/24");
    let gateway = if STATIC_GATEWAY.is_empty() {
        None
    } else {
        Some(parse_ipv4(STATIC_GATEWAY).expect("WORDCLOCK_OTA_GATEWAY must be an IPv4 address"))
    };

    embassy_net::Config::ipv4_static(StaticConfigV4 {
        address,
        gateway,
        dns_servers: Default::default(),
    })
}

fn parse_cidr(text: &str) -> Option<Ipv4Cidr> {
    let (address, prefix) = text.split_once('/')?;
    Some(Ipv4Cidr::new(parse_ipv4(address)?, prefix.parse().ok()?))
}

fn parse_ipv4(text: &str) -> Option<Ipv4Address> {
    let mut octets = [0u8; 4];
    let mut parts = text.split('.');

    for octet in &mut octets {
        *octet = parts.next()?.parse().ok()?;
    }

    if parts.next().is_some() {
        return None;
    }

    Some(Ipv4Address::new(octets[0], octets[1], octets[2], octets[3]))
}

#[allow(
    clippy::large_stack_frames,
    reason = "the association state machine fits comfortably in the task stack"
)]
#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    loop {
        info!("Connecting to Wi-Fi");
        match controller.connect_async().await {
            Ok(info) => {
                info!("Wi-Fi connected: {info:?}");
                let info = controller.wait_for_disconnect_async().await.ok();
                info!("Wi-Fi disconnected: {info:?}");
            }
            Err(error) => error!("Wi-Fi connection failed: {error:?}"),
        }

        Timer::after(Duration::from_secs(5)).await;
    }
}

/// Wiring check for the offcut strip on GPIO3: the primaries, a dim white, a
/// single-pixel index check, then off. It runs beside the OTA server so the
/// image can be replaced over the air from a phone or a shell.
#[cfg(feature = "strip-check")]
#[embassy_executor::task]
async fn strip_check(
    strip: &'static mut Strip,
    frame: &'static mut [RGB8; STRIP_LENGTH],
    bits: &'static mut [u8; STRIP_BIT_BYTES + STRIP_RESET_BYTES],
) {
    // One reusable frame, so the walk costs a single DMA transfer per step and
    // every frame covers the whole chain.
    loop {
        for index in 0..STRIP_LENGTH {
            frame[index] = if index.is_multiple_of(WALK_MARKER_EVERY) {
                WALK_MARKER
            } else {
                WALK_PIXEL
            };

            write_frame(strip, frame, bits);

            frame[index] = RGB8::new(0, 0, 0);
            Timer::after(Duration::from_millis(WALK_STEP_MS)).await;
        }
    }
}

/// Encodes a frame to the SPI bitstream and hands it to DMA.
#[cfg(feature = "strip-check")]
fn write_frame(
    strip: &mut Strip,
    frame: &[RGB8; STRIP_LENGTH],
    bits: &mut [u8; STRIP_BIT_BYTES + STRIP_RESET_BYTES],
) {
    let data = encode(frame, bits);
    let _ = strip.write(data);
}

/// Three SPI bits per LED bit, GRB order, most significant bit first.
///
/// A WS2812 `0` is `100` (0.4 us high, 0.8 us low) and a `1` is `110`
/// (0.8 us high, 0.4 us low), which at 2.5 MHz lands inside the part's
/// tolerance. The reset tail is left zero and is never rewritten.
#[cfg(feature = "strip-check")]
fn encode<'a>(frame: &[RGB8; STRIP_LENGTH], bits: &'a mut [u8]) -> &'a [u8] {
    let used = STRIP_BIT_BYTES;
    bits[..used].fill(0);

    let mut bit = 0;
    for pixel in frame {
        for value in [pixel.g, pixel.r, pixel.b] {
            for shift in (0..8).rev() {
                set_bit(bits, bit);
                if (value >> shift) & 1 == 1 {
                    set_bit(bits, bit + 1);
                }
                bit += 3;
            }
        }
    }

    &bits[..used + STRIP_RESET_BYTES]
}

#[cfg(feature = "strip-check")]
fn set_bit(bits: &mut [u8], index: usize) {
    bits[index / 8] |= 0x80 >> (index % 8);
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface>) {
    runner.run().await
}

/// Repeats the address every few seconds. The one-shot boot lines are usually
/// gone by the time a serial monitor attaches, which cost a debugging cycle
/// during this spike.
#[embassy_executor::task]
async fn heartbeat(stack: Stack<'static>) {
    loop {
        match stack.config_v4() {
            Some(config) => println!(
                "OTA revision={REVISION} on http://{}:{OTA_PORT}/",
                config.address.address()
            ),
            None => println!("OTA revision={REVISION} waiting for an address"),
        }

        Timer::after(Duration::from_secs(10)).await;
    }
}
