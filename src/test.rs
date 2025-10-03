use super::*;
use reqwest::Body;
use std::time::Instant;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

async fn start_test_server() -> String {
    let app = create_app(10);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{}", addr)
}

#[tokio::test]
async fn load_test_stream() {
    let max_concurrent: usize = 20;
    let total_requests: usize = 50;
    let file_path = "docs/text-and-image.docx";

    let shared_server_url = Arc::new(start_test_server().await);
    println!("Test server started on {}", shared_server_url);

    let shared_client = Arc::new(reqwest::Client::new());
    let shared_semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

    let mut client_handles = vec![];

    for client_id in 0..total_requests {
        let semaphore = Arc::clone(&shared_semaphore);
        let server_url = Arc::clone(&shared_server_url);
        let client = Arc::clone(&shared_client);
        let file_path = file_path.to_string();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            println!("starting request {}", client_id);

            // open the file fresh for this request
            let file = File::open(&file_path).await.unwrap();

            // turn into an async byte stream
            let stream = FramedRead::new(file, BytesCodec::new());
            let body = Body::wrap_stream(stream);

            // send request with streamed body
            let res = client
                .post(format!("{}/convert_stream", server_url))
                .body(body)
                .send()
                .await
                .unwrap();

            if res.status().is_success() {
                // e.g. server streams PDF back as raw bytes
                let pdf_bytes = res.bytes().await.unwrap();
                tokio::fs::write(format!("results/{}.pdf", client_id), pdf_bytes)
                    .await
                    .unwrap();

                println!("request {} completed successfully", client_id);
            } else {
                if let Ok(error_text) = res.text().await {
                    println!("Server error: {}", error_text);
                }
                panic!("Server responded with unsuccessful status");
            }
        });

        client_handles.push(handle);
    }

    let start_time = Instant::now();
    for handle in client_handles {
        handle.await.unwrap();
    }
    let end_time = Instant::now();
    println!(
        "requests finished in {}s",
        (end_time - start_time).as_secs()
    );
}

#[tokio::test]
async fn load_test_b64() {
    let max_concurrent: usize = 20;
    let total_requests: usize = 50;
    let file_path = "docs/text-and-image.docx";

    let shared_server_url = Arc::new(start_test_server().await);
    println!("Test server started on {}", shared_server_url);

    // create client requests
    let shared_client = Arc::new(reqwest::Client::new());
    let base64_docx_bytes = bytes::Bytes::from(soffice::file_to_base64(file_path).await.unwrap());
    let mut client_handles = vec![];
    let shared_semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

    for client_id in 0..total_requests {
        let semaphore = Arc::clone(&shared_semaphore);
        let server_url = Arc::clone(&shared_server_url);
        let client = Arc::clone(&shared_client);
        let body = base64_docx_bytes.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            println!("starting request {}", client_id);

            let res = client
                .post(format!("{}/convertb64", server_url))
                .body(body)
                .send()
                .await
                .unwrap();

            if res.status().is_success() {
                let base64_pdf = res.text().await.unwrap();
                soffice::base64_to_file(&base64_pdf, &format!("results/{}.pdf", client_id))
                    .await
                    .unwrap();

                println!("request {} completed successfully", client_id);
            } else {
                // Try to get the response body for more details
                if let Ok(error_text) = res.text().await {
                    println!("Server error: {}", error_text);
                }
                panic!("Server responded with unsuccessful status");
            }
        });
        client_handles.push(handle);
    }

    let start_time = Instant::now();
    for handle in client_handles {
        handle.await.unwrap();
    }
    let end_time = Instant::now();
    let duration = end_time - start_time;
    println!("requests finished in {}s", duration.as_secs());
}
