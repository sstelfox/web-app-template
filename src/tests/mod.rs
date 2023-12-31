#![allow(dead_code)]

mod helpers;

pub(crate) mod prelude {
    #![allow(unused_imports)]

    pub(crate) use crate::tests::helpers::{test_database, TestClient};
    pub(crate) use crate::tests::MockState;
}

#[derive(Clone)]
pub(crate) struct MockState;
