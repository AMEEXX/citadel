//! CITADEL Guard Network Enforcement Engine
//! Native Windows Filtering Platform (WFP) Zero-Internet / College LAN Lockdown.
//!
//! Provides zero-driver, kernel-level traffic filtering:
//! - Permits outbound traffic ONLY to the designated CITADEL Exam Server (IP:Port) on the College LAN
//! - Permits local loopback for inter-process communication (Guard <-> Shell <-> Forge)
//! - Permits DHCP (UDP 67) so student laptops maintain their college Wi-Fi IP leases
//! - Drops ALL other outbound traffic (Internet, web browsing, cloud LLMs, Discord, proxies)
//! - Uses dynamic WFP sessions (`FWPM_SESSION_FLAG_DYNAMIC`) ensuring 100% crash safety:
//!   all filters are purged automatically by the Windows kernel upon process exit or shutdown.

use std::fmt;
use std::net::Ipv4Addr;
use std::ptr;
use windows::core::{GUID, PWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::*;
use windows::Win32::System::Rpc::RPC_C_AUTHN_WINNT;

/// Deterministic GUID for the CITADEL WFP sublayer
/// {b63a9ec6-8d19-4a0b-9dfa-8a4a1f592cf0}
pub const CITADEL_SUBLAYER_GUID: GUID = GUID::from_u128(0xb63a9ec6_8d19_4a0b_9dfa_8a4a1f592cf0);

#[derive(Debug)]
pub enum WfpError {
    EngineOpenFailed(u32),
    SubLayerAddFailed(u32),
    FilterAddFailed(&'static str, u32),
    EngineCloseFailed(u32),
}

impl fmt::Display for WfpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WfpError::EngineOpenFailed(code) => {
                write!(f, "Failed to open WFP engine (Win32 error: 0x{:08X})", code)
            }
            WfpError::SubLayerAddFailed(code) => {
                write!(f, "Failed to add CITADEL sublayer (Win32 error: 0x{:08X})", code)
            }
            WfpError::FilterAddFailed(name, code) => {
                write!(f, "Failed to add WFP filter '{}' (Win32 error: 0x{:08X})", name, code)
            }
            WfpError::EngineCloseFailed(code) => {
                write!(f, "Failed to close WFP engine (Win32 error: 0x{:08X})", code)
            }
        }
    }
}

impl std::error::Error for WfpError {}

/// Handle to an active WFP filtering session
pub struct WfpEngine {
    handle: HANDLE,
    sublayer_key: GUID,
    filter_ids: Vec<u64>,
}

// Safety: WFP handle can be transferred between threads in the service context
unsafe impl Send for WfpEngine {}
unsafe impl Sync for WfpEngine {}

impl WfpEngine {
    /// Opens a dynamic WFP engine session.
    ///
    /// With `FWPM_SESSION_FLAG_DYNAMIC`, any filters and sublayers added through
    /// this session are automatically removed by Windows kernel if the service stops
    /// or terminates unexpectedly.
    pub fn open_dynamic() -> Result<Self, WfpError> {
        let mut session = FWPM_SESSION0::default();
        session.flags = FWPM_SESSION_FLAG_DYNAMIC;

        let mut handle = HANDLE::default();
        let status = unsafe {
            FwpmEngineOpen0(
                None,               // local computer
                RPC_C_AUTHN_WINNT,  // 10: NTLM auth for local BFE
                None,
                Some(&session),
                &mut handle,
            )
        };

        if status != 0 {
            return Err(WfpError::EngineOpenFailed(status));
        }

        Ok(Self {
            handle,
            sublayer_key: CITADEL_SUBLAYER_GUID,
            filter_ids: Vec::new(),
        })
    }

    /// Installs the College LAN Zero-Internet isolation policy.
    ///
    /// - Allows: `server_ip:server_port`
    /// - Allows: Loopback (127.0.0.1/8) for local IPC
    /// - Allows: DHCP (UDP 67) for Wi-Fi address maintenance
    /// - Blocks: All other outbound connections (Default Deny)
    pub fn install_college_lan_policy(
        &mut self,
        server_ip: Ipv4Addr,
        server_port: u16,
    ) -> Result<(), WfpError> {
        self.add_sublayer()?;

        // Rule 1: Allow Exam Server IP:Port (Weight = 100)
        self.add_server_permit_filter(server_ip, server_port, 100)?;

        // Rule 2: Allow DHCP Client to Server (UDP 67) (Weight = 90)
        self.add_dhcp_permit_filter(90)?;

        // Rule 3: Allow Local Loopback (Weight = 80)
        self.add_loopback_permit_filter(80)?;

        // Rule 4: Default Deny / Block All Remaining Outbound IPv4 (Weight = 10)
        self.add_default_deny_filter(10)?;

        // Rule 5: Default Deny / Block All Remaining Outbound IPv6 (Weight = 10)
        self.add_default_deny_v6_filter(10)?;

        Ok(())
    }

