// This script handles scraping roblox API and generating look up table

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

/*
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
}*/

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParsedInstance {
    pub instance: String,
    pub superclass: String,
    pub properties: Vec<ParsedProperty>,
    pub events: Vec<ParsedProperty>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParsedProperty {
    pub name: String,
    pub data_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedApi {
    pub version: String,
    pub instances: ParsedInstances,
}

fn get_cache_file_path() -> PathBuf {
    let exe_path = env::current_exe().expect("Failed to get current exe path!");
    let exe_dir = exe_path.parent().expect("Failed to get exe dir!");
    exe_dir.join("serialized_api.bin")
}

pub fn get_cache() -> Result<Option<CachedApi>, Box<dyn std::error::Error + Send + Sync>> {
    let api_cache_path = get_cache_file_path();
    if !api_cache_path.exists() {
        return Ok(None);
    }

    let bytes = fs::read(&api_cache_path)?;

    // Try new format
    if let Ok(cache) = bincode::deserialize::<CachedApi>(&bytes) {
        return Ok(Some(cache));
    }

    // Fall back to old format (raw ParsedInstances) — treat version as unknown
    // so it will always prompt the user to update once, then save in new format
    if let Ok(instances) = bincode::deserialize::<ParsedInstances>(&bytes) {
        return Ok(Some(CachedApi {
            version: "unknown".to_string(),
            instances,
        }));
    }

    // Probably missing
    Ok(None)
}

pub fn cache_file(
    parsed_instances: &ParsedInstances,
    version: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let api_cache_path = get_cache_file_path();
    let cache = CachedApi {
        version: version.to_string(),
        instances: parsed_instances.clone(),
    };
    let encoded = bincode::serialize(&cache)?;
    let mut file = File::create(api_cache_path)?;
    file.write_all(&encoded)?;
    Ok(())
}

pub async fn create_api_file_readable(
    path: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file_path = path.join("readable_serialized_api.json");
    let mut file = fs::File::create(file_path)?;

    let (dump, _version) = download_api_with_version().await?;
    let processed_result = parse_api_dump(&dump)?;

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
                .filter(|m| {
                    (m.member_type == "Property" || m.member_type == "Event")
                        && !m.tags.contains(&"Deprecated".to_string())
                        && !m.tags.contains(&"ReadOnly".to_string())
                })
                .collect();
            if let Some(parent_inst) = inst_cache.get(top.superclass.as_str()) {
                inst_members.extend(parent_inst);
            }

            let (props, events): (Vec<&Member>, Vec<&Member>) = inst_members
                .clone()
                .into_iter()
                .partition(|m| m.member_type == "Property");

            let properties: Vec<ParsedProperty> = props
                .into_iter()
                .map(|member| ParsedProperty {
                    name: member.name.clone(),
                    data_type: member.value_type.name.clone(),
                })
                .collect();
            let events: Vec<ParsedProperty> = events
                .into_iter()
                .map(|member| ParsedProperty {
                    name: member.name.clone(),
                    data_type: "Function".to_string(),
                })
                .collect();

            inst_cache.insert(top.name.as_str(), inst_members); // You need to cache before parsing properties otherwise it will throw error as you are moving references
            parsed_instances.insert(
                top.name.clone(),
                ParsedInstance {
                    instance: top.name.clone(),
                    superclass: top.superclass.clone(),
                    properties,
                    events,
                },
            );
        }
    }

    parsed_instances
}

pub fn parse_api_dump(api_dump: &str) -> Result<ParsedInstances, serde_json::Error> {
    let api_dump_json: ApiDump = serde_json::from_str(api_dump)?;
    Ok(process_api_dump_json(&api_dump_json))
}

pub async fn get_live_version() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let version_url = "https://clientsettingscdn.roblox.com/v1/client-version/WindowsStudio64";
    let version_json: serde_json::Value = reqwest::get(version_url).await?.json().await?;
    Ok(version_json["clientVersionUpload"]
        .as_str()
        .ok_or("Failed to parse clientVersionUpload from response")?
        .to_string())
}

