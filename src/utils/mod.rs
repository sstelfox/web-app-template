use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::CookieJar;
use time::OffsetDateTime;

pub fn remove_cookie(name: &'static str, mut cookie_jar: CookieJar) -> CookieJar {
    cookie_jar = cookie_jar.remove(Cookie::named(name));
    cookie_jar.add(
        Cookie::build(name, "")
            .path("/")
            .http_only(false)
            .expires(OffsetDateTime::UNIX_EPOCH)
            .finish(),
    )
}
