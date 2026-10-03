# 📦 CITADEL — Pre-Compiled Distribution Binaries

This directory houses pre-compiled, statically linked 64-bit Windows (`x86_64-pc-windows-msvc`) release binaries. These binaries allow operators, test invigilators, and evaluators to run the Citadel platform without requiring a local Rust toolchain or build environment.

---

H§ Binary Inventory

| Binary | Source Crate | Role & Technical Summary |
|---|---|---|
| [`citadel-server.exe`](citadel-server.exe) | [`citadel-server/`](../citadel-server/) | **Air-Gapped Venue Exam Appliance**: High-performance Axum 0.7 REST engine, Server-Sent Events (SSR) live proctoring bus, SQLite & JSON state persistence engine, embedded web templates, and offline Geist / Geist Mono fonts. |
| [`citadel-client.exe`](citadel-client.exe) | [`citadel-client/`](../citadel-client/) | **Candidate Lockdown Kiosk**: Native Win32 Per-Monitor v2 DPI shell, `WDS_KEYLOCP_LL` low-level keyboard hook (blocking Alt+Tab, Windows Keys, Task Manager, DevTools), and isolated Chromium kiosk runner. |
| [`citadel-recovery.exe`](citadel-recovery.exe) | [`citadel-client/src/bin/recovery.rs`](../citadel-client/src/bin/recovery.rs) | **Emergency Failsafe Recovery**: Standalone desktop recovery utility that unlocks candidate workstations, restarts Windows Explorer (`explorer.exe`), removes restrictive registry keys, and re-enables system shortcuts if an abrupt crash occurs during testing. |

---

H§ Building From Source

All binaries in this directory are compiled from the root Cargo workspace using static C runtime linking (`crt-static`), symbol stripping, and size optimization:

```powershell
# From the repository root, compile all targets for Windows x64 release
cargo build --release --target x86_64-pc-windows-msvc

# Compiled binaries will be located at:
# target/x86_64-pc-windows-msvc/release/citadel-server.exe
# target/x86_64-pc-windows-msvc/release/citadel-client.exe
# target/x86_64-pc-windows-msvc/release/recovery.exe
```

> **Compilation Flags**: Static CRT linkage (`-C target-feature=+crt-static`) is configured in [`.cargo/config.toml`](../.cargo/config.toml) to eliminate runtime MSVC DLL dependencies on candidate BYOD machines.
