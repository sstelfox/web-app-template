#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to parse program command lines")]
    ArgumentError(#[from] pico_args::Error),

    #[error("axum web server experienced critical error")]
    AxumServerError(#[from] hyper::Error),
}
