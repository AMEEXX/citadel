//! CITADEL Emergency Recovery Utility (Restoration Supervisor Standalone)
//!
//! Authoritative, standalone executable that releases all locks, sweeps policies,
//! restores Task Manager and services, relaunches Explorer, verifies process termination 3x,
//! and hosts a status handoff loop on loopback.

fn main() {
    citadel_client::run_supervisor();
}
