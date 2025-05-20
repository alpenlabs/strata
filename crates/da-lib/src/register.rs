/// Diff that only represents for value replacements.
pub enum RegisterDiff<T> {
    None,
    Replace(T),
}
