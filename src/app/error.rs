use crate::database::DatabaseSetupError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to parse program command lines")]
    ArgumentError(#[from] pico_args::Error),

    #[error("axum web server experienced critical error")]
    AxumServerError(#[from] hyper::Error),

    #[error("failed to initial the database")]
    DatabaseFailure(#[from] DatabaseSetupError),

    #[error("session key provided could not be parsed as a PEM encoded ES384 private key")]
    InvalidSessionKey(jwt_simple::Error),

    #[error("provided session key was unable to be read")]
    UnreadableSessionKey(std::io::Error),
}

impl Error {
    pub fn invalid_key(err: jwt_simple::Error) -> Self {
        Self::InvalidSessionKey(err)
    }

    pub fn unreadable_key(err: std::io::Error) -> Self {
        Self::UnreadableSessionKey(err)
    }
}
