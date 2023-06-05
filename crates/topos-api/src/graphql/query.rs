use crate::graphql::certificate::Certificate;

pub trait QueryRoot {
    fn get_all_certificates() -> Vec<Certificate>;
}