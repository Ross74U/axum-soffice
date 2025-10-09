use crate::quickconvert::QuickConvertRequest;
use crate::soffice::{FilePathInput, SofficeRequest, SofficeResponse};
use crate::{AppError, AppState};
use axum::{
    body::Body,
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use futures::StreamExt;
use serde::Deserialize;
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

// --- Custom response type that holds both file reader and tempdir
// to ensure the tempdir is not dropped until the response has been complete
//
struct TempFileResponse {
    _tmp_dir: TempDir,
    body: Body,
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

#[derive(Deserialize)]
pub struct ConversionQueries {
    llm_audience: Option<bool>,
}

pub async fn convert_stream_handler(
    queries: Query<ConversionQueries>,
    State(app_state): State<AppState>,
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

    if queries.llm_audience.is_some_and(|x| x) {
        // use quick convert queue for llm audience
        let req = QuickConvertRequest {
            input_path: tmp_docx_path.to_str().unwrap().to_string(),
            output_path: tmp_pdf_path.to_str().unwrap().to_string(),
        };
        app_state.quick_queue.process_in_queue(req).await??;
    } else {
        // regular soffice queue
        let file_path_input = FilePathInput {
            docx: tmp_docx_path.to_str().unwrap().to_string(),
            dir: tmp_dir.path().to_str().unwrap().to_string(),
        };
        let req = SofficeRequest::FilePathInput(file_path_input);
        let SofficeResponse::FilePathConverted = app_state
            .soffice_queue
            .process_in_queue(req).await??
        else {
            return Err(anyhow::anyhow!("impossible response variant").into());
        };
    }

    let pdf_file = File::open(&tmp_pdf_path).await?;
    let pdf_stream = ReaderStream::new(pdf_file);
    let body = Body::from_stream(pdf_stream);

    // Return wrapper that keeps TempDir alive until stream is dropped
    Ok(TempFileResponse {
        _tmp_dir: tmp_dir,
        body,
    })
}
