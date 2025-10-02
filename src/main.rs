mod queue;
mod soffice;
#[cfg(test)]
mod test;
use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use futures::StreamExt;
use queue::QueueProcessor;
use std::{env, sync::Arc};
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

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
        .route("/convertb64", post(convertb64_handler))
        .route("/convert_stream", post(convert_stream_handler))
        .with_state(queue_processor)
}

async fn health() -> &'static str {
    println!("/ health check");
    "running!"
}

async fn convertb64_handler(
    State(queue_processor): State<Arc<QueueProcessor>>,
    body: String,
) -> Result<String, AppError> {
    let result = queue_processor.process_base64(body).await?;
    Ok(result)
}

// --- Custom response type that holds both file reader and tempdir
struct TempFileResponse {
    _tmp_dir: TempDir, // ensures directory isn't deleted early
    body: Body,        // streaming response
}

impl IntoResponse for TempFileResponse {
    fn into_response(self) -> Response {
        (
            [(axum::http::header::CONTENT_TYPE, "application/pdf")],
            self.body,
        )
            .into_response()
    }
}

async fn convert_stream_handler(
    State(queue_processor): State<Arc<QueueProcessor>>,
    body: Body,
) -> Result<impl IntoResponse, AppError> {
    let mut stream = body.into_data_stream();
    let tmp_dir = TempDir::new()?;
    let tmp_docx_path = tmp_dir.path().join("tmp.docx");
    let tmp_pdf_path = tmp_dir.path().join("tmp.pdf");

    let mut docx_file = File::create(&tmp_docx_path).await?;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        docx_file.write_all(&chunk).await?;
    }

    queue_processor
        .process_file_path(
            tmp_docx_path.to_str().unwrap(),
            tmp_dir.path().to_str().unwrap(),
        )
        .await?;

    let pdf_file = File::open(&tmp_pdf_path).await?;
    let pdf_stream = ReaderStream::new(pdf_file);
    let body = Body::from_stream(pdf_stream);

    // Return wrapper that keeps TempDir alive until stream is dropped
    Ok(TempFileResponse {
        _tmp_dir: tmp_dir,
        body,
    })
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
