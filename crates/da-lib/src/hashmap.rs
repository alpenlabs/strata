#[derive(Debug, Clone)]
pub enum HashMapDiff<K, V> {
    None,
    Insert(K, V),
    Remove(K),
    Update(K, V), // Same as Insert ??
}
