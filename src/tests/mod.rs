mod helpers;

pub(crate) mod prelude {
    pub(crate) use crate::tests::MockState;
}

#[derive(Clone)]
pub(crate) struct MockState;
