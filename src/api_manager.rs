use crate::api_parser::{cache_file, download_api, get_cache, parse_api_dump, ParsedInstance};
use std::collections::HashMap;

#[derive(Debug)]
pub struct ApiManager {
    instances: Option<HashMap<String, ParsedInstance>>,
    names: Option<Vec<String>>,
}

impl ApiManager {
    pub fn new() -> Self {
        Self {
            instances: None,
            names: None,
        }
    }

    // This downloads and caches new api file, which then gets loaded
    pub async fn download_api(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let download_result = download_api().await?;
        let parsed_instances = parse_api_dump(&download_result);

        cache_file(&parsed_instances)?;
        self.names = Some(parsed_instances.keys().cloned().collect());
        self.instances = Some(parsed_instances);

        Ok(())
    }

    // This loads api from cached file
    pub async fn load_api(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let cache_result = get_cache()?;
        if cache_result.is_none() {
            return Err("Failed to load api from cache!".into());
        }

        self.instances = cache_result;
        self.names = self
            .instances
            .as_ref()
            .map(|map| map.keys().cloned().collect());

        Ok(())
    }

    pub fn lookup_inst(&self, name: &str) -> Option<&ParsedInstance> {
        self.instances.as_ref()?.get(name)
    }

    fn is_subsequence(&self, pattern: &str, text: &str) -> bool {
        let pattern_lower = pattern.to_lowercase();
        let mut pattern_chars = pattern_lower.chars();
        let mut current_char = pattern_chars.next();

        for c in text.to_lowercase().chars() {
            if Some(c) == current_char {
                current_char = pattern_chars.next();
                if current_char.is_none() {
                    return true;
                }
            }
        }
        current_char.is_none()
    }

    pub fn get_all_inst(&self, index: &str) -> Option<Vec<String>> {
        self.names.as_ref().map(|names| {
            names
                .iter()
                .filter(|name| self.is_subsequence(index, name))
                .cloned()
                .collect()
        })
    }
}
