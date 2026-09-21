pub use windivert::{WinDivert, WinDivertError, WinDivertFlags, WinDivertLayer};

pub type WinDivertHandle = WinDivert;

pub struct FilterConfig {
    pub filter: String,
    pub layer: WinDivertLayer,
    pub priority: i16,
    pub flags: WinDivertFlags,
}

pub fn default_deny_config() -> FilterConfig {
    FilterConfig {
        filter: "true".to_string(),
        layer: WinDivertLayer::Network,
        priority: 0,
        flags: WinDivertFlags::new(),
    }
}

pub fn install_default_deny() -> Result<WinDivertHandle, WinDivertError> {
    let cfg = default_deny_config();
    WinDivert::new(cfg.filter, cfg.layer, cfg.priority, cfg.flags)
}
