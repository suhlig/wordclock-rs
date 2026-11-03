# Runtime metrics

This is the contract for the clock's runtime flash, RAM, and CPU metrics. The web page, the plain-text status document, the SSE `health` event, the future MQTT health document, and the future Prometheus endpoint must all use the names, units, and meanings below. A new transport must not rename or rescale an existing metric.

Status: specified by the `add-web-clock-face` change (`resource-metrics` capability). The implementation is planned there; this document is the contract it must satisfy.

## Measurement

**Flash.** The firmware parses the ESP-IDF image header and segment table of the running app partition to get the image length, and reads the partition size from the partition table. The figures describe the running app slot, not free space on the whole flash chip. The value is sampled once at boot because it cannot change without a reboot.

**RAM.** Three components are reported separately, because a single total would double-count the heap's backing memory:

- Heap used, maximum, and total from the allocator.
- Static `.data` plus `.bss` from the linker symbols, including the heap's backing arrays.
- Main-stack high-water and total from the linker region. The high-water is approximate: the unused stack is painted at boot and scanned at most once per 60 seconds.

**CPU.** Busy time is `1 - idle / wall` over the last 60 seconds. Idle microseconds are accumulated in the RTOS idle hook around the wait-for-interrupt instruction. Task and interrupt time count as busy; time waiting for an interrupt counts as idle. This is total CPU busy time, not application CPU time, and it is not a cycle-exact profile.

## Cadence

- Flash: once at boot.
- Heap, stack, and CPU: every 60 seconds.
- SSE: a `health` event immediately on connect and at least every 60 seconds.
- `/status`: on demand, from the same snapshot.
- The 60-second window is the only averaging interval. Ratios use one decimal place.

## Metrics

| Name | Unit | Meaning | Source |
| :--- | :--- | :--- | :--- |
| `flash_app_used_bytes` | bytes | Length of the running app image | ESP-IDF image header and segment table |
| `flash_app_total_bytes` | bytes | Size of the running app partition | Partition table |
| `flash_app_free_bytes` | bytes | `flash_app_total_bytes - flash_app_used_bytes` | Derived |
| `flash_app_used_pct` | percent | `flash_app_used_bytes / flash_app_total_bytes * 100` | Derived |
| `ram_heap_used_bytes` | bytes | Current heap usage | `esp_alloc::HEAP.stats()` |
| `ram_heap_max_bytes` | bytes | Peak heap usage since boot | `esp_alloc::HEAP.stats()` |
| `ram_heap_total_bytes` | bytes | Total heap regions | `esp_alloc::HEAP.stats()` |
| `ram_heap_used_pct` | percent | `ram_heap_used_bytes / ram_heap_total_bytes * 100` | Derived |
| `ram_static_bytes` | bytes | `.data` plus `.bss` | Linker symbols |
| `ram_stack_used_bytes` | bytes | Approximate main-stack high-water | Painted stack scan |
| `ram_stack_total_bytes` | bytes | Main-stack region | Linker symbols |
| `ram_stack_used_pct` | percent | `ram_stack_used_bytes / ram_stack_total_bytes * 100` | Derived |
| `cpu_busy_pct` | percent | Busy time over the last 60 seconds | Idle-hook accounting |
| `cpu_busy_max_pct` | percent | Highest 60-second value since boot | Derived |
| `cpu_freq_mhz` | MHz | Core clock | Build configuration |
| `uptime_s` | seconds | Seconds since boot | Monotonic clock |

## Payloads

The SSE `health` event carries one JSON object:

```text
event: health
data: {"uptime_s":3600,"flash_app_used_bytes":615808,"flash_app_total_bytes":1966080,"flash_app_free_bytes":1350272,"flash_app_used_pct":31.3,"ram_heap_used_bytes":54312,"ram_heap_max_bytes":54432,"ram_heap_total_bytes":103184,"ram_heap_used_pct":52.7,"ram_static_bytes":132464,"ram_stack_used_bytes":6120,"ram_stack_total_bytes":188048,"ram_stack_used_pct":3.3,"cpu_busy_pct":1.4,"cpu_busy_max_pct":12.0,"cpu_freq_mhz":160}

```

The plain-text status uses one `key=value` line per metric, with the same names and units:

```text
flash_app_used_bytes=615808
flash_app_total_bytes=1966080
flash_app_free_bytes=1350272
flash_app_used_pct=31.3
ram_heap_used_bytes=54312
ram_heap_max_bytes=54432
ram_heap_total_bytes=103184
ram_heap_used_pct=52.7
ram_static_bytes=132464
ram_stack_used_bytes=6120
ram_stack_total_bytes=188048
ram_stack_used_pct=3.3
cpu_busy_pct=1.4
cpu_busy_max_pct=12.0
cpu_freq_mhz=160
uptime_s=3600
```

The future MQTT health document carries the same JSON keys and units, retained on its own topic. Topic naming belongs to the MQTT change, and these metrics must not be added to the shown-state document described in design D22 of `replace-fadecandy-with-rust`.

The future Prometheus endpoint exposes every name as a gauge with the same unit and a `wordclock_` prefix, for example `wordclock_flash_app_used_bytes`, `wordclock_ram_heap_used_bytes`, and `wordclock_cpu_busy_pct`.

## Rules

- Bytes are integers. Percentages are 0–100 with one decimal place. `uptime_s`, `cpu_freq_mhz`, and the byte fields are integers.
- A value that cannot be measured is reported as `unknown` in the plain-text status and `null` in JSON, never as zero.
- The page may render human-friendly labels such as "Heap 54 / 103 KiB", but the underlying health JSON, the status keys, the MQTT document, and the Prometheus names use the metric names in the table above.
- The heap, static RAM, and stack figures are separate components. Do not add them into a single "RAM used" figure, because the heap's backing arrays are part of `.bss`.
- The stack high-water is approximate and is labelled as such wherever it is displayed.
- The host preview server marks firmware-only metrics as not applicable; it must not invent values.
- Measurement overhead stays under 1% average CPU and under 1 KiB of heap, and the health payload stays under 1 KiB.
