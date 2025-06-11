// This script handles scraping roblox API and generating look up table

use bincode::config::standard;
use bincode::{decode_from_std_read, encode_into_std_write, Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs};

type ParsedInstances = HashMap<String, ParsedInstance>;

#[derive(Deserialize, Debug)]
pub struct ApiDump {
    #[serde(rename = "Classes")]
    pub classes: Vec<Instance>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Instance {
    #[serde(default, rename = "Members")]
    pub members: Vec<Member>,
    #[serde(default, rename = "MemoryCategory")]
    pub memory_category: String,
    #[serde(default, rename = "Name")]
    pub name: String,
    #[serde(default, rename = "Superclass")]
    pub superclass: String,
    #[serde(default, rename = "Tags")]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Member {
    #[serde(default, rename = "Category")]
    pub category: String, // Member category (e.g., "Behavior")
    #[serde(default, rename = "MemberType")]
    pub member_type: String, // Member type (e.g., "Property")
    #[serde(default, rename = "Name")]
    pub name: String, // Member name (e.g., "Archivable")
    #[serde(default, rename = "Tags")]
    pub tags: Vec<String>,
    #[serde(default, rename = "ValueType")]
    pub value_type: ValueType, // Value type (e.g., {"Category": "Primitive", "Name": "bool"})
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Security {
    #[serde(default, rename = "Read")]
    pub read: String, // Security level for read access
    #[serde(default, rename = "Write")]
    pub write: String, // Security level for write access
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Serialization {
    #[serde(default, rename = "CanLoad")]
    pub can_load: bool, // Whether the member can be loaded
    #[serde(default, rename = "CanSave")]
    pub can_save: bool, // Whether the member can be saved
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ValueType {
    #[serde(default, rename = "Category")]
    pub category: String, // Category of value (e.g., "Primitive")
    #[serde(default, rename = "Name")]
    pub name: String, // Name of value type (e.g., "bool")
}
impl Default for ValueType {
    fn default() -> Self {
        Self {
            category: "".to_string(),
            name: "".to_string(),
        }
    }
}

#[derive(Encode, Decode, Serialize, Deserialize, Debug, Clone)]
pub struct ParsedInstance {
    pub instance: String,
    pub superclass: String,
    pub properties: Vec<ParsedProperty>,
}

#[derive(Encode, Decode, Serialize, Deserialize, Debug, Clone)]
pub struct ParsedProperty {
    pub name: String,
    pub data_type: String,
}

fn get_cache_file_path() -> PathBuf {
    let exe_path = env::current_exe().expect("Failed to get current exe path!");
    let exe_dir = exe_path.parent().expect("Failed to get exe dir!");
    exe_dir.join("serialized_api.bin")
}

pub fn get_cache() -> Result<Option<ParsedInstances>, Box<dyn std::error::Error + Send + Sync>> {
    let api_cache_path = get_cache_file_path();
    if api_cache_path.exists() {
        let mut file = File::open(&api_cache_path)?;
        let (parsed_api, _bytes_read): (ParsedInstances, usize) =
            decode_from_std_read(&mut file, standard())?;
        Ok(Some(parsed_api))
    } else {
        Ok(None)
    }
}

pub fn cache_file(parsed_instances: &ParsedInstances) -> Result<(), Box<dyn std::error::Error>> {
    let api_cache_path = get_cache_file_path();
    let mut file = fs::File::create(api_cache_path)?;
    encode_into_std_write(parsed_instances, &mut file, standard())?;
    file.flush()?;
    Ok(())
}

pub async fn create_api_file_readable(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = path.join("readable_serialized_api.json");
    let mut file = fs::File::create(file_path)?;

    let download_result = download_api().await?;
    let processed_result = parse_api_dump(&download_result);
    let json_string = serde_json::to_string_pretty(&processed_result)?;
    file.write_all(json_string.as_bytes())?;
    file.flush()?;

    Ok(())
}

fn process_api_dump_json(api_dump_json: &ApiDump) -> ParsedInstances {
    let mut inst_cache = HashMap::new();
    let mut inst_look_up = HashMap::new();

    for instance in &api_dump_json.classes {
        inst_look_up.insert(instance.name.as_str(), instance);
    }

    let mut parsed_instances = HashMap::new();
    let mut parsing_stack = Vec::new();

    for (&name, &inst) in &inst_look_up {
        if inst_cache.contains_key(name) {
            // Because we put parents in stack and process them too, we need to skip already processed instances
            continue;
        }
        parsing_stack.push(inst);

        // Basically since Rust kinda makes it cancer to do recursion, I just check all instances that need to be parsed to parse current instance
        let mut parent_name = inst.superclass.as_str();
        while !parent_name.is_empty()
            && parent_name != "<ROOT>"
            && !inst_cache.contains_key(parent_name)
        {
            if let Some(parent_inst) = inst_look_up.get(parent_name) {
                parsing_stack.push(parent_inst);
                parent_name = parent_inst.superclass.as_str();
            } else {
                break;
            }
        }

        while let Some(top) = parsing_stack.pop() {
            let mut inst_members: Vec<&Member> = top
                .members
                .iter()
                .filter(|member| {
                    member.member_type == "Property"
                        && !member.tags.contains(&"Deprecated".to_string())
                        && !member.tags.contains(&"ReadOnly".to_string())
                })
                .collect();
            if let Some(parent_inst) = inst_cache.get(top.superclass.as_str()) {
                inst_members.extend(parent_inst)
            }

            let properties: Vec<ParsedProperty> = inst_members
                .iter()
                .map(|member| ParsedProperty {
                    name: member.name.clone(),
                    data_type: member.value_type.name.clone(),
                })
                .collect();
            inst_cache.insert(top.name.as_str(), inst_members); // You need to cache before parsing properties otherwise it will throw error as you are moving references
            parsed_instances.insert(
                top.name.clone(),
                ParsedInstance {
                    instance: top.name.clone(),
                    superclass: top.superclass.clone(),
                    properties,
                },
            );
        }
    }

    parsed_instances
}

pub fn parse_api_dump(api_dump: &str) -> ParsedInstances {
    let api_dump_json: ApiDump =
        serde_json::from_str(&api_dump).expect("Failed to serialize JSON!");
    process_api_dump_json(&api_dump_json)
}

pub async fn download_api() -> Result<String, reqwest::Error> {
    let req_version_url = "https://setup.rbxcdn.com/versionQTStudio";
    let version_result = reqwest::get(req_version_url).await?.text().await?;

    let api_dump_url = format!(
        "https://setup.rbxcdn.com/{}-API-Dump.json",
        version_result.trim()
    );
    let api_dump_data = reqwest::get(&api_dump_url).await?.text().await?;
    Ok(api_dump_data)
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write, path::Path};

    use crate::api_parser::{cache_file, download_api, parse_api_dump};

    #[tokio::test]
    async fn test_downloading_api() -> Result<(), Box<dyn std::error::Error>> {
        let dump_path = "api_dump.json";

        let download_result = download_api().await?;
        let mut file = fs::File::create(dump_path)?;
        file.write_all(download_result.as_bytes())?;
        file.flush()?;

        Ok(())
    }

    #[tokio::test]
    async fn test_processing_with_cache() -> Result<(), Box<dyn std::error::Error>> {
        let dump_path = "api_dump.json";
        if !Path::new(dump_path).exists() {
            return Err("Failed to find api_dump.json!".into());
        }

        let api_dump_cache_content = fs::read_to_string(dump_path)?;
        let parsed_instances = parse_api_dump(&api_dump_cache_content);
        cache_file(&parsed_instances)?;

        Ok(())
    }
}
