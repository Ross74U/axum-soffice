use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use std::{process::Command, process::Stdio};
use tempfile::TempDir;
use tokio::*;

pub struct Daemon {
    child: std::process::Child,
    port: u16,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        println!("Closing unoserver on port {}", self.port);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start a LibreOffice unoserver daemon on the given port
pub fn start_unoserver_daemon(port: u16) -> Result<Daemon> {
    let child = Command::new("unoserver")
        .args(["--daemon", "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    // Give the daemon a short moment to start before returning
    std::thread::sleep(std::time::Duration::from_secs(1));

    Ok(Daemon { child, port })
}

/// Do the actual conversion via `unoconvert`
fn convert_with_unoconvert(input: &str, output: &str, port: u16) -> anyhow::Result<()> {
    println!("running unoconvert request to port {}", port);
    let status = Command::new("unoconvert")
        .args(["--port", &port.to_string(), input, output])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit()) // capture stderr while debugging
        .status()?;

    if !status.success() {
        println!("unoconvert request to port {} exited with {}", port, status);
        anyhow::bail!("unoconvert exited with {}", status);
    }
    Ok(())
}

pub async fn convert_file_path(docx_path: &str, pdf_path: &str, port: u16) -> anyhow::Result<()> {
    let docx_path = String::from(docx_path);
    let pdf_path = String::from(pdf_path);
    tokio::task::spawn_blocking(move || convert_with_unoconvert(&docx_path, &pdf_path, port))
        .await?
}

pub async fn convert_base64_pdf(docx_base64: &str, port: u16) -> anyhow::Result<String> {
    let tmp_dir = TempDir::new()?;
    let tmp_docx_path = format!("{}/tmp.docx", tmp_dir.path().display());
    let tmp_pdf_path = format!("{}/tmp.pdf", tmp_dir.path().display());
    let tmp_pdf_path_clone = tmp_pdf_path.clone();

    base64_to_file(docx_base64, &tmp_docx_path).await?;

    let _ = tokio::task::spawn_blocking(move || {
        convert_with_unoconvert(&tmp_docx_path, &tmp_pdf_path_clone, port)
    })
    .await?;

    let output_base64: String = file_to_base64(&tmp_pdf_path)
        .await
        .map_err(|e| anyhow!("[file_to_base64] {}", e))?;
    Ok(output_base64)
}

pub async fn base64_to_file(base64_str: &str, file_path: &str) -> anyhow::Result<()> {
    let decoded_data = general_purpose::STANDARD.decode(base64_str)?;
    fs::write(file_path, decoded_data).await?;
    Ok(())
}

pub async fn file_to_base64(file_path: &str) -> anyhow::Result<String> {
    let file_data = fs::read(file_path).await?;
    Ok(general_purpose::STANDARD.encode(file_data))
}
