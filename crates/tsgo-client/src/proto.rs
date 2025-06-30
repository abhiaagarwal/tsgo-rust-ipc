use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub options: Value,
    pub file_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponse {
    pub id: String,
    pub config_file_name: String,
    pub compiler_options: Value,
    pub root_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolResponse {
    pub id: String,
    pub name: String,
    pub flags: u32,
    pub check_flags: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeResponse {
    pub id: String,
    pub flags: u32,
}
