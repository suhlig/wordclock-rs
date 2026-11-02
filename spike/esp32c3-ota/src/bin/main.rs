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
use core::net::{IpAddr, SocketAddr};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use alloc::string::String;

use embassy_executor::Spawner;
use embassy_net::dns::DnsQueryType;
use embassy_net::tcp::{State, TcpSocket};
use embassy_net::udp::{PacketMetadata, UdpSocket};
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
#[cfg(feature = "strip-check")]
use esp_hal::spi::Mode;
#[cfg(feature = "strip-check")]
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
use sntpc::{NtpContext, NtpTimestampGenerator, get_time};
use sntpc_net_embassy::UdpSocketWrapper;
use static_cell::StaticCell;

#[cfg(feature = "frame-check")]
use esp32c3_ota::LOGICAL_PIXELS;
#[cfg(feature = "frame-check")]
use esp32c3_ota::clock;
#[cfg(feature = "strip-check")]
use esp32c3_ota::color::Rgb;
#[cfg(feature = "frame-check")]
use esp32c3_ota::color::{ColorPicker, DEFAULT_BIRTHDAYS, GammaLut};
#[cfg(feature = "frame-check")]
use esp32c3_ota::frame::Frame;
#[cfg(feature = "strip-check")]
use esp32c3_ota::strip;

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
/// It sits at 24: 23 verified the four-bit strip cell at the production
/// colour, and this one adds SNTP and the status fields that a site survey
/// from a phone needs, without a USB cable at the wall.
const REVISION: u32 = 24;

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

/// The walk, and the payload it carries.
///
/// The default sweep is dim white, which proves the wiring. With `dense-check`
/// it carries the colour the fixture frame really renders instead, because a dim
/// white walk (`10`) and a fill at the low level (`06`) are both nearly all-zero
/// bits and neither exercises the shape the failing frame has: `ee0b01` is eight
/// ones in the green channel and almost nothing elsewhere.
///
/// Either way one pixel at a time keeps the current at roughly the 335 mA
/// quiescent term plus about 20 mA of drive.
#[cfg(all(
    feature = "strip-check",
    not(feature = "frame-check"),
    not(feature = "fill-check"),
    not(feature = "identity-check"),
    not(feature = "boundary-check")
))]
mod walk_pattern {
    use super::Rgb;

    #[cfg(not(feature = "dense-check"))]
    pub const STEP_MS: u64 = 300;
    #[cfg(not(feature = "dense-check"))]
    pub const PIXEL: Rgb = Rgb::new(16, 16, 16);
    /// Every tenth pixel is green instead of white, so a failure can be counted.
    #[cfg(not(feature = "dense-check"))]
    pub const MARKER: Rgb = Rgb::new(0, 16, 0);

    #[cfg(feature = "dense-check")]
    pub const STEP_MS: u64 = 400;
    #[cfg(feature = "dense-check")]
    pub const PIXEL: Rgb = Rgb::new(0xee, 0x0b, 0x01);
    /// Deliberately the same colour as [`PIXEL`].
    ///
    /// A marker made every tenth pixel green by design, which hid the thing we
    /// are chasing: a stray *green* pixel could always be the marker. With one
    /// colour for every step, a stray green can only be a stale byte left in the
    /// bitstream. Counting is no longer possible, which is the price.
    #[cfg(feature = "dense-check")]
    pub const MARKER: Rgb = PIXEL;

    pub const MARKER_EVERY: usize = 10;
}

/// One channel at a time, so a pixel showing any other colour proves that the
/// bytes on the wire are not the bytes we encoded. White is omitted: all three
/// channels at once triples the current for no extra information.
///
/// The level keeps the strip inside 500 mA on the bench supply. The project's
/// model is `pixels * 1 mA` quiescent plus `counts * 20/255 mA` of drive, so
/// 335 pixels at this level draw `335 + 335 * 6 * 20/255` = about 493 mA.
/// Raise it to 24 once a stronger supply feeds the strip: that is about 965 mA
/// and is bright enough to judge colour across a lit room.
///
/// Note what this cannot test: level 6 is `0b00000110`, whose last LED bit is a
/// `0`, so no pixel in this fill can trigger the boundary fault. It proves the
/// chain is uniformly addressable and nothing about the transport's timing.
/// `boundary-check` is the pattern for that.
#[cfg(all(
    feature = "fill-check",
    not(feature = "identity-check"),
    not(feature = "boundary-check")
))]
const FILL_LEVEL: u8 = 6;

