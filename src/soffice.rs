use base64::{engine::general_purpose, Engine as _};
use std::{process::Command, process::Stdio};
use tempfile::TempDir;
use tokio::*;

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

pub async fn convert_base64_pdf(docx_base64: &str) -> anyhow::Result<String> {
    let tmp_dir = TempDir::new()?;
    let tmp_docx_path = format!("{}/tmp.docx", tmp_dir.path().display());
    let tmp_pdf_path = format!("{}/tmp.docx", tmp_dir.path().display());
    let tmp_dir_path = format!("{}", tmp_dir.path().display());

    base64_to_file(&docx_base64, &tmp_docx_path).await?;
    let _ = tokio::task::spawn_blocking(move || {
        convert_with_libreoffice(&tmp_docx_path, &tmp_dir_path)
    })
    .await?;
    let output_base64: String = file_to_base64(&tmp_pdf_path).await?;
    Ok(output_base64)
}

async fn base64_to_file(base64_str: &str, file_path: &str) -> anyhow::Result<()> {
    let decoded_data = general_purpose::STANDARD.decode(base64_str)?;
    fs::write(file_path, decoded_data).await?;
    Ok(())
}

async fn file_to_base64(file_path: &str) -> anyhow::Result<String> {
    // Read file as bytes
    let file_data = fs::read(file_path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read file '{}': {}", file_path, e))?;

    // Encode to base64
    let base64_string = general_purpose::STANDARD.encode(file_data);

    Ok(base64_string)
}
