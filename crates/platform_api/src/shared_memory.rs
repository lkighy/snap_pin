use crate::PlatformError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedMemoryCreateRequest {
    pub name: String,
    pub bytes: Vec<u8>,
}

pub trait SharedMemoryLease: Send {}

pub struct SharedMemoryHandle {
    pub name: String,
    pub byte_len: usize,
    lease: Option<Box<dyn SharedMemoryLease>>,
}

impl SharedMemoryHandle {
    pub fn new(name: impl Into<String>, byte_len: usize) -> Self {
        Self {
            name: name.into(),
            byte_len,
            lease: None,
        }
    }

    pub fn with_lease<L>(name: impl Into<String>, byte_len: usize, lease: L) -> Self
    where
        L: SharedMemoryLease + 'static,
    {
        Self {
            name: name.into(),
            byte_len,
            lease: Some(Box::new(lease)),
        }
    }

    pub fn has_lease(&self) -> bool {
        self.lease.is_some()
    }
}

impl std::fmt::Debug for SharedMemoryHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SharedMemoryHandle")
            .field("name", &self.name)
            .field("byte_len", &self.byte_len)
            .field("has_lease", &self.has_lease())
            .finish()
    }
}

pub trait SharedMemory: Send + Sync {
    fn create(
        &self,
        request: SharedMemoryCreateRequest,
    ) -> Result<SharedMemoryHandle, PlatformError>;

    fn open(&self, name: &str, byte_len: usize) -> Result<Vec<u8>, PlatformError>;
}
