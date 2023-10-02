use std::ops::Deref;
use std::sync::Arc;

use axum::extract::FromRef;
use jwt_simple::prelude::*;

use crate::app::State;

#[derive(Clone)]
pub struct SessionVerificationKey(Arc<ES384PublicKey>);

impl SessionVerificationKey {
    pub fn new(key: ES384PublicKey) -> Self {
        Self(Arc::new(key))
    }
}

impl Deref for SessionVerificationKey {
    type Target = Arc<ES384PublicKey>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
