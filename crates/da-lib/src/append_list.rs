#[derive(Debug, Clone)]
pub enum AppendOnlyListDiff<T> {
    None,
    Append(T),
}
