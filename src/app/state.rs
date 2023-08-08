use crate::app::Config;

#[derive(Clone)]
pub struct State {
}

impl State {
    // not implemented as a From trait so it can be async
    pub async fn from_config(_config: &Config) -> Self {
        Self {}
    }
}
