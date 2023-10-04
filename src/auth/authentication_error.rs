#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError<'a> {
    #[error("no credentials available for provider '{0}'")]
    ProviderNotConfigured(&'a str),

    #[error("attempted to authenticate against an unknown provider")]
    UnknownProvider,
}
