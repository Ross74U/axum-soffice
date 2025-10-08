// soffice conversion queue for HUMAN consumption
//
use crate::queue_handler;
use crate::queue_processor::Handler;
use crate::utils;
use std::{process::Command, process::Stdio};
use tempfile::TempDir;

pub enum SofficeRequest {
    Base64String(String),
    FilePathInput(FilePathInput),
}

pub struct FilePathInput {
    pub docx: String,
    pub dir: String,
}

pub enum SofficeResponse {
    Base64String(String),
    FilePathConverted,
}

queue_handler!(
pub SofficeQueueHandler,
    (request: SofficeRequest) -> anyhow::Result<SofficeResponse> {
        request_handler(request).await
    }
);

async fn request_handler(request: SofficeRequest) -> anyhow::Result<SofficeResponse> {
    match request {
        SofficeRequest::Base64String(docx_base64) => match convert_base64_pdf(&docx_base64).await {
            Ok(pdf_string) => Ok(SofficeResponse::Base64String(pdf_string)),
            Err(e) => Err(e),
        },
        SofficeRequest::FilePathInput(file_path_input) => {
            match convert_file_path(&file_path_input.docx, &file_path_input.dir).await {
                Ok(_) => Ok(SofficeResponse::FilePathConverted),
                Err(e) => Err(e),
            }
        }
    }
}

pub async fn convert_file_path(docx_path: &str, pdf_path: &str) -> anyhow::Result<()> {
    let docx_path = String::from(docx_path);
    let dir_path = String::from(pdf_path);
    tokio::task::spawn_blocking(move || convert_with_libreoffice(&docx_path, &dir_path)).await?
}

pub async fn convert_base64_pdf(docx_base64: &str) -> anyhow::Result<String> {
    let tmp_dir = TempDir::new()?;
    let tmp_docx_path = format!("{}/tmp.docx", tmp_dir.path().display());
    let tmp_pdf_path = format!("{}/tmp.pdf", tmp_dir.path().display());
    let tmp_dir_path = format!("{}", tmp_dir.path().display());

    utils::base64_to_file(docx_base64, &tmp_docx_path).await?;
    let _ = tokio::task::spawn_blocking(move || {
        convert_with_libreoffice(&tmp_docx_path, &tmp_dir_path)
    })
    .await?;
    let output_base64: String = utils::file_to_base64(&tmp_pdf_path).await?;
    Ok(output_base64)
}

fn convert_with_libreoffice(input: &str, output_dir: &str) -> anyhow::Result<()> {
    // 1. per-process profile
    let tmp_profile = TempDir::new()?;
    let profile_uri = format!("file://{}", tmp_profile.path().display());

    // 2. spawn soffice
    let status = Command::new("soffice")
        .args([
            "--headless",
            "--nologo",
            "--nodefault",
            "--nolockcheck",
            "--norestore",
            &format!("-env:UserInstallation={}", profile_uri),
            "--convert-to",
            "pdf",
            "--outdir",
            output_dir,
            input,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    if !status.success() {
        anyhow::bail!("LibreOffice exited with {}", status);
    }
    Ok(())
}
