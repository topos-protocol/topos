use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use crate::graphql::query::QueryRoot;

pub type ServiceSchema = Schema<dyn QueryRoot, EmptyMutation, EmptySubscription>;