    fn add_sublayer(&mut self) -> Result<(), WfpError> {
        let mut name_wide: Vec<u16> = "CITADEL Exam Sublayer\0".encode_utf16().collect();
        let mut sublayer = FWPM_SUBLAYER0::default();
        sublayer.subLayerKey = self.sublayer_key;
        sublayer.weight = 0x8000; // High priority sublayer
        sublayer.displayData.name = PWSTR(name_wide.as_mut_ptr());

        let status = unsafe {
            FwpmSubLayerAdd0(self.handle, &sublayer, None)
        };

        // FWP_E_ALREADY_EXISTS = 0x8032000C
        if status != 0 && status != 0x8032000C {
            return Err(WfpError::SubLayerAddFailed(status));
        }

        Ok(())
    }

    fn add_server_permit_filter(
        &mut self,
        server_ip: Ipv4Addr,
        server_port: u16,
        weight: u64,
    ) -> Result<(), WfpError> {
        let mut cond_ip = FWPM_FILTER_CONDITION0::default();
        cond_ip.fieldKey = FWPM_CONDITION_IP_REMOTE_ADDRESS;
        cond_ip.matchType = FWP_MATCH_EQUAL;
        cond_ip.conditionValue.r#type = FWP_UINT32;
        cond_ip.conditionValue.Anonymous.uint32 = u32::from(server_ip);

        let mut cond_port = FWPM_FILTER_CONDITION0::default();
        cond_port.fieldKey = FWPM_CONDITION_IP_REMOTE_PORT;
        cond_port.matchType = FWP_MATCH_EQUAL;
        cond_port.conditionValue.r#type = FWP_UINT16;
        cond_port.conditionValue.Anonymous.uint16 = server_port;

        let mut conditions = [cond_ip, cond_port];

        let mut weight_val = weight;
        let mut name_wide: Vec<u16> = "CITADEL Permit Exam Server\0".encode_utf16().collect();
        let mut filter = FWPM_FILTER0::default();
        filter.layerKey = FWPM_LAYER_ALE_AUTH_CONNECT_V4;
        filter.action.r#type = FWP_ACTION_PERMIT;
        filter.subLayerKey = self.sublayer_key;
        filter.weight.r#type = FWP_UINT64;
        filter.weight.Anonymous.uint64 = &mut weight_val;
        filter.displayData.name = PWSTR(name_wide.as_mut_ptr());
        filter.filterCondition = conditions.as_mut_ptr();
        filter.numFilterConditions = conditions.len() as u32;

        let mut filter_id = 0u64;
        let status = unsafe {
            FwpmFilterAdd0(self.handle, &filter, None, Some(&mut filter_id))
        };

        if status != 0 {
            return Err(WfpError::FilterAddFailed("PermitExamServer", status));
        }

        self.filter_ids.push(filter_id);
        Ok(())
    }

    fn add_dhcp_permit_filter(&mut self, weight: u64) -> Result<(), WfpError> {
        let mut cond_proto = FWPM_FILTER_CONDITION0::default();
        cond_proto.fieldKey = FWPM_CONDITION_IP_PROTOCOL;
        cond_proto.matchType = FWP_MATCH_EQUAL;
        cond_proto.conditionValue.r#type = FWP_UINT8;
        cond_proto.conditionValue.Anonymous.uint8 = 17; // UDP

        let mut cond_port = FWPM_FILTER_CONDITION0::default();
        cond_port.fieldKey = FWPM_CONDITION_IP_REMOTE_PORT;
        cond_port.matchType = FWP_MATCH_EQUAL;
        cond_port.conditionValue.r#type = FWP_UINT16;
        cond_port.conditionValue.Anonymous.uint16 = 67; // DHCP server port

        let mut conditions = [cond_proto, cond_port];

        let mut weight_val = weight;
        let mut name_wide: Vec<u16> = "CITADEL Permit DHCP\0".encode_utf16().collect();
        let mut filter = FWPM_FILTER0::default();
        filter.layerKey = FWPM_LAYER_ALE_AUTH_CONNECT_V4;
        filter.action.r#type = FWP_ACTION_PERMIT;
        filter.subLayerKey = self.sublayer_key;
        filter.weight.r#type = FWP_UINT64;
        filter.weight.Anonymous.uint64 = &mut weight_val;
        filter.displayData.name = PWSTR(name_wide.as_mut_ptr());
        filter.filterCondition = conditions.as_mut_ptr();
        filter.numFilterConditions = conditions.len() as u32;

        let mut filter_id = 0u64;
        let status = unsafe {
            FwpmFilterAdd0(self.handle, &filter, None, Some(&mut filter_id))
        };

        if status != 0 {
            return Err(WfpError::FilterAddFailed("PermitDHCP", status));
        }

        self.filter_ids.push(filter_id);
        Ok(())
    }

