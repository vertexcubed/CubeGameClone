pub mod block;

use std::collections::hash_map::Iter;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use bevy::prelude::*;
use crate::core::errors::RegistryError;
use crate::registry::block::Block;

#[derive(Default)]
pub struct RegistryPlugin;

impl Plugin for RegistryPlugin {
    fn build(&self, app: &mut App) {
        app
            // .insert_resource(BlockRegistry::new())
            .insert_resource(Registry::<Block>::new("block"))
        ;
    }
}

/// A type that can be registered in a Registry.
/// Must have some way of getting a string id, and maybe make a default value.
pub trait RegistryObject {

    /// returns the id of this registry object as a String
    fn get_id(&self) -> &str;

    /// creates an initial instance of this registry object, or None if no such initial value exists.
    /// Usually used to represent a "Nothing" case, e.g. Air, empty item, etc.
    /// When registries are created, an initial value is created and registered immediately.
    fn make_initial() -> Option<Self> where Self: Sized;
}


/// A map of String ids to objects, representing something that can be "registered" during game load. This includes stuff like blocks, items, machines, etc.
///
/// When accessing registries in a system, use `Res<RegistryHandle<T>>` after registration, and `Res<Registry<T>>` during registration.
#[derive(Resource, Debug)]
pub struct Registry<T: RegistryObject> {
    name: String,
    map: HashMap<String, T>,
    frozen: bool,
}

impl <T: RegistryObject> Registry<T> {
    pub fn new(name: &str) -> Self {
        let mut map = HashMap::new();
        if let Some(initial) = T::make_initial() {
            map.insert(initial.get_id().to_string(), initial);
        }
        Self {
            name: name.to_string(),
            map,
            frozen: false,
        }
    }

    pub fn register(&mut self, obj: T) -> std::result::Result<(), RegistryError> {
        let id = obj.get_id();
        if self.frozen {
            Err(RegistryError::Frozen(self.name.clone()))
        }
        else if self.map.contains_key(id) {
            Err(RegistryError::Duplicate(String::from(id), self.name.clone()))
        }
        else {
            self.map.insert(String::from(id), obj);
            Ok(())
        }
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.map.get(id)
    }

    pub fn iter(&self) -> Iter<'_, String, T> {
        self.map.iter()
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }
}


/// Represents a pointer to a registry. This is backed by an `Arc<T>`, so it is cheap to clone, and it can be shared with
/// multiple threads.
/// 
/// When accessing registries in a system, use `Res<RegistryHandle<T>>` after registration, and `Res<Registry<T>>` during registration.
#[derive(Debug, Resource)]
pub struct RegistryHandle<T: RegistryObject> {
    inner: Arc<Registry<T>>
}
impl <T: RegistryObject> RegistryHandle<T> {
    pub fn new(registry: Registry<T>) -> Self {
        Self {
            inner: Arc::new(registry)
        }
    }
    pub fn get(&self) -> &Arc<Registry<T>> {
        &self.inner
    }
}

impl <T: RegistryObject> Clone for RegistryHandle<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone()
        }
    }
}

impl <T: RegistryObject> AsRef<Registry<T>> for RegistryHandle<T> {
    fn as_ref(&self) -> &Registry<T> {
        self.inner.as_ref()
    }
}

impl <T: RegistryObject> From<Registry<T>> for RegistryHandle<T> {
    fn from(value: Registry<T>) -> Self {
        Self::new(value)
    }
}

impl <T: RegistryObject> Deref for RegistryHandle<T> {
    type Target = Registry<T>;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}