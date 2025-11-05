pub mod block;
mod machine;

use crate::asset::block::BlockAsset;
use crate::core::errors::RegistryError;
use crate::core::state::LoadingState;
use crate::core::AllBlockAssets;
use crate::registry::block::Block;
use bevy::prelude::*;
use std::collections::hash_map::Iter;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

/// Plugin that handles registries and registration for certain game object types. 
/// Examples of this include blocks, items, level entities, machines, etc.,
#[derive(Default)]
pub struct RegistryPlugin;

impl Plugin for RegistryPlugin {
    fn build(&self, app: &mut App) {
        app
            // .insert_resource(BlockRegistry::new())
            .insert_resource(Registry::<Block>::new("block"))
            .add_systems(OnEnter(LoadingState::Registries), create_block_registry)
            .add_systems(OnExit(LoadingState::Registries), freeze_registries)
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


//===============
//    Systems
//===============
fn create_block_registry(
    mut block_reg: ResMut<Registry<Block>>,
    all_block_handles: Res<AllBlockAssets>,
    block_asset: Res<Assets<BlockAsset>>,
    mut next_load_state: ResMut<NextState<LoadingState>>,
) -> Result<(), BevyError> {

    info!("Creating block registry.");

    for h in all_block_handles.inner.iter() {
        let block = Block::from_asset(block_asset.get(h).unwrap());
        block_reg.register(block)?;
    }
    next_load_state.set(LoadingState::Textures);

    Ok(())
}


// freezes registries, moving them to ReadOnlyRegistry resources which are backed by an arc
fn freeze_registries(
    world: &mut World
) {
    // old writeable registry is removed from the world, and replaced with a Read Only Registry that is backed by an arc.
    let mut old_reg = world.remove_resource::<Registry<Block>>().unwrap();
    old_reg.freeze();
    world.insert_resource(RegistryHandle::new(old_reg));
}