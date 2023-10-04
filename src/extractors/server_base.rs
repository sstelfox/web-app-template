
use url::Url;

const X_FORWARDED_HOST_HEADER_KEY: &str = "X-Forwarded-Host";

const X_FORWARDED_SCHEME_HEADER_KEY: &str = "X-Forwarded-Proto";

pub struct ServerBase(Url);
