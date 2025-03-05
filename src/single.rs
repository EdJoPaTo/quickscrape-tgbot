pub trait Single<T> {
    fn single(&mut self) -> Result<T, SingleError>;
}

#[derive(Debug)]
pub enum SingleError {
    Zero,
    MoreThanOne,
}

impl core::fmt::Display for SingleError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zero => fmt.pad("Expected a single element but got none"),
            Self::MoreThanOne => fmt.pad("Expected a single element but got more than one"),
        }
    }
}

impl core::error::Error for SingleError {}

impl<I, T> Single<T> for I
where
    I: Iterator<Item = T>,
{
    fn single(&mut self) -> Result<T, SingleError> {
        let first = self.next();
        if self.next().is_some() {
            return Err(SingleError::MoreThanOne);
        }
        first.ok_or(SingleError::Zero)
    }
}
