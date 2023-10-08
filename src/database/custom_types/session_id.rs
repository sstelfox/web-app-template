use crate::database::custom_types::Did;

#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub struct SessionId(Did);

impl From<String> for SessionId {
    fn from(val: String) -> Self {
        Self(Did::try_from(val).expect("session ID to be valid"))
    }
}
