use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Tell Cargo to rerun this build script if any files in maps/ change
    println!("cargo:rerun-if-changed=maps/");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("map_registry.rs");

    let mut map_entries = Vec::new();

    // Scan the maps directory
    if let Ok(entries) = fs::read_dir("maps") {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(extension) = path.extension() {
                    if extension == "json" {
                        if let Some(filename) = path.file_stem() {
                            if let Some(filename_str) = filename.to_str() {
                                // Read the JSON file to extract metadata
                                if let Ok(content) = fs::read_to_string(&path) {
                                    if let Ok(metadata) = extract_map_metadata(&content) {
                                        map_entries.push((filename_str.to_string(), metadata));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by name for consistent ordering
    map_entries.sort_by(|a, b| a.1.name.cmp(&b.1.name));

    // Generate the code
    let mut code = String::from("// Auto-generated map registry\n\n");

    // Generate the MapType enum
    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n");
    code.push_str("pub enum MapType {\n");
    for (filename, _) in &map_entries {
        let variant_name = filename_to_variant_name(filename);
        code.push_str(&format!("    {},\n", variant_name));
    }
    code.push_str("}\n\n");

    // Generate the impl block
    code.push_str("impl MapType {\n");
    code.push_str("    pub fn all() -> Vec<Self> {\n");
    code.push_str("        vec![\n");
    for (filename, _) in &map_entries {
        let variant_name = filename_to_variant_name(filename);
        code.push_str(&format!("            MapType::{},\n", variant_name));
    }
    code.push_str("        ]\n");
    code.push_str("    }\n\n");

    code.push_str("    pub fn name(&self) -> &'static str {\n");
    code.push_str("        match self {\n");
    for (filename, metadata) in &map_entries {
        let variant_name = filename_to_variant_name(filename);
        code.push_str(&format!("            MapType::{} => \"{}\",\n", variant_name, metadata.name));
    }
    code.push_str("        }\n");
    code.push_str("    }\n\n");

    code.push_str("    pub fn description(&self) -> &'static str {\n");
    code.push_str("        match self {\n");
    for (filename, metadata) in &map_entries {
        let variant_name = filename_to_variant_name(filename);
        code.push_str(&format!("            MapType::{} => \"{}\",\n", variant_name, metadata.description));
    }
    code.push_str("        }\n");
    code.push_str("    }\n\n");

    code.push_str("    pub fn create_map(&self, _width: usize, _height: usize) -> Result<crate::map::GridMap, Box<dyn std::error::Error>> {\n");
    code.push_str("        let json_content = match self {\n");
    for (filename, _) in &map_entries {
        let variant_name = filename_to_variant_name(filename);
        code.push_str(&format!("            MapType::{} => include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/maps/{}.json\")),\n", variant_name, filename));
    }
    code.push_str("        };\n");
    code.push_str("        let map: crate::map::GridMap = serde_json::from_str(json_content)?;\n");
    code.push_str("        Ok(map)\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    // Write the generated code
    fs::write(&dest_path, code).unwrap();
}

#[derive(Debug)]
struct MapMetadata {
    name: String,
    description: String,
}

fn extract_map_metadata(json_content: &str) -> Result<MapMetadata, Box<dyn std::error::Error>> {
    let value: serde_json::Value = serde_json::from_str(json_content)?;
    let name = value.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Map")
        .to_string();
    let description = value.get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(MapMetadata { name, description })
}

fn filename_to_variant_name(filename: &str) -> String {
    // Convert filename to PascalCase for enum variant
    let mut result = String::new();
    let mut capitalize_next = true;

    for ch in filename.chars() {
        if ch == '_' || ch == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    result
}
