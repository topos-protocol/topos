use super::v1::SubnetId;

impl<T> From<T> for SubnetId
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        Self {
            value: value.into(),
        }
    }
}
