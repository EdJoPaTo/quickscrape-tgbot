/// Create a [`LazyLock`](std::sync::LazyLock) [`scraper::Selector`]
macro_rules! selector {
    ($selector:literal) => {{
        static SELECTOR: std::sync::LazyLock<scraper::Selector> =
            std::sync::LazyLock::new(|| scraper::Selector::parse($selector).unwrap());
        &SELECTOR
    }};
}
pub(crate) use selector;
