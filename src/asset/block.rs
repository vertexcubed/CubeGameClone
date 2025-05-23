use crate::asset::AssetLoaderError;
use crate::core::errors::RegistryError;
use crate::core::state::LoadingState;
use bevy::asset::io::Reader;
use bevy::asset::{ron, AssetLoader, AssetPath, LoadContext, RenderAssetUsages};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension};
use bevy::tasks::ConditionalSendFuture;
use bimap::hash::Iter;
use bimap::BiMap;
use serde::Deserialize;
use std::collections::HashMap;


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