use std::ops::Deref;

use object_store::local::LocalFileSystem;

pub struct UploadStore(LocalFileSystem);

impl UploadStore {
    pub fn new(inner: LocalFileSystem) -> Self {
        Self(inner)
    }
}

impl Deref for UploadStore {
    type Target = LocalFileSystem;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
