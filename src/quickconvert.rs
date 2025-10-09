use crate::queue_handler;
use crate::queue_processor::Handler;
use anyhow::Result;
use std::process::{Command, Stdio};

pub struct QuickConvertRequest {
    pub input_path: String,
    pub output_path: String,
}

queue_handler!(
pub QuickQueueHandler,
    (request: QuickConvertRequest) -> Result<()> {
        request_handler(request).await
    }
);

async fn request_handler(request: QuickConvertRequest) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        convert_with_docx2pdf_rs(&request.input_path, &request.output_path)
    })
    .await?
}

fn convert_with_docx2pdf_rs(input_path: &str, output_path: &str) -> anyhow::Result<()> {
    let command = std::env::var("DOCX2PDF_RS_COMMAND").unwrap_or("docx2pdf_rs".to_string());
    let status = Command::new(command)
        .args(["-o", output_path, input_path])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    if !status.success() {
        anyhow::bail!("docx2pdf_rs exited with {}", status);
    }
    Ok(())
}
