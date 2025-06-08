use crate::api_parser::{get_cache, ParsedInstance};
use std::collections::HashMap;

pub struct ApiManager {
    instances: HashMap<String, ParsedInstance>,
}

impl ApiManager {
    fn new(parsed_instances: HashMap<String, ParsedInstance>) -> Self {
        Self {
            instances: parsed_instances,
        }
    }

    fn lookup_inst(&self, name: &str) -> Option<&ParsedInstance> {
        self.instances.get(name)
    }
}

pub fn init_api_manager() -> Result<ApiManager, Box<dyn std::error::Error>> {
    let api_cache = get_cache()?;
    let parsed_instances = api_cache
        .ok_or_else(|| -> Box<dyn std::error::Error> { "Failed to find api_dump.json!".into() })?;
    Ok(ApiManager::new(parsed_instances))
}
