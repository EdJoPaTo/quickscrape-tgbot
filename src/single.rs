use scraper::ElementRef;
use scraper::html::Select;

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

impl<'a> Single<ElementRef<'a>> for Select<'a, '_> {
    fn single(&mut self) -> Result<ElementRef<'a>, SingleError> {
        let first = self.next();
        if self.next().is_some() {
            return Err(SingleError::MoreThanOne);
        }
        first.ok_or(SingleError::Zero)
    }
}
