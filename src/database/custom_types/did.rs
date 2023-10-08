use uuid::Uuid;

#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub struct Did(Uuid);

impl TryFrom<String> for Did {
    type Error = uuid::Error;

    fn try_from(val: String) -> Result<Self, Self::Error> {
        Uuid::parse_str(&val).map(Self)
    }
}
