mod helpers;

pub(crate) mod prelude {
    pub(crate) use crate::tests::MockState;
    pub(crate) use crate::tests::helpers::TestClient;
}

#[derive(Clone)]
pub(crate) struct MockState {
}
