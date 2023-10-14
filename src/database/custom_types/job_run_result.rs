pub enum JobRunResult {
    Panic,
    TimedOut,
    Error,
    Success,
}

impl JobRunResult {
    pub fn as_i32(&self) -> i32 {
        match &self {
            JobRunResult::Panic => 1,
            JobRunResult::TimedOut => 2,
            JobRunResult::Error => 3,
            JobRunResult::Success => 4,
        }
    }

    //pub fn from_i32(val: i32) -> Self {
    //    let variant = match val {
    //    };

    //}
}

#[derive(Debug, thiserror::Error)]
pub enum JobRunResultError {
}