#[cfg(all(feature = "fill-check", not(feature = "identity-check")))]
const FILL_COLOURS: [Rgb; 4] = [
    Rgb::new(FILL_LEVEL, 0, 0),
    Rgb::new(0, FILL_LEVEL, 0),
    Rgb::new(0, 0, FILL_LEVEL),
    Rgb::new(0, 0, 0),
];

/// Reframes per fill colour, one per second: long enough to walk the whole
/// chain, and a strip that reset itself recovers without a power cycle.
#[cfg(all(feature = "fill-check", not(feature = "identity-check")))]
const FILL_REFRAMES: u32 = 5;

/// A fixed run of known colours at the start of the chain, held still.
///
/// The walk moves, which makes it hard to tell a channel-order swap from pixel
/// data spreading over two chips. Static single channels can be read back
/// pixel by pixel in one glance: red, green, blue, yellow, white, then black.
/// If the chain shows those five colours in that order over five physical
/// pixels, the encoding is right and the walk's oddity is something else. If
/// the colours come out permuted, it is channel order. If they smear over seven
/// or eight pixels, one logical pixel is not one chip.
#[cfg(all(feature = "identity-check", not(feature = "boundary-check")))]
const IDENTITY_COLOURS: [Rgb; 6] = [
    Rgb::new(238, 0, 0),
    Rgb::new(0, 238, 0),
    Rgb::new(0, 0, 238),
    Rgb::new(238, 238, 0),
    Rgb::new(255, 255, 255),
    Rgb::new(0, 0, 0),
];

/// The alternating boundary pattern.
///
/// Every pixel that is not black carries the colour the fixture frame really
/// renders, `0xee0b01`. Its blue byte is `0x01` — a `0` followed by a `1`, the
/// shape that produced the stray — and its green byte is `0xee`, so the payload
/// is as dense as the frame the clock shows. The pixel after each lit one is
/// black, so a stray shows up as a lit pixel rather than as a slightly greener
/// neighbour, and 168 boundaries are live at once across the whole chain,
/// including the junction between the offcut and the 300 added to it.
///
/// A clean chain reads as one lit pixel, one dark pixel, repeating to the end.
/// The fixed fault reads as a red/green checkerboard, because every dark pixel
/// in between picks up `G = 0x80`.
///
/// Roughly 2 A, scaled from the 578 mA measured on the dim variant, so it needs
/// a bench supply comfortably above 1 A. For a small supply, drop it to
/// `Rgb::new(32, 8, 1)`: that keeps the `0` then `1` ending at about a sixth of
/// the drive, at the cost of a hue that is more orange and bytes that are
/// sparse.
#[cfg(feature = "boundary-check")]
const BOUNDARY_COLOUR: Rgb = Rgb::new(0xee, 0x0b, 0x01);

/// Every second pixel is lit, the rest stay black.
#[cfg(feature = "boundary-check")]
const BOUNDARY_PERIOD: usize = 2;

/// Where the identity run starts.
///
/// Moving it is the point of the constant. At offset 0 the reversed run lighted
/// pixels 0 to 4 and a stray green appeared at pixel 5, the first pixel after
/// the run. At offset 1 the run is pixels 1 to 5, so if the stray green moves to
/// pixel 6 the fault travels with the data and lives in the transport; if it
/// stays at pixel 5, that LED is faulty instead.
#[cfg(all(feature = "identity-check", not(feature = "boundary-check")))]
const IDENTITY_OFFSET: usize = 1;

/// The DMA buffer: one encoded frame plus its reset tail.
///
/// The waveform itself is not defined here. The SPI clock, the bit cell and the
/// reset length are in `src/strip.rs`, and that module's host tests derive the
/// four pulse widths and the reset from them and check them against the
/// `WS2812B` datasheet window. Nothing about the timing is tuned in this file.
#[cfg(feature = "strip-check")]
const STRIP_BUFFER_BYTES: usize = strip::buffer_len(STRIP_LENGTH);

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
static STRIP_FRAME: StaticCell<[Rgb; STRIP_LENGTH]> = StaticCell::new();

