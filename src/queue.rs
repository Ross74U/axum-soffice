use super::soffice;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::{mpsc, oneshot, Mutex};

struct ProcessingRequest {
    docx_base64: String,
    response_tx: oneshot::Sender<anyhow::Result<String>>,
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

    pub async fn process_base64(&self, docx_base64: String) -> anyhow::Result<String> {
        let (response_tx, response_rx) = oneshot::channel();
        let request = ProcessingRequest {
            docx_base64,
            response_tx,
        };
        self.sender.send(request)?;
        match response_rx.await {
            Ok(result) => result,
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
    println!("worker {} started", worker_id);

    loop {
        let processing_request = {
            let mut channel_rx = receiver.lock().await;
            channel_rx.recv().await
        };

        println!("{} active workers", active_counter.display_value());
        if let Some(request) = processing_request {
            active_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match soffice::convert_base64_pdf(&request.docx_base64).await {
                Ok(pdf_base64) => {
                    let _ = request.response_tx.send(Ok(pdf_base64));
                }
                Err(e) => {
                    eprintln!("Error occured on worker_id {}: {}", worker_id, e);
                    let _ = request.response_tx.send(Err(e));
                }
            }
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
