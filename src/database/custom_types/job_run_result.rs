pub enum JobRunResult {
    Error = 1,
    Panic = 2,
    TimedOut = 3,
    Success = 4,
}

#[derive(Debug, thiserror::Error)]
pub enum JobRunResultError {
}
