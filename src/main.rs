mod queue_processor; // trait for any queue-based service with channels
mod soffice;
mod stream_handler;
#[cfg(test)]
mod test;
mod utils;

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use queue_processor::QueueProcessor;
use soffice::{SofficeQueueHandler, SofficeRequest, SofficeResponse};
use std::{env, sync::Arc};
use stream_handler::convert_stream_handler;

#[derive(Clone)]
struct AppState {
    soffice_queue: Arc<QueueProcessor<SofficeRequest, Result<SofficeResponse>>>,
    // todo impl fast llm queue
}

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

fn create_app(soffice_workers: usize) -> Router {
    let soffice_handler = SofficeQueueHandler::new();
    let soffice_queue = Arc::new(QueueProcessor::new(soffice_workers, soffice_handler));
    let app_state = AppState { soffice_queue };
    Router::new()
        .route("/", get(health))
        .route("/convertb64", post(convertb64_handler))
        .route("/convert_stream", post(convert_stream_handler))
        .with_state(app_state)
}

async fn health() -> &'static str {
    println!("/ health check");
    "running!"
}

async fn convertb64_handler(
    State(app_state): State<AppState>,
    body: String,
) -> Result<String, AppError> {
    let request = SofficeRequest::Base64String(body);
    let SofficeResponse::Base64String(result) =
        app_state.soffice_queue.process_in_queue(request).await?? 
    else {
        return Err(anyhow::anyhow!("what, impossible response variant").into());
    };

    Ok(result)
}

struct AppError(anyhow::Error);
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