/// The encoded bitstream handed to DMA: 4 KiB for 335 pixels at four bits per
/// LED bit, plus the reset tail.
#[cfg(feature = "strip-check")]
static STRIP_BITS: StaticCell<[u8; STRIP_BUFFER_BYTES]> = StaticCell::new();

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

/// Fixture used by the frame-check build, matching `examples/dump_frame.rs`
/// and the frame the gate measured.
#[cfg(feature = "frame-check")]
const FIXTURE_SEED: u64 = 42;
#[cfg(feature = "frame-check")]
const FIXTURE_YEAR: i32 = 2026;
#[cfg(feature = "frame-check")]
const FIXTURE_MONTH: u32 = 7;
#[cfg(feature = "frame-check")]
const FIXTURE_DAY: u32 = 15;
#[cfg(feature = "frame-check")]
const FIXTURE_HOUR: u32 = 6;
#[cfg(feature = "frame-check")]
const FIXTURE_MINUTE: u32 = 30;

/// Renders the fixture frame with the real core and returns both the corrected
/// pixels for the strip and the dump text the host prints.
#[cfg(feature = "frame-check")]
#[allow(
    clippy::large_stack_frames,
    reason = "the gamma LUT is a bounded table and lives in the future or on the \
    server path, not in a hot loop"
)]
fn render_fixture() -> Result<(alloc::vec::Vec<Rgb>, String), &'static str> {
    let date = chrono::NaiveDate::from_ymd_opt(FIXTURE_YEAR, FIXTURE_MONTH, FIXTURE_DAY)
        .ok_or("invalid fixture date")?;

    let mut picker = ColorPicker::from_rng(
        fastrand::Rng::with_seed(FIXTURE_SEED),
        DEFAULT_BIRTHDAYS.to_vec(),
    );
    picker.begin_minute();

    let mut frame = Frame::blank(LOGICAL_PIXELS);
    for index in clock::pixels(FIXTURE_HOUR, FIXTURE_MINUTE) {
        frame.set(index, picker.choose(date, FIXTURE_HOUR, FIXTURE_MINUTE).rgb);
    }

    let corrected = frame.corrected_logical(&GammaLut::new([1.0, 1.0, 1.0]));

    let mut text = String::new();
    let _ = writeln!(
        text,
        "FRAME time={}-{:02}-{:02}T{:02}:{:02} color=none seed={FIXTURE_SEED}",
        FIXTURE_YEAR, FIXTURE_MONTH, FIXTURE_DAY, FIXTURE_HOUR, FIXTURE_MINUTE
    );
    for (index, (&raw, &shown)) in frame.logical().iter().zip(corrected.iter()).enumerate() {
        let _ = writeln!(
            text,
            "{index} {:02x}{:02x}{:02x} {:02x}{:02x}{:02x}",
            raw.r, raw.g, raw.b, shown.r, shown.g, shown.b
        );
    }
    let _ = writeln!(text, "END");

    Ok((corrected, text))
}

fn set_led(led: &mut Leds<'_>, color: RGB8) {
    led.write([color]).expect("RMT write failed");
}

fn now_secs() -> u64 {
    Instant::now().as_secs()
}

/// The SNTP server. Plain NTP over UDP, so no TLS and no certificate store.
const NTP_SERVER: &str = "pool.ntp.org";

/// How often to resynchronize. One minute matches the clock's own resolution
/// and costs nothing.
const SNTP_RESYNC_SECS: u64 = 60;

/// Whether an SNTP reply has been accepted since boot.
static SYNCED: AtomicBool = AtomicBool::new(false);

/// Unix seconds reported by the last accepted reply, zero before the first.
static LAST_SYNC_UNIX: AtomicU32 = AtomicU32::new(0);

/// Monotonic seconds at the last accepted reply, so the status page can show
/// how old the synchronization is without a wall clock.
static LAST_SYNC_AT: AtomicU32 = AtomicU32::new(0);

/// The DHCP lease in host byte order, zero until the address arrives.
static LOCAL_IP: AtomicU32 = AtomicU32::new(0);

