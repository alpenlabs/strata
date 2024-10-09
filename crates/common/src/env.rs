use std::{env, str::FromStr};

/// Parse an `envvar` as `T`. Return `fallback` if env missing or parsing fails.
pub fn parse_env_or<T: FromStr>(envvar: &str, fallback: T) -> T {
    env::var(envvar)
        .map(|s| T::from_str(&s).ok())
        .ok()
        .flatten()
        .unwrap_or(fallback)
}
