//! CITADEL Central UI Rendering Engine
//!
//! Provides zero-internet self-contained interfaces:
//! 1. Candidate Portal: Resizable split-screen code assessment portal with Ace editor,
//!    canonical Two Sum problem, test execution with sample diffs, and locked/unlocked submission flow.
//! 2. Protected Recruiter LMS: Administrative management console with candidate monitoring,
//!    one-click disqualification, live metric sync, and question/test case configuration.
//! 3. Admin Access Denied: 403 Forbidden security screen for unauthorized access.
//! 4. Gatekeeper: Candidate download instructions.

pub fn render_portal_html() -> &'static str {
    include_str!("../templates/portal.html")
}

pub fn render_recruiter_lms_html() -> &'static str {
    include_str!("../templates/recruiter.html")
}

pub fn render_admin_denied_html() -> &'static str {
    include_str!("../templates/denied.html")
}

pub fn render_gatekeeper_html() -> &'static str {
    include_str!("../templates/gatekeeper.html")
}