/// Timestamp source for the SNTP request.
///
/// `sntpc` wants a local clock that is consistent between the send and the
/// receive; it takes the reply's NTP era from it rather than trusting it as
/// wall time, so a clock that starts at boot is correct here. That is why this
/// uses the embassy clock and not the RC `Rtc` — D20 forbids the `Rtc` as the
/// *display* time base, which is a different job.
#[derive(Clone, Copy)]
struct Timestamp {
    micros: u64,
}

impl NtpTimestampGenerator for Timestamp {
    fn init(&mut self) {
        self.micros = Instant::now().as_micros();
    }

    fn timestamp_sec(&self) -> u64 {
        self.micros / 1_000_000
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.micros % 1_000_000) as u32
    }
}

/// The request line and the headers we care about.
struct Request {
    is_get: bool,
    is_put: bool,
    targets_ota: bool,
    #[cfg_attr(not(feature = "frame-check"), allow(dead_code))]
    targets_frame: bool,
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
        targets_frame: target == "/frame",
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
    let _ = writeln!(document, "frame_check={}", cfg!(feature = "frame-check"));
    let _ = writeln!(document, "ip={}", ip_text());
    let _ = writeln!(
        document,
        "synced={} sync_age_s={} last_sync_unix={}",
        SYNCED.load(Ordering::Relaxed),
        sync_age(),
        LAST_SYNC_UNIX.load(Ordering::Relaxed)
    );

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

/// The lease as a dotted quad, or `none` before it arrives.
fn ip_text() -> String {
    let raw = LOCAL_IP.load(Ordering::Relaxed);

    if raw == 0 {
        return "none".into();
    }

    alloc::format!(
        "{}.{}.{}.{}",
        (raw >> 24) & 0xff,
        (raw >> 16) & 0xff,
        (raw >> 8) & 0xff,
        raw & 0xff
    )
}

/// Seconds since the last accepted SNTP reply, or `none` if there never was one.
fn sync_age() -> String {
    if !SYNCED.load(Ordering::Relaxed) {
        return "none".into();
    }

    let at = LAST_SYNC_AT.load(Ordering::Relaxed) as u64;
    alloc::format!("{}", now_secs().saturating_sub(at))
}

fn describe<T: core::fmt::Debug>(value: Option<T>) -> String {
    match value {
        // `otadata` is empty until the first update, which is a normal state
        // and not an error.
        None => "unset".into(),
        Some(value) => alloc::format!("{value:?}"),
    }
}

/// `off` when this image has no strip check, `failed` when the SPI+DMA driver
/// could not be set up, `on` when it is driving the chain.
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

    if request.is_get && !request.targets_ota && !request.targets_frame {
        let body = status_document(flash, partition_buffer);
        respond(socket, "200 OK", &body).await?;
        return Ok(KeepRunning);
    }

    #[cfg(feature = "frame-check")]
    if request.is_get && request.targets_frame {
        match render_fixture() {
            Ok((_, text)) => respond(socket, "200 OK", &text).await?,
            Err(reason) => respond(socket, "500 Internal Server Error", reason).await?,
        }
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
            .with_frequency(Rate::from_khz(strip::SPI_KHZ))
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
        let frame = STRIP_FRAME.init([Rgb::BLACK; STRIP_LENGTH]);
        let bits = STRIP_BITS.init([0u8; STRIP_BUFFER_BYTES]);

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
    spawner.spawn(sntp_task(stack).expect("failed to spawn the SNTP task"));

    set_led(&mut led, LED_CONNECTING);
    stack.wait_config_up().await;

    if let Some(config) = stack.config_v4() {
        LOCAL_IP.store(
            u32::from_be_bytes(config.address.address().octets()),
            Ordering::Relaxed,
        );
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

/// Drives the strip beside the OTA server, with the diagnostic the build selects.
#[cfg(feature = "strip-check")]
#[embassy_executor::task]
async fn strip_check(
    strip: &'static mut Strip,
    frame: &'static mut [Rgb; STRIP_LENGTH],
    bits: &'static mut [u8; STRIP_BUFFER_BYTES],
) {
    #[cfg(feature = "boundary-check")]
    hold_boundary(strip, frame, bits).await;

    #[cfg(all(feature = "identity-check", not(feature = "boundary-check")))]
    hold_identity(strip, frame, bits).await;

    #[cfg(all(
        feature = "fill-check",
        not(feature = "identity-check"),
        not(feature = "boundary-check")
    ))]
    hold_fill(strip, frame, bits).await;

