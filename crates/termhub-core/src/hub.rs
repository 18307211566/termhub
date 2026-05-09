#[derive(Debug, Default)]
pub struct Hub;

#[derive(Debug, Default)]
pub struct HubHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SubscriberId(pub u64);
