#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PublishType {
    #[default]
    Normal,
    Temporary,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PublishSource {
    #[default]
    Client,
    SessionSync,
}