    #[cfg(all(
        feature = "frame-check",
        not(feature = "fill-check"),
        not(feature = "identity-check"),
        not(feature = "boundary-check")
    ))]
    hold_fixture(strip, frame, bits).await;

    #[cfg(not(any(
        feature = "frame-check",
        feature = "fill-check",
        feature = "identity-check",
        feature = "boundary-check"
    )))]
    walk(strip, frame, bits).await;
}

/// Holds the alternating boundary pattern, re-sent once a second.
///
/// Static rather than a walk, because the readback is a count: on a clean chain
/// the lit pixels are every second one and the dark ones in between stay dark.
/// Re-sending it means a strip that reset itself recovers without a power cycle,
/// and a marginal fault has 168 boundaries per frame to show itself on.
#[cfg(feature = "boundary-check")]
async fn hold_boundary(
    strip: &'static mut Strip,
    frame: &'static mut [Rgb; STRIP_LENGTH],
    bits: &'static mut [u8; STRIP_BUFFER_BYTES],
) -> ! {
    for (index, slot) in frame.iter_mut().enumerate() {
        *slot = if index.is_multiple_of(BOUNDARY_PERIOD) {
            BOUNDARY_COLOUR
        } else {
            Rgb::BLACK
        };
    }

    loop {
        write_frame(strip, frame, bits);
        Timer::after(Duration::from_secs(1)).await;
    }
}

/// Holds a fixed run of known colours at the start of the chain.
#[cfg(all(feature = "identity-check", not(feature = "boundary-check")))]
async fn hold_identity(
    strip: &'static mut Strip,
    frame: &'static mut [Rgb; STRIP_LENGTH],
    bits: &'static mut [u8; STRIP_BUFFER_BYTES],
) -> ! {
    frame.fill(Rgb::BLACK);
    for (offset, colour) in IDENTITY_COLOURS.iter().enumerate() {
        if let Some(slot) = frame.get_mut(IDENTITY_OFFSET + offset) {
            *slot = *colour;
        }
    }

    loop {
        write_frame(strip, frame, bits);
        Timer::after(Duration::from_secs(1)).await;
    }
}

/// Fills every pixel with the same colour and cycles through the primaries.
///
/// The frame the clock renders is already uniform, so identical bytes to every
/// pixel are the sharpest transport test available: whatever the chain shows
/// here was not chosen by the colour code.
#[cfg(all(
    feature = "fill-check",
    not(feature = "identity-check"),
    not(feature = "boundary-check")
))]
async fn hold_fill(
    strip: &'static mut Strip,
    frame: &'static mut [Rgb; STRIP_LENGTH],
    bits: &'static mut [u8; STRIP_BUFFER_BYTES],
) -> ! {
    loop {
        for colour in FILL_COLOURS {
            frame.fill(colour);

            for _ in 0..FILL_REFRAMES {
                write_frame(strip, frame, bits);
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }
}

/// Shows the fixture frame and rewrites it every second, so a strip reset
/// recovers without a power cycle.
#[cfg(all(
    feature = "frame-check",
    not(feature = "fill-check"),
    not(feature = "identity-check"),
    not(feature = "boundary-check")
))]
async fn hold_fixture(
    strip: &'static mut Strip,
    frame: &'static mut [Rgb; STRIP_LENGTH],
    bits: &'static mut [u8; STRIP_BUFFER_BYTES],
) -> ! {
    let (pixels, _) = render_fixture().expect("the fixture frame must render");

    for (slot, pixel) in frame.iter_mut().zip(pixels.iter()) {
        *slot = *pixel;
    }

    loop {
        write_frame(strip, frame, bits);
        Timer::after(Duration::from_secs(1)).await;
    }
}

