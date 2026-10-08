use std::sync::{Arc, atomic::AtomicI32};

use arc_swap::ArcSwap;
use rsa::RsaPublicKey;
use ruthenium_config::{AdvancedConfig, BasicConfig};
use tokio::{sync::OnceCell, task::JoinHandle};
use tokio_util::task::TaskTracker;

use crate::server::key_store::KeyStore;

mod key_store;

pub struct Server {
    pub basic_config: BasicConfig,
    pub advanced_config: AdvancedConfig,

    key_store: OnceCell<Arc<KeyStore>>,
    pub mojang_public_keys: ArcSwap<Vec<RsaPublicKey>>,
    pub tick_count: AtomicI32,
    tasks: TaskTracker,
    pub runtime: tokio::runtime::Handle,
}

impl Server {
    pub async fn new(
        basic_config: BasicConfig,
        advanced_config: AdvancedConfig,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let server = Self {
            basic_config,
            advanced_config,
            key_store: OnceCell::new(),
            mojang_public_keys: ArcSwap::from_pointee(Vec::new()),
            tick_count: AtomicI32::new(0),
            tasks: TaskTracker::new(),
            runtime: tokio::runtime::Handle::current(),
        };

        let server = Arc::new(server);

        let server_clone = server.clone();
        server.spawn_task(async move {
            let key_store = Arc::new(KeyStore::new());
            let _ = server_clone.key_store.set(key_store);
        });

        Ok(server)
    }

    pub fn spawn_task<F>(&self, task: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.tasks.spawn_on(task, &self.runtime)
    }
}
