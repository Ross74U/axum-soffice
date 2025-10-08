use base64::{engine::general_purpose, Engine as _};
use tokio::fs;

/* Utils */
pub async fn base64_to_file(base64_str: &str, file_path: &str) -> anyhow::Result<()> {
    let decoded_data = general_purpose::STANDARD.decode(base64_str)?;
    fs::write(file_path, decoded_data).await?;
    Ok(())
}

pub async fn file_to_base64(file_path: &str) -> anyhow::Result<String> {
    // Read file as bytes
    let file_data = fs::read(file_path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read file '{}': {}", file_path, e))?;

    // Encode to base64
    let base64_string = general_purpose::STANDARD.encode(file_data);

    Ok(base64_string)
}
