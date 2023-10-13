pub enum BackgroundJobState {
    New = 0,
    Started = 1,
    Retrying = 2,
    Cancelled = 3,
    Failed = 4,
    Complete = 5,
}

#[derive(Debug, thiserror::Error)]
pub enum BackgroundJobStateError {
}
