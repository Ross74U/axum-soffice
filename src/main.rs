mod queue;
mod soffice;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use queue::QueueProcessor;

async fn health() -> &'static str {
    "running!"
}

async fn convertb64(
    State(queue_processor): State<Arc<QueueProcessor>>,
    body: String,
) -> Result<String, AppError> {
    let result = queue_processor.process_base64(body).await?;
    Ok(result)
}

#[tokio::main]
async fn main() {
    let queue_processor = Arc::new(QueueProcessor::new(5).unwrap());

    let app = Router::new()
        .route("/", get(health))
        .route("/convertb64", post(convertb64))
        .with_state(queue_processor);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    println!("Axum server running on http://localhost:8000");
    axum::serve(listener, app).await.unwrap();
}

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}
// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
