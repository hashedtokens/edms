use crate::error::{EdmsError, EdmsResult};
use serde_json::Value;

pub fn is_valid_json(input: &str) -> EdmsResult<Value> {
    serde_json::from_str(input).map_err(|e| EdmsError::InvalidJson(e.to_string()))
}

pub fn check_json_format(input: &str) -> i32 {
    match is_valid_json(input) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

pub fn is_valid_json_file(file_path: &str) -> EdmsResult<Value> {
    let contents = std::fs::read_to_string(file_path)?;
    is_valid_json(&contents)
}

pub fn check_json_file_format(file_path: &str) -> i32 {
    match is_valid_json_file(file_path) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}
