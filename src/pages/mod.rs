use askama::Template;

use crate::extractors::SessionIdentity;

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub session: SessionIdentity,
}

#[derive(Template)]
#[template(path = "not_found.html")]
pub struct NotFoundTemplate;
