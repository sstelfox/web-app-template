pub(crate) fn collect_error_messages(base_error: impl std::error::Error) -> Vec<String> {
    let mut errors = vec![base_error.to_string()];
    let mut source = base_error.source();

    while let Some(err) = source {
        errors.push(err.to_string());
        source = err.source();
    }

    errors
}
