#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<T> {
    pub rows: Vec<T>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}
