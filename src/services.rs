mod auth_client_service;
mod faction_relations_service;
mod bot_db_service;
mod ss14_database_service;

pub use auth_client_service::*;
pub use faction_relations_service::*;
pub use bot_db_service::*;
pub use ss14_database_service::*;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Default)]
pub struct ServicesContainer {
    services: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl ServicesContainer {
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<T: Any + Send + Sync>(&self, service: T) {
        let mut guard = self.services.write().unwrap();
        guard.insert(TypeId::of::<T>(), Arc::new(service));
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let guard = self.services.read().unwrap();
        guard
            .get(&TypeId::of::<T>())
            .map(|service| Arc::downcast::<T>(Arc::clone(service)).unwrap())
    }

    pub fn get_unsafe<T: Any + Send + Sync>(&self) -> Arc<T> {
        self.get().unwrap()
    }
}
