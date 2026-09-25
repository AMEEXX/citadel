use citadel_client::find_browser_executable;

#[test]
fn test_browser_executable_found() {
    let browser = find_browser_executable();
    assert!(
        browser.is_some(),
        "Expected Microsoft Edge or Google Chrome to be detected on the Windows test machine"
    );
    let path = browser.unwrap();
    assert!(path.exists(), "Browser path {:?} must exist on disk", path);
}
