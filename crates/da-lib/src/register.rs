/// Diff that only represents for value replacements.
#[derive(Debug, Clone)]
pub enum RegisterDiff<T> {
    None,
    Replace(T),
}
