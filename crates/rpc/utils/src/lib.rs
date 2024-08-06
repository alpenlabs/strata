use jsonrpsee::types::ErrorObjectOwned;

pub fn to_jsonrpsee_error_object(err: impl ToString, message: &str) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        jsonrpsee::types::error::UNKNOWN_ERROR_CODE,
        message,
        Some(err.to_string()),
    )
}

pub fn to_jsonrpsee_error<T: ToString>(message: &'static str) -> impl Fn(T) -> ErrorObjectOwned {
    move |err: T| to_jsonrpsee_error_object(err, message)
}
