use crate::graphql::subnet::SubnetId;

#[derive(Debug, serde::Serialize, serde::Deserialize, async_graphql::OneofObject)]
pub enum SubnetFilter {
    Source(SubnetId),
    Target(SubnetId),
}