/// Walks one dim pixel through the whole chain.
#[cfg(all(
    feature = "strip-check",
    not(any(
        feature = "frame-check",
        feature = "fill-check",
        feature = "identity-check",
        feature = "boundary-check"
    ))
))]
async fn walk(
    strip: &'static mut Strip,
    frame: &'static mut [Rgb; STRIP_LENGTH],
    bits: &'static mut [u8; STRIP_BUFFER_BYTES],
) -> ! {
    loop {
        for index in 0..STRIP_LENGTH {
            frame[index] = if index.is_multiple_of(walk_pattern::MARKER_EVERY) {
                walk_pattern::MARKER
            } else {
                walk_pattern::PIXEL
            };

            write_frame(strip, frame, bits);

            frame[index] = Rgb::BLACK;
            Timer::after(Duration::from_millis(walk_pattern::STEP_MS)).await;
        }
    }
}

/// Encodes a frame to the SPI bitstream and hands it to DMA.
#[cfg(feature = "strip-check")]
fn write_frame(
    strip: &mut Strip,
    frame: &[Rgb; STRIP_LENGTH],
    bits: &mut [u8; STRIP_BUFFER_BYTES],
) {
    let Some(data) = strip::encode(frame, bits) else {
        error!("the strip buffer cannot hold {STRIP_LENGTH} pixels");
        return;
    };

    if let Err(error) = strip.write(data) {
        error!("strip write failed: {error:?}");
    }
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
            Some(config) => {
                LOCAL_IP.store(
                    u32::from_be_bytes(config.address.address().octets()),
                    Ordering::Relaxed,
                );
                println!(
                    "OTA revision={REVISION} on http://{}:{OTA_PORT}/ synced={}",
                    config.address.address(),
                    SYNCED.load(Ordering::Relaxed)
                );
            }
            None => println!("OTA revision={REVISION} waiting for an address"),
        }

        Timer::after(Duration::from_secs(10)).await;
    }
}

/// Resynchronizes from SNTP and records the result for the status page.
///
/// It deliberately does not touch the display: a missing time source must never
/// change a frame. It waits for the lease, resolves the pool, then queries once
/// a minute, so the status page can report the age of the last good reply.
#[allow(
    clippy::large_stack_frames,
    reason = "an embassy task future lives in the executor's static task arena, not on \
    the stack; the size comes from the DNS and SNTP futures, which are outside this crate"
)]
#[embassy_executor::task]
async fn sntp_task(stack: Stack<'static>) {
    loop {
        if stack.config_v4().is_none() {
            Timer::after(Duration::from_secs(1)).await;
            continue;
        }

        let address: IpAddr = match stack.dns_query(NTP_SERVER, DnsQueryType::A).await {
            Ok(addresses) if !addresses.is_empty() => addresses[0].into(),
            Ok(_) => {
                error!("DNS returned no address for {NTP_SERVER}");
                Timer::after(Duration::from_secs(10)).await;
                continue;
            }
            Err(error) => {
                error!("DNS query failed: {error:?}");
                Timer::after(Duration::from_secs(10)).await;
                continue;
            }
        };

        let mut rx_meta = [PacketMetadata::EMPTY; 2];
        let mut rx_buffer = [0u8; 256];
        let mut tx_meta = [PacketMetadata::EMPTY; 2];
        let mut tx_buffer = [0u8; 256];
        let mut socket = UdpSocket::new(
            stack,
            &mut rx_meta,
            &mut rx_buffer,
            &mut tx_meta,
            &mut tx_buffer,
        );

        if let Err(error) = socket.bind(123) {
            error!("cannot bind the SNTP socket: {error:?}");
            Timer::after(Duration::from_secs(10)).await;
            continue;
        }

        let socket = UdpSocketWrapper::new(socket);

        match get_time(
            SocketAddr::from((address, 123)),
            &socket,
            NtpContext::new(Timestamp { micros: 0 }),
        )
        .await
        {
            Ok(time) => {
                let unix = time.sec();
                SYNCED.store(true, Ordering::Relaxed);
                LAST_SYNC_UNIX.store(unix as u32, Ordering::Relaxed);
                LAST_SYNC_AT.store(now_secs() as u32, Ordering::Relaxed);
                info!("SNTP synchronized: unix={unix}");
            }
            Err(error) => error!("SNTP request failed: {error:?}"),
        }

        Timer::after(Duration::from_secs(SNTP_RESYNC_SECS)).await;
    }
}
