//! CITADEL Central UI Rendering Engine
//!
//! Provides zero-internet self-contained interfaces:
//! 1. Candidate Portal: Resizable split-screen code assessment portal with Ace editor,
//!    canonical Two Sum problem, test execution with sample diffs, and locked/unlocked submission flow.
//! 2. Protected Recruiter LMS: Administrative management console with candidate monitoring,
//!    one-click disqualification, live metric sync, and question/test case configuration.
//! 3. Admin Access Denied: 403 Forbidden security screen for unauthorized access.
//! 4. Gatekeeper: Candidate download instructions.
//! 5. Mobile Blocked: Dedicated prompt informing candidates that a laptop is strictly required in production mode.

use std::sync::OnceLock;

fn rewrite_versioned_assets(raw: &str) -> String {
    let mut out = raw.to_string();
    for (name, asset) in crate::assets::registry().iter() {
        let unversioned = format!("/static/{}", name);
        let versioned = format!("/static/{}?v={}", name, asset.hash8);
        out = out.replace(&unversioned, &versioned);
    }
    out
}

pub fn render_portal_html() -> &'static str {
    static PORTAL: OnceLock<String> = OnceLock::new();
    PORTAL.get_or_init(|| {
        rewrite_versioned_assets(include_str!("../templates/portal.html"))
    })
}

pub fn render_recruiter_lms_html() -> &'static str {
    static RECRUITER: OnceLock<String> = OnceLock::new();
    RECRUITER.get_or_init(|| {
        rewrite_versioned_assets(include_str!("../templates/recruiter.html"))
    })
}

pub fn render_admin_denied_html() -> &'static str {
    static DENIED: OnceLock<String> = OnceLock::new();
    DENIED.get_or_init(|| {
        rewrite_versioned_assets(include_str!("../templates/denied.html"))
    })
}

pub fn render_mobile_blocked_html() -> &'static str {
    static MOBILE: OnceLock<String> = OnceLock::new();
    MOBILE.get_or_init(|| {
        rewrite_versioned_assets(include_str!("../templates/mobile_blocked.html"))
    })
}

pub fn render_gatekeeper_html(is_production: bool, is_mobile: bool) -> String {
    static GK_BASE: OnceLock<String> = OnceLock::new();
    let raw = GK_BASE.get_or_init(|| {
        rewrite_versioned_assets(include_str!("../templates/gatekeeper.html"))
    });

    let mut page = if is_production {
        let mut p = raw.replace(
            r#"<a href="/exam" class="direct-link">Launch Web Assessment Directly (Testing Mode) &rarr;</a>"#,
            r#"<div style="margin-bottom: 20px;"></div>"#,
        );
        if let (Some(s), Some(e)) = (
            p.find("<!-- {{TESTING_ONLY_RESTORE_START}} -->"),
            p.find("<!-- {{TESTING_ONLY_RESTORE_END}} -->"),
        ) {
            let end_tag = "<!-- {{TESTING_ONLY_RESTORE_END}} -->";
            p.replace_range(s..e + end_tag.len(), "");
        }
        if let (Some(s), Some(e)) = (
            p.find("<!-- {{TESTING_ONLY_RECOVERY_START}} -->"),
            p.find("<!-- {{TESTING_ONLY_RECOVERY_END}} -->"),
        ) {
            let end_tag = "<!-- {{TESTING_ONLY_RECOVERY_END}} -->";
            p.replace_range(s..e + end_tag.len(), "");
        }
        p
    } else {
        raw.clone()
    };

    page = page.replace(
        "/* {{IS_PRODUCTION_FLAG}} */ false",
        if is_production { "true" } else { "false" },
    );

    if is_production && is_mobile {
        page = page.replace(
            r#"id="mobile-warning-banner" class="mobile-warning" style="display: none;"#,
            r#"id="mobile-warning-banner" class="mobile-warning" style="display: block;"#,
        );
    }

    page
}
