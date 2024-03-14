use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ToStrError, CONTENT_RANGE, LOCATION, RANGE,
};
use reqwest::redirect::Policy;

const EMBEDDING_MODEL: &str = "thenlper/gte-base";

const RERANKING_MODEL: &str = "BAAI/bge-reranker-base";

const SAFE_TENSOR_REPO_FMT: &str = "https://huggingface.co/{}/resolve/main/model.safetensors";

const HTTP_CLIENT_CONTACT: &str = "https://github.com/sstelfox/web-app-template";

/// The available version information retrieved from HuggingFace.
#[derive(Debug)]
pub struct ModelVersion {
    commit: String,
    etag: Option<String>,
    size: usize,
}

impl ModelVersion {
    pub fn commit(&self) -> &str {
        &self.commit
    }

    pub fn etag(&self) -> Option<&str> {
        self.etag.as_ref().map(|x| x.as_str())
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

/// Performs an online check against HuggingFace to determien what the current version of the
/// remote model is.
///
///
/// # Arguments
///
/// * `model` - The path of the HuggingFace repo including the user namespace.
///
/// # Examples
///
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// #   use web_app_template::llm::hugging_face::check_safetensor_model_version;
///     let model_version = check_safetensor_model_version("thenlper/gte-base").await?;
/// #   Ok(())
/// # }
/// ```
///
/// # Note
///
/// Currently this is limited to the safetensor models but we'll need a variety of
/// model support in the future, at which point this function will likely be renamed and
/// deprecated.
pub async fn check_safetensor_model_version(model: &str) -> Result<ModelVersion, HuggingFaceError> {
    let client = no_redirect_light_client();

    // todo: This really needs to be more generic than just looking at safetensor model versions,
    // bt for now this should be sufficient.
    let model_url = SAFE_TENSOR_REPO_FMT.replace("{}", model);
    let mut response = client
        .get(&model_url)
        .send()
        .await
        .map_err(HuggingFaceError::NoMetadata)?;

    let metadata_headers = response.headers();

    // Try and use the custom X-Linked-Etag header to get the cache key for this model falling
    // back on the standard Etag header if its not present
    let etag = metadata_headers
        .get(HeaderName::from_static("x-linked-etag"))
        .or_else(|| metadata_headers.get(HeaderName::from_static("etag")))
        .map(clean_etag)
        .transpose()?;

    // The commit level is also in a custom header
    let current_commit =
        retrieve_header(HeaderName::from_static("x-repo-commit"), metadata_headers)?;

    if response.status().is_redirection() {
        let next_location = retrieve_header(LOCATION, metadata_headers)?;

        response = client
            .get(&next_location)
            // This request only checks the current version of the repository, it doesn't download
            // anything. Specifically request that no data is returned. This matches the requested
            // behavior HuggingFace has requested for cacheing download clients.
            .header(RANGE, "bytes=0-0")
            .send()
            .await
            .map_err(HuggingFaceError::RedirectFailed)?;
    }

    // HuggingFace lets us know how big the file is going to be so we can make a determination
    // before attempting an actual download.
    let content_range = retrieve_header(CONTENT_RANGE, response.headers())?;

    let size = content_range
        .split('/')
        .last()
        .ok_or(HuggingFaceError::BadContentRange)?
        .parse()
        .map_err(HuggingFaceError::InvalidSize)?;

    Ok(ModelVersion {
        commit: current_commit,
        etag,
        size,
    })
}

/// Converts a response header into the unquoted string. In general Etag headers
/// shouldn't be used to identify a specific version only whether it has changed
/// or not. The [`ModelVersion::commit`] attribute should be used for version
/// identification.
///
/// # Arguments
///
/// * `etag` - The [`HeaderValue`] returned in the etag header from a huggingface
///   repo response.
///
/// # Note
///
/// In the future this function may start returning a digest over the raw etag
/// string to prevent accidental misuse.
fn clean_etag(etag: &HeaderValue) -> Result<String, HuggingFaceError> {
    etag.to_str()
        .map_err(HuggingFaceError::InvalidHeaderValue)
        .map(|v| v.to_string().replace('"', ""))
}

/// Returns a configured HTTP client that allows us to handle redirects in a custom way. We are a
/// good netizen and set a custom user agent to allow remote hosts to identify us.
fn no_redirect_light_client() -> reqwest::Client {
    let mut default_headers = HeaderMap::new();
    default_headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    let user_agent = format!(
        "{}/{}; +{HTTP_CLIENT_CONTACT}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    // todo: add a timeout to these request
    let client = reqwest::Client::builder()
        .default_headers(default_headers)
        .redirect(Policy::none())
        .user_agent(user_agent)
        .build()
        .expect("static client build should always succeed");

    client
}

fn retrieve_header(name: HeaderName, headers: &HeaderMap) -> Result<String, HuggingFaceError> {
    headers
        .get(name)
        .ok_or(HuggingFaceError::MissingHeader)?
        .to_str()
        .map_err(HuggingFaceError::InvalidHeaderValue)
        .map(|v| v.to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum HuggingFaceError {
    #[error("bad format for content range header")]
    BadContentRange,

    #[error("error occurred building a client: {0}")]
    BuildError(reqwest::Error),

    #[error("expected a header to be a valid string")]
    InvalidHeaderValue(ToStrError),

    #[error("the provided content size wasn't a number")]
    InvalidSize(std::num::ParseIntError),

    #[error("a required header was missing")]
    MissingHeader,

    #[error("unable to make first metadata request: {0}")]
    NoMetadata(reqwest::Error),

    #[error("attempting to follow the provided redirect failed: {0}")]
    RedirectFailed(reqwest::Error),
}
