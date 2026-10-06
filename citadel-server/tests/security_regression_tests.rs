use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use citadel_server::build_app;

#[tokio::test]
async fn traversal_encoded_dotdot_is_404() {
    let app = build_app();

    let paths = [
        "/static/..%2Fsrc%2Fquestions.rs",
        "/static/..%2F..%2F.git%2Fconfig",
        "/static/..%2F..%2Fopencode.json",
        "/static/fonts%2F..%2F..%2FCargo.toml",
    ];

    for path in paths {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "Path {} should be 404", path);
    }
}

#[tokio::test]
async fn backslash_and_absolute_are_404() {
    let app = build_app();

    let paths = [
        "/static/..%5Csrc%5Capi.rs",
        "/static/%2Fetc%2Fpasswd",
        "/static/C:%5Cwindows%5Cwin.ini",
    ];

    for path in paths {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "Path {} should be 404", path);
    }
}

#[tokio::test]
async fn hidden_cases_never_in_any_static_response() {
    let app = build_app();

    let fuzz_paths = [
        "/static/..%2Fsrc%2Fquestions.rs",
        "/static/..%2Fsrc%2Fapi.rs",
        "/static/..%2Fsrc%2Fjudge.rs",
        "/static/..%2F..%2Fcitadel-server%2Fsrc%2Fquestions.rs",
        "/static/..%2f..%2fquestions.rs",
        "/static/..%2Ftemplates%2Fportal.html",
        "/static/fonts%2F..%2Fquestions.rs",
        "/static/questions.rs",
    ];

    for path in fuzz_paths {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8_lossy(&body_bytes);
        assert!(!body_str.contains("hidden_cases"), "Path {} leaked hidden_cases!", path);
    }
}

#[tokio::test]
async fn legit_assets_still_200() {
    let app = build_app();

    let legit = [
        "/static/citadel-skin.css",
        "/static/fonts/Geist-Variable.woff2",
        "/static/citadel-restore.js",
        "/static/favicon-32x32.png",
    ];

    for path in legit {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "Legit path {} should be 200 OK", path);
    }
}
