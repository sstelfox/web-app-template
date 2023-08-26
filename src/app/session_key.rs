use std::ops::Deref;
use std::sync::Arc;

use jwt_simple::prelude::*;

#[derive(Clone)]
pub struct SessionCreator(Arc<ES384KeyPair>);

impl SessionCreator {
    pub fn new(key: ES384KeyPair) -> Self {
        Self(Arc::new(key))
    }
}

impl SessionCreator {
    pub fn verifier(&self) -> SessionVerifier {
        let key_pair = self.0.clone();
        SessionVerifier::new(key_pair.public_key())
    }
}

impl Deref for SessionCreator {
    type Target = Arc<ES384KeyPair>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub struct SessionVerifier(Arc<ES384PublicKey>);

impl SessionVerifier {
    pub fn new(key: ES384PublicKey) -> Self {
        Self(Arc::new(key))
    }
}

impl Deref for SessionVerifier {
    type Target = Arc<ES384PublicKey>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
