use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs as sync_fs;
use std::path::Path;
use std::sync::Mutex;
use tokio::fs;
use tokio::task;

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
    //#[serde(rename = "Security")]
    //pub security: Security, // Security details for read and write access
    //#[serde(rename = "Serialization")]
    //pub serialization: Serialization, // Serialization details
    //#[serde(default, rename = "ThreadSafety")]
    //pub thread_safety: String, // Thread safety information (e.g., "ReadSafe")
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

#[derive(Serialize, Debug, Clone)]
pub struct ParsedInstance {
    instance: String,
    superclass: String,
    properties: Vec<ParsedProperty>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ParsedProperty {
    pub name: String,
    pub data_type: String,
}

async fn get_cached_api_dump(cache_path: &str) -> Result<Option<String>, std::io::Error> {
    if Path::new(cache_path).exists() {
        let contents = fs::read_to_string(cache_path).await?;
        Ok(Some(contents))
    } else {
        Ok(None)
    }
}

async fn download_api_dump() -> Result<String, reqwest::Error> {
    let ver_url = "https://setup.rbxcdn.com/versionQTStudio";
    let ver = reqwest::get(ver_url).await?.text().await?;
    let ver = ver.trim();

    let dump_url = format!("https://setup.rbxcdn.com/{}-API-Dump.json", ver);
    let dump_data = reqwest::get(&dump_url).await?.text().await?;
    Ok(dump_data)
}

fn build_class_map<'a>(dump: &'a ApiDump) -> HashMap<&'a str, &'a Instance> {
    let mut map = HashMap::new();
    for instance in &dump.classes {
        map.insert(instance.name.as_str(), instance);
    }
    map
}

fn get_all_property_members<'a>(
    class: &'a Instance,
    class_map: &HashMap<&'a str, &'a Instance>,
) -> Vec<&'a Member> {
    let mut members: Vec<&Member> = class
        .members
        .iter()
        .filter(|member| member.member_type == "Property")
        .collect();
    if !class.superclass.is_empty() && class.superclass != "<ROOT>" {
        if let Some(super_instance) = class_map.get(class.superclass.as_str()) {
            members.extend(get_all_property_members(super_instance, class_map));
        }
    }
    members
}

fn extract_parsed_info(dump: &ApiDump) -> Vec<ParsedInstance> {
    let class_map = build_class_map(dump);
    let mut parsed_instances = Vec::new();
    for class in &dump.classes {
        // Collect properties from the class and its superclasses.
        let members = get_all_property_members(class, &class_map);
        let properties: Vec<ParsedProperty> = members
            .into_iter()
            .map(|member| {
                // Clone the data type name from value_type
                let data_type = member.value_type.name.clone();
                ParsedProperty {
                    name: member.name.clone(),
                    data_type,
                }
            })
            .collect();
        parsed_instances.push(ParsedInstance {
            instance: class.name.clone(),
            superclass: class.superclass.clone(),
            properties,
        });
    }
    parsed_instances
}

fn save_parsed_api_dump(
    parsed: &[ParsedInstance],
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let serialized = serde_json::to_string_pretty(parsed)?;
    let mut file = sync_fs::File::create(output_path)?;
    use std::io::Write;
    file.write_all(serialized.as_bytes())?;
    Ok(())
}

pub async fn generate_api_metadata(output_path: &str) {
    let cache_path = "api_dump.json";

    let json_str = fs::read_to_string(cache_path)
        .await
        .expect("Failed to read the cache file");
    let dump: ApiDump = serde_json::from_str(&json_str).expect("Failed to deserialize JSON");

    let parsed_info = extract_parsed_info(&dump);
    let output_path = output_path.to_string(); // Clone for thread safety

    task::spawn_blocking(move || {
        save_parsed_api_dump(&parsed_info, &output_path).expect("Failed to save parsed API dump");
    })
    .await
    .expect("Failed to execute blocking task");
}

/// Global cache for parsed metadata.
static API_METADATA_CACHE: Lazy<Mutex<Option<Vec<ParsedInstance>>>> =
    Lazy::new(|| Mutex::new(None));

/// Initialize the global metadata cache by parsing the API dump file.
pub fn init_metadata() -> Result<(), Box<dyn Error>> {
    // Read the cached API dump (synchronously for simplicity).
    let json_str = sync_fs::read_to_string("api_dump.json")?;
    let dump: ApiDump = serde_json::from_str(&json_str)?;
    let parsed_info = extract_parsed_info(&dump);
    let mut cache = API_METADATA_CACHE.lock().unwrap();
    *cache = Some(parsed_info);
    Ok(())
}

/// Retrieve metadata (all properties including inherited ones) for a given instance name
/// from the global cache.
pub fn get_metadata(instance_name: &str) -> Option<Vec<ParsedProperty>> {
    let cache = API_METADATA_CACHE.lock().unwrap();
    if let Some(ref metadata) = *cache {
        // Build a lookup map from instance name to ParsedInstance.
        let class_map: HashMap<&str, &ParsedInstance> = metadata
            .iter()
            .map(|pi| (pi.instance.as_str(), pi))
            .collect();
        // Recursively collect properties from the class and its superclasses.
        fn collect_properties<'a>(
            name: &str,
            map: &HashMap<&'a str, &'a ParsedInstance>,
        ) -> Vec<ParsedProperty> {
            if let Some(pi) = map.get(name) {
                let mut props = pi.properties.clone();
                if !pi.superclass.is_empty() && pi.superclass != "<ROOT>" {
                    props.extend(collect_properties(&pi.superclass, map));
                }
                props
            } else {
                Vec::new()
            }
        }
        let props = collect_properties(instance_name, &class_map);
        if props.is_empty() {
            None
        } else {
            Some(props)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_dump() {
        let output_path = "test_api_dump.json";
        if Path::new(output_path).exists() {
            fs::remove_file(output_path)
                .await
                .expect("Failed to remove preexisting file!");
        }
        generate_api_metadata(output_path).await;
    }
}
