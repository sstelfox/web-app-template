use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::response::{IntoResponse, Response};
use axum::RequestPartsExt;
use http::request::Parts;
use http::{HeaderValue, StatusCode};

pub struct Requestor {
    do_not_track: bool,

    //client_ip: std::net::IpAddr,
    //user_agent: String,
    referrer: Option<String>,
}

impl Requestor {
    /// Used for various internal source tracking and security measures. When the user agent send a
    /// Do-Not-Track signal we respect that and only return the referrer if it matches our origin.
    ///
    /// We'll track path-through-the-application still but nothing about the user or where they
    /// originated from outside our domain.
    pub fn referrer(&self) -> Option<String> {
        if self.do_not_track {
            None
        } else {
            self.referrer.clone()
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Requestor
where
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let mut requestor = Self {
            do_not_track: false,
            referrer: None,
        };

        for val in parts.headers.get_all(http::header::REFERER) {
            if let Ok(new_ref) = val.to_str() {
                requestor.referrer = match requestor.referrer {
                    Some(referrer) => Some([&referrer, new_ref].join("-/-")),
                    None => Some(new_ref.to_string()),
                };
            }
        }

        if let Some(dnt_val) = parts.headers.get(http::header::DNT) {
            if dnt_val == "1" {
                requestor.do_not_track = true;
            }
        }

        Ok(requestor)
    }
}
