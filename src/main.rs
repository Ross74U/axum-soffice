mod queue;
mod soffice;
#[cfg(test)]
mod test;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use queue::QueueProcessor;
use std::{env, sync::Arc};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let mut port = String::from("8000");
    let mut addr = String::from("0.0.0.0");
    if args.len() > 1 {
        // read bind address and port from arguments
        if args.len() % 2 == 0 {
            panic!("There should be an even number of arguments.");
        }
        for arg_num in 1..args.len() {
            if arg_num % 2 == 0 {
                continue;
            } else if args[arg_num] == "--port" {
                port = args[arg_num + 1].clone();
            } else if args[arg_num] == "--addr" {
                addr = args[arg_num + 1].clone();
            } else {
                panic!("Unknown argument {}", args[arg_num]);
            }
        }
    }
    let app = create_app(5);
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", addr, port))
        .await
        .unwrap();
    println!("Axum server running on http://{}:{}", addr, port);
    axum::serve(listener, app).await.unwrap();
}

fn create_app(num_workers: usize) -> Router {
    let queue_processor = Arc::new(QueueProcessor::new(num_workers).unwrap());
    Router::new()
        .route("/", get(health))
        .route("/convertb64", post(convertb64))
        .with_state(queue_processor)
}

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
