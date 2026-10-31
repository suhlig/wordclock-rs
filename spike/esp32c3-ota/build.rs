use std::collections::HashMap;
use std::path::Path;

fn main() {
    linker_be_nice();
    // make sure linkall.x is the last linker script (otherwise might cause problems with flip-link)
    println!("cargo:rustc-link-arg=-Tlinkall.x");
    load_env();
}

/// Exports the Wi-Fi credentials and the optional static IP as compile-time
/// environment variables.
///
/// `wifi.env` is looked up in this crate first and then in `../esp32c3-wifi`,
/// so the credentials already used for the Wi-Fi spike work without a copy.
/// `ota.env` optionally pins the address; without it the firmware uses DHCP.
fn load_env() {
    println!("cargo:rerun-if-changed=wifi.env");
    println!("cargo:rerun-if-changed=../esp32c3-wifi/wifi.env");
    println!("cargo:rerun-if-changed=ota.env");

    let credentials = read_env(Path::new("wifi.env"))
        .or_else(|| read_env(Path::new("../esp32c3-wifi/wifi.env")))
        .unwrap_or_default();
    let ota = read_env(Path::new("ota.env")).unwrap_or_default();

    emit("WORDCLOCK_WIFI_SSID", &credentials, "WORDCLOCK_WIFI_SSID");
    emit(
        "WORDCLOCK_WIFI_PASSWORD",
        &credentials,
        "WORDCLOCK_WIFI_PASSWORD",
    );
    emit("WORDCLOCK_OTA_IP", &ota, "WORDCLOCK_OTA_IP");
    emit("WORDCLOCK_OTA_GATEWAY", &ota, "WORDCLOCK_OTA_GATEWAY");
}

fn emit(variable: &str, values: &HashMap<String, String>, key: &str) {
    let value = values.get(key).map(String::as_str).unwrap_or_default();
    println!("cargo:rustc-env={variable}={value}");
}

fn read_env(path: &Path) -> Option<HashMap<String, String>> {
    let contents = std::fs::read_to_string(path).ok()?;
    let mut values = HashMap::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    Some(values)
}

fn linker_be_nice() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let kind = &args[1];
        let what = &args[2];

        match kind.as_str() {
            "undefined-symbol" => match what.as_str() {
                what if what.starts_with("_defmt_") => {
                    eprintln!();
                    eprintln!(
                        "💡 `defmt` not found - make sure `defmt.x` is added as a linker script and you have included `use defmt_rtt as _;`"
                    );
                    eprintln!();
                }
                "_stack_start" => {
                    eprintln!();
                    eprintln!("💡 Is the linker script `linkall.x` missing?");
                    eprintln!();
                }
                what if what.starts_with("esp_rtos_") => {
                    eprintln!();
                    eprintln!(
                        "💡 `esp-radio` has no scheduler enabled. Make sure you have initialized `esp-rtos` or provided an external scheduler."
                    );
                    eprintln!();
                }
                "embedded_test_linker_file_not_added_to_rustflags" => {
                    eprintln!();
                    eprintln!(
                        "💡 `embedded-test` not found - make sure `embedded-test.x` is added as a linker script for tests"
                    );
                    eprintln!();
                }
                "free"
                | "malloc"
                | "calloc"
                | "get_free_internal_heap_size"
                | "malloc_internal"
                | "realloc_internal"
                | "calloc_internal"
                | "free_internal" => {
                    eprintln!();
                    eprintln!(
                        "💡 Did you forget the `esp-alloc` dependency or didn't enable the `compat` feature on it?"
                    );
                    eprintln!();
                }
                _ => (),
            },
            // we don't have anything helpful for "missing-lib" yet
            _ => {
                std::process::exit(1);
            }
        }

        std::process::exit(0);
    }

    println!(
        "cargo:rustc-link-arg=--error-handling-script={}",
        std::env::current_exe().unwrap().display()
    );
}
