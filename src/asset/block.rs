use crate::asset::AssetLoaderError;
use crate::asset::AssetLoaderError::{InvalidRon, Io};
use crate::core::errors::RegistryError;
use crate::core::state::LoadingState;
use crate::world::block::Direction;
use bevy::asset::io::Reader;
use bevy::asset::{ron, AssetLoader, AssetPath, LoadContext, RenderAssetUsages};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension};
use bevy::tasks::ConditionalSendFuture;
use bimap::hash::Iter;
use bimap::BiMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::iter::Map;

#[derive(Debug, Hash, Clone, PartialEq, Eq, Asset, TypePath, Serialize, Deserialize)]
#[serde(rename="Block")]
pub struct BlockAsset {
    pub id: String,
    pub hardness: u32,
    pub states: Vec<BlockStateAsset>,
    pub default_state: BTreeMap<String, String>,
    pub models: Vec<BlockStateModelDef>
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename="BlockState")]
pub struct BlockStateAsset {
    pub name: String,
    pub values: Vec<String>
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename="ModelDef")]
pub struct BlockStateModelDef {
    pub state: BTreeMap<String, String>,
    model: String,
    #[serde(skip)]
    pub model_handle: Handle<BlockModelAsset>
}


fn get_state<'a>(key: &str, states: &'a Vec<BlockStateAsset>) -> Option<&'a BlockStateAsset> {
    for state in states.iter() {
        if state.name == key {
            return Some(state);
        }
    }
    None
}


#[derive(Default)]
pub struct BlockLoader;

impl AssetLoader for BlockLoader {
    type Asset = BlockAsset;
    type Settings = ();
    type Error = AssetLoaderError;

    fn load(&self, reader: &mut dyn Reader, _settings: &Self::Settings, load_context: &mut LoadContext) -> impl ConditionalSendFuture<Output=std::result::Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let mut block = ron::de::from_bytes::<BlockAsset>(&bytes)?;
            
            validate_state(block.id.as_str(), &block.default_state, &block.states)?;


            // let model_str = format!("model/{}.model.ron", state.model.clone());
            // state.model_handle = load_context.load(AssetPath::parse(model_str.as_str()));

            for model_def in block.models.iter_mut() {
                let active_state = &model_def.state;
                // validate the active state
                validate_state(block.id.as_str(), active_state, &block.states)?;

                // TODO: exhaustiveness checking

                // set the model handle
                let model_str = format!("model/{}.model.ron", model_def.model.clone());
                model_def.model_handle = load_context.load(AssetPath::parse(model_str.as_str()));
            }


            Ok(block)
        })
    }

    fn extensions(&self) -> &[&str] {
        &["block.ron"]
    }
}

fn validate_state(id: &str, state: &BTreeMap<String, String>, state_def: &Vec<BlockStateAsset>) -> Result<(), AssetLoaderError> {
    
    for (k, v) in state.iter() {
        match get_state(k, state_def) {
            None => {
                let str = format!("State {} does not exist for block {}", k, id);
                return Err(InvalidRon(str));
            }
            Some(def) => {
                if !def.values.contains(v) {
                    let str = format!("Value {} does not exist for state {} in block {}", v, k, id);
                    return Err(InvalidRon(str));
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Asset, TypePath, Deserialize)]
#[serde(rename="BlockModel")]
pub struct BlockModelAsset {
    pub parent: Option<String>,
    #[serde(skip)]
    pub parent_handle: Option<Handle<BlockModelAsset>>,
    #[serde(default)]
    pub faces: Vec<BlockModelFace>,
    #[serde(default)]
    pub full_sides: Vec<Direction>,
    #[serde(default)]
    pub textures: BTreeMap<String, String>,
    #[serde(skip)]
    pub texture_handles: BTreeMap<String, Handle<Image>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename="Face")]
pub struct BlockModelFace {
    #[serde(rename="type")]
    pub face_type: FaceType,
    pub vertices: Vec<ModelVertex>,
    pub normal: Vec3,
    pub texture: String,
    pub cull_mode: Option<Direction>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename="Vertex")]
pub struct ModelVertex {
    pub pos: Vec3,
    pub uv: Vec2,
}

#[derive(Debug, Hash, Clone, PartialEq, Deserialize)]
pub enum FaceType {
    Quad,
    Triangle
}







#[derive(Default)]
pub struct BlockModelLoader;
impl AssetLoader for BlockModelLoader {
    type Asset = BlockModelAsset;
    type Settings = ();
    type Error = AssetLoaderError;

    fn load(&self, reader: &mut dyn Reader, settings: &Self::Settings, load_context: &mut LoadContext) -> impl ConditionalSendFuture<Output=std::result::Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let mut model = ron::de::from_bytes::<BlockModelAsset>(&bytes)?;

            
            // get the model handle from the model path
            // let texture_str = format!("texture/{}.png", model.texture.clone());
            // model.texture_handle = load_context.load(AssetPath::parse(texture_str.as_str()));

            // setup parent.
            if let Some(parent) = &model.parent {
                let parent_str = format!("model/{}.model.ron", parent);
                model.parent_handle = Some(load_context.load(AssetPath::parse(parent_str.as_str())));
            }
            
            // setup texture map
            for (k, v) in model.textures.iter() {
                let texture_str = format!("texture/{}.ktx2", v.clone());
                model.texture_handles.insert(k.clone(), load_context.load(AssetPath::parse(texture_str.as_str())));
            }
            

            Ok(model)
        })
    }

    fn extensions(&self) -> &[&str] {
        &["model.ron"]
    }
}