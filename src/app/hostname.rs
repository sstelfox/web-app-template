use url::Url;

// todo: I really want to extract this from requests instead of having to make this a config
// option...
#[derive(Clone)]
pub struct Hostname(pub Url);
