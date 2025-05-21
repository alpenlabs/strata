#[derive(Debug, Clone)]
pub enum ListDiff<T> {
    None,
    Pop,
    Extend(Vec<T>),
    DeleteAt(usize),
    Replace(Vec<T>),
}
