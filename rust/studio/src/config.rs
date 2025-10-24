// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! Handles One ROM ROM metadata and image JSON format config files

pub const CONFIG_SITE_BASE: &str = "images.onerom.org";
pub const CONFIG_MANIFEST: &str = "configs.json";

/// Configs structure representing available configuration files
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Configs {
    pub version: usize,
    pub configs: Vec<String>,
    #[serde(skip)]
    pub names: Option<Vec<String>>,
}

impl Configs {
    /// Create a new Configs instance
    pub fn from_json(json: String) -> Result<Self, String> {
        let mut configs: Configs = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse Configs JSON:\n  - {e}"))?;
        
        // Create names (required by pick list) and sort alphabetically
        let mut names = configs.configs.iter()
            .filter_map(|c| {
                let file_name = c.split('/').last()?.split('.').next()?;
                Some(file_name.to_string())
            })
            .collect::<Vec<_>>();
        names.sort();
        configs.names = Some(names);

        Ok(configs)
    }
}

impl std::fmt::Display for Configs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Configs({})",
            self.configs.len(),
        )
    }
}

impl Configs {
    /// Create Configs from network manifest
    pub async fn from_network_async() -> Result<Self, String> {
        let url = Self::manifest_url();
        let response = reqwest::get(&url)
            .await
            .map_err(|e| format!("Network error fetching Configs manifest:\n  - {e}"))?;
        let text = response
            .text()
            .await
            .map_err(|e| format!("Network error reading Configs manifest:\n  - {e}"))?;
        Self::from_json(text)
    }

    /// Return names of the configs
    pub fn names(&self) -> &Vec<String> {
        // The config string is path/to/name.json.
        // We want to extract the name without the path and extension.
        &self.names.as_ref().unwrap()
    }

    /// Return names of the configs as a single string with commas
    pub fn names_str(&self) -> String {
        self.names().join(", ")
    }

    /// Return path for config of a given name
    pub fn path(&self, name: &str) -> Option<String> {
        for c in &self.configs {
            let file_name = c.split('/').last()?.split('.').next()?;
            if file_name == name {
                return Some(c.clone());
            }
        }
        None
    }

    pub fn config_url(&self, name: &str) -> Option<String> {
        let path = self.path(name)?;
        Some(format!("https://{}/{}", CONFIG_SITE_BASE, path))
    }

    /// Return configs manifest URL
    pub fn manifest_url() -> String {
        format!("https://{}/{}", CONFIG_SITE_BASE, CONFIG_MANIFEST)
    }
}


pub async fn get_config_from_url(url: &String) -> Result<Vec<u8>, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Network error fetching Config:\n  - {e}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Network error reading Config:\n  - {e}"))?;
    Ok(bytes.to_vec())
}