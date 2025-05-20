pub enum AppendOnlyListDiff<T> {
    None,
    Append(T),
}
