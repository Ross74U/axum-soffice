use super::soffice;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::{mpsc, oneshot, Mutex};

struct ProcessingRequest {
    input: ProcessingInput,
    response_tx: oneshot::Sender<anyhow::Result<ProcessingResponse>>,
}

enum ProcessingInput {
    Base64String(String),
    FilePathInput(FilePathInput),
}

struct FilePathInput {
    tmp_docx_path: String,
    tmp_dir_path: String,
}

enum ProcessingResponse {
    FilePathConverted,
    Base64String(String),
}

pub struct QueueProcessor {
    sender: mpsc::UnboundedSender<ProcessingRequest>,
}

impl QueueProcessor {
    pub fn new(num_workers: usize) -> anyhow::Result<Self> {
        let (sender, receiver) = mpsc::unbounded_channel();

        let shared_receiver = Arc::new(Mutex::new(receiver));
        let active_counter = Arc::new(AtomicUsize::new(0));
        for worker_id in 0..num_workers {
            let receiver = Arc::clone(&shared_receiver);
            let active_counter = Arc::clone(&active_counter);
            tokio::spawn(async move { worker(worker_id, receiver, active_counter).await });
        }
        Ok(Self { sender })
    }

    pub async fn process_file_path(
        &self,
        tmp_docx_path: &str,
        tmp_dir_path: &str,
    ) -> anyhow::Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        let input = FilePathInput {
            tmp_docx_path: tmp_docx_path.to_string(),
            tmp_dir_path: tmp_dir_path.to_string(),
        };
        let request = ProcessingRequest {
            input: ProcessingInput::FilePathInput(input),
            response_tx,
        };
        self.sender.send(request)?;
        match response_rx.await {
            Ok(result) => match result {
                Ok(ProcessingResponse::FilePathConverted) => Ok(()),
                Ok(_) => Err(anyhow::anyhow!("Expected FilePathConverted")),
                Err(e) => Err(e),
            },
            Err(_) => {
                anyhow::bail!("worker disconnected before sending result");
            }
        }
    }

    pub async fn process_base64(&self, docx_base64: String) -> anyhow::Result<String> {
        let (response_tx, response_rx) = oneshot::channel();
        let request = ProcessingRequest {
            input: ProcessingInput::Base64String(docx_base64),
            response_tx,
        };
        self.sender.send(request)?;
        match response_rx.await {
            Ok(result) => match result {
                Ok(ProcessingResponse::Base64String(s)) => Ok(s),
                Ok(_) => Err(anyhow::anyhow!("Expected FilePathConverted")),
                Err(e) => Err(e),
            },
            Err(_) => {
                anyhow::bail!("worker disconnected before sending result");
            }
        }
    }
}

async fn worker(
    worker_id: usize,
    receiver: Arc<Mutex<mpsc::UnboundedReceiver<ProcessingRequest>>>,
    active_counter: Arc<AtomicUsize>,
) {
    println!("worker {} started", worker_id + 1);

    loop {
        let processing_request = {
            let mut channel_rx = receiver.lock().await;
            channel_rx.recv().await
        };

        if let Some(req) = processing_request {
            active_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            println!("{} active workers", active_counter.display_value());

            let result = match req.input {
                ProcessingInput::Base64String(docx_base64) => {
                    match soffice::convert_base64_pdf(&docx_base64).await {
                        Ok(pdf_string) => Ok(ProcessingResponse::Base64String(pdf_string)),
                        Err(e) => Err(e),
                    }
                }
                ProcessingInput::FilePathInput(file_path_input) => {
                    match soffice::convert_file_path(
                        &file_path_input.tmp_docx_path,
                        &file_path_input.tmp_dir_path,
                    )
                    .await
                    {
                        Ok(_) => Ok(ProcessingResponse::FilePathConverted),
                        Err(e) => Err(e),
                    }
                }
            };

            _ = req.response_tx.send(result);
            active_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        };
    }
}

trait AtomicDisplay {
    fn display_value(&self) -> String;
}

impl AtomicDisplay for AtomicUsize {
    fn display_value(&self) -> String {
        self.load(Ordering::Relaxed).to_string()
    }
}
