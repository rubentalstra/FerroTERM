use axum::body::Body;
use http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn health_answers_ok() -> Result<(), Box<dyn std::error::Error>> {
    let request = Request::get("/health").body(Body::empty())?;
    let response = notio_server::router().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn unknown_route_is_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let request = Request::get("/nope").body(Body::empty())?;
    let response = notio_server::router().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}
