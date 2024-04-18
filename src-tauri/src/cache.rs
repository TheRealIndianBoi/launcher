use std::collections::HashMap;

use log::error;
use serde::{Deserialize, Serialize};

use crate::util::network::download_json;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
  #[error("{0}")]
  ModSource(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModVersion {
  pub version: String,
  pub display_name: String,
  pub description: String,
  pub authors: Vec<String>,
  pub tags: Vec<String>,
  pub supported_games: Vec<String>, // map to SupportedMap
  pub published_date: String,
  pub assets: HashMap<String, String>,
  pub website_url: Option<String>,
  pub cover_art_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModSourceData {
  pub schema_version: String,
  pub source_name: String,
  pub last_updated: String,
  pub mods: HashMap<String, Vec<ModVersion>>,
  pub texture_packs: HashMap<String, ModVersion>,
}

pub struct LauncherCache {
  pub mod_sources: HashMap<String, ModSourceData>,
}

impl LauncherCache {
  pub fn default() -> Self {
    Self {
      mod_sources: HashMap::new(),
    }
  }

  pub async fn refresh_mod_sources(&mut self, sources: Vec<String>) -> Result<(), CacheError> {
    self.mod_sources.clear();
    for source in sources {
      let source_json = download_json(&source).await;
      match source_json {
        Ok(json) => match serde_json::from_str(&json) {
          Ok(json_value) => {
            let source_data: ModSourceData = json_value;
            self.mod_sources.insert(source, source_data);
          }
          Err(err) => error!("Unable to convert {json} to typed value: {err:?}"),
        },
        Err(err) => {
          error!("Unable to download json from {source}: {err:?}")
        }
      }
    }
    Ok(())
  }
}
