use guard_net::{default_deny_config, WinDivertLayer};

#[test]
fn test_default_deny_filter_and_layer() {
    let config = default_deny_config();
    assert_eq!(config.filter, "true");
    assert!(matches!(config.layer, WinDivertLayer::Network));
}
