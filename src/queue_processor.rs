use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

pub trait Handler<Req, Res>: Clone + Send + 'static {
    fn process(&self, req: Req) -> impl Future<Output = Res> + Send;
}

struct ProcessingRequest<Req, Res> {
    request: Req,
    response_tx: oneshot::Sender<Res>,
}

pub struct QueueProcessor<Req, Res> {
    sender: mpsc::UnboundedSender<ProcessingRequest<Req, Res>>,
}

impl<Req, Res> QueueProcessor<Req, Res>
where
    Req: Send + 'static,
    Res: Send + 'static,
{
    pub fn new<H>(num_workers: usize, handler: H) -> Self
    where
        H: Handler<Req, Res> + Send + 'static,
    {
        let (sender, receiver) = mpsc::unbounded_channel();

        let shared_receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..num_workers {
            let receiver = Arc::clone(&shared_receiver);
            let handler = handler.clone();
            tokio::spawn(Self::worker_loop(handler, receiver));
        }
        Self { sender }
    }

    async fn worker_loop<H: Handler<Req, Res>>(
        handler: H,
        rx: Arc<Mutex<mpsc::UnboundedReceiver<ProcessingRequest<Req, Res>>>>,
    ) {
        loop {
            let req = {
                let mut channel_rx = rx.lock().await;
                channel_rx.recv().await
            };
            let Some(req) = req else {continue;};
            let res = handler.process(req.request).await;
            _ = req.response_tx.send(res);
        }
    }

    pub async fn process_in_queue(&self, request: Req) -> Result<Res> {
        let (response_tx, response_rx): (oneshot::Sender<Res>, oneshot::Receiver<Res>) =
            oneshot::channel();

        let request = ProcessingRequest {
            request,
            response_tx,
        };

        self.sender
            .send(request)
            .map_err(|_| anyhow!("failed to send request"))?;
        Ok(response_rx.await?)
    }
}

/// Macro to quickly construct a handler using an async block
/// # EXAMPLE:
///
///```
///handler!(MacroHandler, (req: ExampleRequest) -> ExampleResponse {
///   ExampleResponse {
///       output: format!("Processed {}", req.input),
///   }
///});
///```
#[macro_export]
macro_rules! queue_handler {
    ($vis:vis $name:ident, ($arg:ident: $req:ty) -> $res:ty $handler:block) => {
        #[derive(Clone)]
        $vis struct $name {}

        impl $name {
            pub fn new() -> Self { Self {} }
        }

        impl Handler<$req, $res> for $name {
            async fn process(&self, $arg: $req) -> $res {
                $handler
            }
        }
    };
}
