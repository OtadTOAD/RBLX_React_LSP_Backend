use crate::api_parser::{cache_file, download_api, get_cache, parse_api_dump, ParsedInstance};
use std::collections::HashMap;

#[derive(Debug)]
pub struct ApiManager {
    instances: Option<HashMap<String, ParsedInstance>>,
}

impl ApiManager {
    pub fn new() -> Self {
        Self { instances: None }
    }

    // This downloads and caches new api file, which then gets loaded
    pub async fn download_api(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let download_result = download_api().await?;
        let parsed_instances = parse_api_dump(&download_result);

        cache_file(&parsed_instances)?;
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

        Ok(())
    }

    pub fn lookup_inst(&self, name: &str) -> Option<&ParsedInstance> {
        if self.instances.is_none() {
            return None;
        }

        self.instances.as_ref()?.get(name)
    }
}
