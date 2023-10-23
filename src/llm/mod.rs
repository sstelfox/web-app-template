pub mod hugging_face {
    pub const EMBEDDING_MODEL: &str = "thenlper/gte-base";

    pub const RERANKING_MODEL: &str = "BAAI/bge-reranker-base";

    const SAFE_TENSOR_REPO_FMT: &str = "https://huggingface.co/{}/resolve/main/model.safetensors";

    pub struct ModelVersion(String);

    use reqwest::header::{HeaderMap, HeaderValue};
    use reqwest::redirect::Policy;

    fn no_redirect_client() -> Result<reqwest::Client, HuggingFaceError> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .redirect(Policy::none())
            .user_agent("test-app-client/0.1.0")
            .build()
            .map_err(HuggingFaceError::BuildError)?;

        Ok(client)
    }

    pub async fn check_safetensor_model_version(model: &str) -> Result<ModelVersion, HuggingFaceError> {
        let model_url = SAFE_TENSOR_REPO_FMT.replace("{}", model);

        let mut headers = HeaderMap::new();
        headers.insert("Range", HeaderValue::from_static("bytes=0-0"));

        let client = no_redirect_client()?;
        let request = client
            .get(model_url)
            .headers(headers);

        let response = request
            .send()
            .await
            .map_err(HuggingFaceError::VersionCheckError)?;

        println!("Response: {:?} {}", response.version(), response.status());
        println!("Headers: {:#?}\n", response.headers());

        todo!()
    }

    #[derive(Debug, thiserror::Error)]
    pub enum HuggingFaceError {
        #[error("error occurred building a client: {0}")]
        BuildError(reqwest::Error),

        #[error("unable to check model version: {0}")]
        VersionCheckError(reqwest::Error),
    }
}
