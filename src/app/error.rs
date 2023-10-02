use crate::database::DatabaseError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to parse program command lines")]
    ArgumentError(#[from] pico_args::Error),

    #[error("axum web server experienced critical error")]
    AxumServerError(#[from] hyper::Error),

    #[error("failed to initial the database")]
    DatabaseFailure(#[from] DatabaseError),

    #[error("unable to read config items from the environment")]
    EnvironmentUnavailable(dotenvy::Error),

    #[error("provided database url wasn't a valid URI")]
    InvalidDatabaseUrl(url::ParseError),

    #[error("listen address wasn't a valid socket address")]
    InvalidListenAddr(std::net::AddrParseError),

    #[error("session key provided could not be parsed as a PEM encoded ES384 private key")]
    InvalidSessionKey(jwt_simple::Error),

    #[error("provided smtp url wasn't a valid URI")]
    InvalidSmtpUrl(url::ParseError),

    #[error("client ID for performing Google OAuth was not provided")]
    MissingGoogleClientId,

    #[error("client secret for performing Google OAuth was not provided")]
    MissingGoogleClientSecret,

    #[error("provided session key was unable to be read")]
    UnreadableSessionKey(std::io::Error),

    #[error("unable to write generated session key")]
    UnwritableSessionKey(std::io::Error),
}