    fn add_loopback_permit_filter(&mut self, weight: u64) -> Result<(), WfpError> {
        let mut cond_loopback = FWPM_FILTER_CONDITION0::default();
        cond_loopback.fieldKey = FWPM_CONDITION_FLAGS;
        cond_loopback.matchType = FWP_MATCH_FLAGS_ALL_SET;
        cond_loopback.conditionValue.r#type = FWP_UINT32;
        cond_loopback.conditionValue.Anonymous.uint32 = FWP_CONDITION_FLAG_IS_LOOPBACK;

        let mut conditions = [cond_loopback];

        let mut weight_val = weight;
        let mut name_wide: Vec<u16> = "CITADEL Permit Loopback\0".encode_utf16().collect();
        let mut filter = FWPM_FILTER0::default();
        filter.layerKey = FWPM_LAYER_ALE_AUTH_CONNECT_V4;
        filter.action.r#type = FWP_ACTION_PERMIT;
        filter.subLayerKey = self.sublayer_key;
        filter.weight.r#type = FWP_UINT64;
        filter.weight.Anonymous.uint64 = &mut weight_val;
        filter.displayData.name = PWSTR(name_wide.as_mut_ptr());
        filter.filterCondition = conditions.as_mut_ptr();
        filter.numFilterConditions = conditions.len() as u32;

        let mut filter_id = 0u64;
        let status = unsafe {
            FwpmFilterAdd0(self.handle, &filter, None, Some(&mut filter_id))
        };

        if status != 0 {
            return Err(WfpError::FilterAddFailed("PermitLoopback", status));
        }

        self.filter_ids.push(filter_id);
        Ok(())
    }

    fn add_default_deny_filter(&mut self, weight: u64) -> Result<(), WfpError> {
        let mut weight_val = weight;
        let mut name_wide: Vec<u16> = "CITADEL Default Deny Outbound\0".encode_utf16().collect();
        let mut filter = FWPM_FILTER0::default();
        filter.layerKey = FWPM_LAYER_ALE_AUTH_CONNECT_V4;
        filter.action.r#type = FWP_ACTION_BLOCK;
        filter.subLayerKey = self.sublayer_key;
        filter.weight.r#type = FWP_UINT64;
        filter.weight.Anonymous.uint64 = &mut weight_val;
        filter.displayData.name = PWSTR(name_wide.as_mut_ptr());
        filter.filterCondition = ptr::null_mut();
        filter.numFilterConditions = 0; // Matches all remaining connections in this layer

        let mut filter_id = 0u64;
        let status = unsafe {
            FwpmFilterAdd0(self.handle, &filter, None, Some(&mut filter_id))
        };

        if status != 0 {
            return Err(WfpError::FilterAddFailed("DefaultBlockOutbound", status));
        }

        self.filter_ids.push(filter_id);
        Ok(())
    }

    fn add_default_deny_v6_filter(&mut self, weight: u64) -> Result<(), WfpError> {
        let mut weight_val = weight;
        let mut name_wide: Vec<u16> = "CITADEL Default Deny IPv6 Outbound\0".encode_utf16().collect();
        let mut filter = FWPM_FILTER0::default();
        filter.layerKey = FWPM_LAYER_ALE_AUTH_CONNECT_V6;
        filter.action.r#type = FWP_ACTION_BLOCK;
        filter.subLayerKey = self.sublayer_key;
        filter.weight.r#type = FWP_UINT64;
        filter.weight.Anonymous.uint64 = &mut weight_val;
        filter.displayData.name = PWSTR(name_wide.as_mut_ptr());
        filter.filterCondition = ptr::null_mut();
        filter.numFilterConditions = 0; // Matches all remaining IPv6 connections

        let mut filter_id = 0u64;
        let status = unsafe {
            FwpmFilterAdd0(self.handle, &filter, None, Some(&mut filter_id))
        };

        if status != 0 {
            return Err(WfpError::FilterAddFailed("DefaultBlockOutboundV6", status));
        }

        self.filter_ids.push(filter_id);
        Ok(())
    }

    /// Explicitly closes the WFP engine session.
    pub fn close(mut self) -> Result<(), WfpError> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<(), WfpError> {
        if !self.handle.is_invalid() {
            let status = unsafe { FwpmEngineClose0(self.handle) };
            self.handle = HANDLE::default();
            if status != 0 {
                return Err(WfpError::EngineCloseFailed(status));
            }
        }
        Ok(())
    }
}

impl Drop for WfpEngine {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
