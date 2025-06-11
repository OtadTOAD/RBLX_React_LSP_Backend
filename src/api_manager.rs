use crate::api_parser::{cache_file, download_api, get_cache, parse_api_dump, ParsedInstance};
use std::collections::HashMap;

#[derive(Debug)]
pub struct ApiManager {
    instances: Option<HashMap<String, ParsedInstance>>,
    names: Option<Vec<String>>,
    freq_lookup: HashMap<String, usize>,
}

impl ApiManager {
    pub fn new() -> Self {
        Self {
            instances: None,
            names: None,
            freq_lookup: HashMap::new(),
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
    pub async fn load_api(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    fn build_word_freq(doc: &str) -> HashMap<String, usize> {
        let mut freq = HashMap::new();
        for word in doc.split(|c: char| !c.is_alphabetic()) {
            if !word.is_empty() {
                *freq.entry(word.to_string()).or_insert(0) += 1;
            }
        }
        freq
    }

    pub fn update_freq(&mut self, doc: &str, multiplier: usize) {
        let word_freq = Self::build_word_freq(doc);
        let look_up = &mut self.freq_lookup;

        if let Some(instance_list) = self.instances.as_ref() {
            for (name, inst) in instance_list {
                *look_up.entry(name.clone()).or_insert(0) +=
                    multiplier * (*word_freq.get(name).unwrap_or(&0));

                for property in &inst.properties {
                    let prop_name = &property.name;
                    *look_up.entry(prop_name.clone()).or_insert(0) +=
                        multiplier * (*word_freq.get(prop_name).unwrap_or(&0));
                }
            }
        }
    }

    pub fn lookup_properties(&self, inst_name: &str) -> Option<Vec<(String, String)>> {
        let instances = self.instances.as_ref()?;
        let instance = instances.get(inst_name)?;

        let mut props: Vec<(String, String)> = instance
            .properties
            .iter()
            .map(|p| (p.name.clone(), p.data_type.clone()))
            .collect();

        props.sort_by(|a, b| {
            let freq_a = self.freq_lookup.get(&a.0).copied().unwrap_or(0);
            let freq_b = self.freq_lookup.get(&b.0).copied().unwrap_or(0);
            freq_b
                .cmp(&freq_a) // First by freq
                .then_with(|| b.0.len().cmp(&a.0.len())) // Then by length(Longer text is annoying to type)
                .then_with(|| a.0.cmp(&b.0)) // Then by lex as tie breaker
        });

        Some(props)
    }

    pub fn get_all_inst(&self, index: &str) -> Option<Vec<String>> {
        self.names.as_ref().map(|names| {
            let mut filtered: Vec<String> = names
                .iter()
                .filter(|name| self.is_subsequence(index, name))
                .cloned()
                .collect();

            filtered.sort_by(|a, b| {
                let freq_a = self.freq_lookup.get(a).copied().unwrap_or(0);
                let freq_b = self.freq_lookup.get(b).copied().unwrap_or(0);
                freq_b
                    .cmp(&freq_a) // First by freq
                    .then_with(|| b.len().cmp(&a.len())) // Then by length(Longer text is annoying to type)
                    .then_with(|| a.cmp(b)) // Then by lex as tie breaker
            });

            filtered
        })
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
}
