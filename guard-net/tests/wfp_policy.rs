use guard_net::{CITADEL_SUBLAYER_GUID, WfpError};

#[test]
fn test_citadel_sublayer_guid_format() {
    let guid_str = format!("{:?}", CITADEL_SUBLAYER_GUID);
    assert!(!guid_str.is_empty());
}

#[test]
fn test_wfp_error_display() {
    let err = WfpError::EngineOpenFailed(0x80320001);
    assert!(err.to_string().contains("Failed to open WFP engine"));

    let err2 = WfpError::FilterAddFailed("PermitExamServer", 0x8032000C);
    assert!(err2.to_string().contains("PermitExamServer"));
}
