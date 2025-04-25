use std::str::FromStr;

pub(crate) type BoxedInner = dyn std::error::Error + Send + Sync;
pub(crate) type BoxedErr = Box<BoxedInner>;

/// This indicates runtime failure in the underlying platform storage system. The details of the
/// failure can be retrieved from the attached platform error.
#[derive(Debug)]
#[allow(unused)]
pub struct PlatformFailure(BoxedErr);

impl PlatformFailure {
    pub fn new<E>(e: E) -> Self
    where
        E: Into<BoxedErr>,
    {
        Self(e.into())
    }
}

/// This indicates that the underlying secure storage holding saved items could not be accessed.
/// Typically this is because of access rules in the platform; for example, it might be that the
/// credential store is locked. The underlying platform error will typically give the reason.
#[derive(Debug)]
#[allow(unused)]
pub struct NoStorageAccess(BoxedErr);

impl NoStorageAccess {
    pub fn new<E>(e: E) -> Self
    where
        E: Into<BoxedErr>,
    {
        Self(e.into())
    }
}

/// Errors displayed to the user when using the Strata CLI
pub enum DisplayedError {
    /// Errors the use can address by updating configuration or providing expected input
    UserError(String),
    /// Internal errors encountered when servicing user's request.
    InternalError(String, Box<dyn std::fmt::Debug>),
}

pub(crate) fn user_error(msg: impl Into<String>) -> DisplayedError {
    DisplayedError::UserError(msg.into())
}

#[allow(unused)]
pub(crate) fn internal_error<E>(msg: impl AsRef<str>) -> impl FnOnce(E) -> DisplayedError
where
    E: std::fmt::Debug + 'static,
{
    move |e| {
        DisplayedError::InternalError(
            String::from_str(msg.as_ref()).expect("infallible"),
            Box::new(e),
        )
    }
}

pub(crate) trait DisplayableError {
    type T;
    fn user_error(self, msg: impl AsRef<str>) -> Result<Self::T, DisplayedError>;
    fn internal_error(self, msg: impl AsRef<str>) -> Result<Self::T, DisplayedError>;
}

impl<T, E: std::fmt::Debug + 'static> DisplayableError for Result<T, E> {
    type T = T;

    fn user_error(self, msg: impl AsRef<str>) -> Result<Self::T, DisplayedError> {
        self.map_err(|_| DisplayedError::UserError(msg.as_ref().to_string()))
    }

    fn internal_error(self, msg: impl AsRef<str>) -> Result<Self::T, DisplayedError> {
        self.map_err(|e| DisplayedError::InternalError(msg.as_ref().to_string(), Box::new(e)))
    }
}

impl std::fmt::Display for DisplayedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayedError::UserError(msg) => f.write_fmt(format_args!("User input error: {msg}")),
            DisplayedError::InternalError(msg, e) => {
                f.write_fmt(format_args!("Internal error: {msg}: {e:?}"))
            }
        }
    }
}