pub async fn download_api_with_version(
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    match download_api_from_clientsettings().await {
        Ok(result) => return Ok(result),
        Err(e) => eprintln!(
            "Primary API source failed ({}), falling back to QTStudio...",
            e
        ),
    }

    // Fallback
    let version = reqwest::get("https://setup.rbxcdn.com/versionQTStudio")
        .await?
        .text()
        .await?;
    let version = version.trim().to_string();
    let dump = reqwest::get(format!(
        "https://setup.rbxcdn.com/{}-API-Dump.json",
        version
    ))
    .await?
    .text()
    .await?;
    Ok((dump, version))
}

async fn download_api_from_clientsettings(
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let version_url = "https://clientsettingscdn.roblox.com/v1/client-version/WindowsStudio64";
    let version_json: serde_json::Value = reqwest::get(version_url).await?.json().await?;

    let version = version_json["clientVersionUpload"]
        .as_str()
        .ok_or("Failed to parse clientVersionUpload from response")?
        .to_string();

    let api_dump_url = format!("https://setup.rbxcdn.com/{}-API-Dump.json", version);
    let dump = reqwest::get(&api_dump_url).await?.text().await?;
    Ok((dump, version))
}

#[cfg(test)]
mod tests {
    use crate::api_parser::{
        cache_file, download_api_with_version, get_live_version, parse_api_dump, CachedApi,
        ParsedInstances,
    };
    use std::{env, fs, path::Path};

    // Download without needing version
    pub async fn download_api() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let (dump, _) = download_api_with_version().await?;
        Ok(dump)
    }

    fn temp_dir() -> std::path::PathBuf {
        let dir = env::temp_dir().join("rblx_react_lsp_tests");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn test_downloading_api() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let download_result = download_api().await?;
        assert!(!download_result.is_empty(), "API dump should not be empty");
        Ok(())
    }

    #[tokio::test]
    async fn test_processing_with_cache() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (dump, version) = download_api_with_version().await?;
        let parsed_instances = parse_api_dump(&dump)?;

        let cache = CachedApi {
            version: version.clone(),
            instances: parsed_instances.clone(),
        };

        let cache_path = temp_dir().join("serialized_api.bin");
        let encoded = bincode::serialize(&cache)?;
        fs::write(&cache_path, &encoded)?;

        let read_back = fs::read(&cache_path)?;
        let decoded: CachedApi = bincode::deserialize(&read_back)?;
        assert!(!decoded.instances.is_empty(), "Cache should have instances");
        assert_eq!(
            decoded.version, version,
            "Version should round-trip correctly"
        );

        fs::remove_file(&cache_path).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_backwards_compat_cache() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let dump = download_api().await?;
        let parsed_instances = parse_api_dump(&dump)?;

        // Write in old format (raw ParsedInstances, no version)
        let cache_path = temp_dir().join("serialized_api_old.bin");
        let encoded = bincode::serialize(&parsed_instances)?;
        fs::write(&cache_path, &encoded)?;

        // Read it back using the old format directly to simulate what get_cache does
        let read_back = fs::read(&cache_path)?;
        let decoded: ParsedInstances = bincode::deserialize(&read_back)?;
        assert!(
            !decoded.is_empty(),
            "Old format cache should still be readable"
        );

        fs::remove_file(&cache_path).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_version_fetch() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let version = get_live_version().await?;
        assert!(!version.is_empty(), "Version string should not be empty");
        println!("Live version: {}", version);
        Ok(())
    }

    // Run with: cargo test test_generate_bundled_cache -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn test_generate_bundled_cache() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let out_dir = Path::new("bundled").to_path_buf();
        fs::create_dir_all(&out_dir)?;

        println!("Downloading API dump...");
        let (dump, version) = download_api_with_version().await?;
        println!("Version: {}", version);

        let parsed_instances = parse_api_dump(&dump)?;
        cache_file(&parsed_instances, &version)?;

        let out_path = out_dir.join("serialized_api.bin");
        let cache = CachedApi {
            version: version.clone(),
            instances: parsed_instances.clone(),
        };
        let encoded = bincode::serialize(&cache)?;
        fs::write(&out_path, &encoded)?;

        println!("Bundled cache written to: {}", out_path.display());
        println!("Instance count: {}", parsed_instances.len());
        Ok(())
    }
}
