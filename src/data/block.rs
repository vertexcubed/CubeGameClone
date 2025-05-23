use crate::asset::AssetLoaderError;
use crate::errors::RegistryError;
use bevy::asset::io::Reader;
use bevy::asset::{ron, AssetLoader, AssetPath, LoadContext};
use bevy::prelude::*;
use bevy::tasks::ConditionalSendFuture;
use bimap::BiMap;
use bimap::hash::Iter;
use serde::Deserialize;

#[derive(Resource, Default, Debug)]
pub struct AllBlocks {
    map: BiMap<String, Handle<Block>>,
    
    frozen: bool,
}
impl AllBlocks {
    pub fn get_block(&self, name: &str) -> Option<Handle<Block>> {
        self.map.get_by_left(name).cloned()
    }
    
    pub fn get_id(&self, block: &Handle<Block>) -> Option<String> {
        self.map.get_by_right(block).cloned()
    }
    
    pub fn add(&mut self, name: &str, value: Handle<Block>) -> Result<(), RegistryError>{
        if self.frozen {
            return Err(RegistryError::Frozen("block".to_string()));
        }
        
        if let Some(_) = self.map.get_by_left(name) {
            return Err(RegistryError::Duplicate(name.to_string(), "block".to_string()));
        }
        
        self.map.insert(name.to_string(), value);
        Ok(())
    }
    
    pub fn iter(&self) -> Iter<'_, String, Handle<Block>> {
        self.map.iter()
    }
    
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }
    
    // purposely cannot unfreeze a registry
    pub fn freeze(&mut self) {
        self.frozen = true;
        info!("Freezing block registry.");
    }
}







#[derive(Debug, Hash, Clone, PartialEq, Eq, Asset, TypePath, Deserialize)]
pub struct Block {
    pub id: String,
    pub hardness: u32,
    model: String,
    #[serde(skip)]
    pub model_handle: Handle<BlockModel>
}


#[derive(Default)]
pub struct BlockLoader;

impl AssetLoader for BlockLoader {
    type Asset = Block;
    type Settings = ();
    type Error = AssetLoaderError;

    fn load(&self, reader: &mut dyn Reader, _settings: &Self::Settings, load_context: &mut LoadContext) -> impl ConditionalSendFuture<Output=std::result::Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let mut block = ron::de::from_bytes::<Block>(&bytes)?;

            // get the model handle from the model path
            let model_str = format!("model/{}.model.ron", block.model.clone());
            block.model_handle = load_context.load(AssetPath::parse(model_str.as_str()));


            Ok(block)
        })
    }

    fn extensions(&self) -> &[&str] {
        &["block.ron"]
    }
}


#[derive(Debug, Hash, Clone, PartialEq, Eq, Asset, TypePath, Deserialize)]
pub struct BlockModel {
    texture: String,
    #[serde(skip)]
    pub texture_handle: Handle<Image>
}

#[derive(Default)]
pub struct BlockModelLoader;
impl AssetLoader for BlockModelLoader {
    type Asset = BlockModel;
    type Settings = ();
    type Error = AssetLoaderError;

    fn load(&self, reader: &mut dyn Reader, settings: &Self::Settings, load_context: &mut LoadContext) -> impl ConditionalSendFuture<Output=std::result::Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let mut model = ron::de::from_bytes::<BlockModel>(&bytes)?;

            // get the model handle from the model path
            let texture_str = format!("texture/{}.png", model.texture.clone());
            model.texture_handle = load_context.load(AssetPath::parse(texture_str.as_str()));


            Ok(model)
        })
    }

    fn extensions(&self) -> &[&str] {
        &["model.ron"]
    }
}