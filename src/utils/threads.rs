use anyhow::{Result, anyhow};

pub fn join_thread(thread: std::thread::JoinHandle<()>) -> Result<()> {
    thread
        .join()
        .map_err(|e| match e.downcast_ref::<&'static str>() {
            Some(s) => anyhow!("Background thread failed: {}", *s),
            None => match e.downcast_ref::<String>() {
                Some(s) => anyhow!("Background thread failed: {}", s),
                None => anyhow!("Background thread failed with unknown error type."),
            },
        })
}
