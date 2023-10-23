pub mod hugging_face {
    pub const EMBEDDING_MODEL: &str = "thenlper/gte-base";

    pub const RERANKING_MODEL: &str = "BAAI/bge-reranker-base";

    const SAFE_TENSOR_REPO_FMT: &str = "https://huggingface.co/{}/resolve/main/model.safetensors";

    pub struct ModelVersion {
        commit: String,
        etag: String,
        size: usize,
    }

    use reqwest::header::{CONTENT_RANGE, LOCATION, RANGE, HeaderMap, HeaderName, HeaderValue, ToStrError};
    use reqwest::redirect::Policy;

    fn no_redirect_client() -> Result<reqwest::Client, HuggingFaceError> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .redirect(Policy::none())
            .user_agent(format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(HuggingFaceError::BuildError)?;

        Ok(client)
    }

    fn header(name: HeaderName, headers: &HeaderMap) -> Result<String, HuggingFaceError> {
        headers
            .get(name)
            .ok_or(HuggingFaceError::MissingHeader)?
            .to_str()
            .map_err(HuggingFaceError::InvalidHeaderValue)
            .map(|v| v.to_string())
    }

    pub async fn check_safetensor_model_version(model: &str) -> Result<ModelVersion, HuggingFaceError> {
        let client = no_redirect_client()?;

        let model_url = SAFE_TENSOR_REPO_FMT.replace("{}", model);

        let mut response = client
            .get(&model_url)
            .send()
            .await
            .map_err(HuggingFaceError::NoMetadata)?;

        let metadata_headers = response.headers();

        // Try and use the custom X-Linked-Etag header to get the cache key for this model falling
        // back on the standard Etag header if its not present
        let etag = match metadata_headers.get(HeaderName::from_static("X-Linked-Etag")) {
            Some(e) => e,
            None => {
                metadata_headers
                    .get(HeaderName::from_static("Etag"))
                    .ok_or(HuggingFaceError::MissingHeader)?
            }
        };
        let etag = etag.to_str().map_err(HuggingFaceError::InvalidHeaderValue)?.to_string().replace('"', "");

        // The commit level is also in a custom header
        let current_commit = header(HeaderName::from_static("X-Repo-Commit"), &metadata_headers)?;

        if response.status().is_redirection() {
            let next_location = header(LOCATION, &metadata_headers)?;

            response = client
                .get(&model_url)
                // We don't actually want the data, indicate as much
                .header(RANGE, "bytes=0-0")
                .send()
                .await
                .map_err(HuggingFaceError::RedirectFailed)?;
        }

        let content_range = header(CONTENT_RANGE, response.headers())?;

        let size = content_range.split('/').last().ok_or(HuggingFaceError::BadContentRange)?.parse().map_err(HuggingFaceError::InvalidSize)?;

        Ok(ModelVersion {
            commit: current_commit,
            etag,
            size,
        })
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
}
