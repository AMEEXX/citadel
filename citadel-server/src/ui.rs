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

pub fn render_portal_html() -> &'static str {
    include_str!("../templates/portal.html")
}

pub fn render_recruiter_lms_html() -> &'static str {
    include_str!("../templates/recruiter.html")
}

pub fn render_admin_denied_html() -> &'static str {
    include_str!("../templates/denied.html")
}

pub fn render_mobile_blocked_html() -> &'static str {
    include_str!("../templates/mobile_blocked.html")
}

pub fn render_gatekeeper_html(is_production: bool, is_mobile: bool) -> String {
    let raw = include_str!("../templates/gatekeeper.html");
    let mut page = if is_production {
        // In Production Mode, remove the testing-mode bypass link entirely
        raw.replace(
            r#"<a href="/exam" class="direct-link">Launch Web Assessment Directly (Testing Mode) &rarr;</a>"#,
            r#"<div style="margin-bottom: 20px;"></div>"#,
        )
    } else {
        raw.to_string()
    };

    // Inject is_production flag into client-side script
    page = page.replace(
        "/* {{IS_PRODUCTION_FLAG}} */ false",
        if is_production { "true" } else { "false" },
    );

    if is_production && is_mobile {
        // Make mobile warning banner visible on server render
        page = page.replace(
            r#"id="mobile-warning-banner" class="mobile-warning" style="display: none;"#,
            r#"id="mobile-warning-banner" class="mobile-warning" style="display: block;"#,
        );
    }

    page
}
