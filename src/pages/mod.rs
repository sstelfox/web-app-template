use crate::database::custom_types::{OAuthProviderAccountId, SessionId, UserId};

#[derive(askama::Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub provider_account_id: OAuthProviderAccountId,
    pub session_id: SessionId,
    pub user_id: UserId,
}
