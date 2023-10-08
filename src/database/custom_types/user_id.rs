use crate::database::custom_types::Did;

#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub struct UserId(Did);

impl From<String> for UserId {
    fn from(val: String) -> Self {
        Self(Did::try_from(val).expect("user ID to be valid"))
    }
}
