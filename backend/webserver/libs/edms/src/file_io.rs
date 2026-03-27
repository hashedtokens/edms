use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;

const EDMS_DATA_DIR: &str = "edms_data";

fn get_endpoint_dir(endpoint_id: &str) -> String {
    format!("{EDMS_DATA_DIR}/{endpoint_id}")
}

fn write_json_file(file_path: &str, data: &Value) -> io::Result<()> {
    let json_str = serde_json::to_string_pretty(data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(file_path, json_str)?;
    Ok(())
}

fn read_json_file(file_path: &str) -> io::Result<Value> {
    let contents = fs::read_to_string(file_path)?;
    serde_json::from_str(&contents).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn write_request_json(
    endpoint_id: &str,
    request_number: i32,
    data: &Value,
) -> io::Result<String> {
    let dir = get_endpoint_dir(endpoint_id);
    fs::create_dir_all(&dir)?;
    let file_path = format!("{dir}/request-{request_number:03}.json");
    write_json_file(&file_path, data)?;
    Ok(file_path)
}

pub fn write_response_json(
    endpoint_id: &str,
    request_number: i32,
    data: &Value,
) -> io::Result<String> {
    let dir = get_endpoint_dir(endpoint_id);
    fs::create_dir_all(&dir)?;
    let file_path = format!("{dir}/response-{request_number:03}.json");
    write_json_file(&file_path, data)?;
    Ok(file_path)
}

pub fn read_request_json(file_path: &str) -> io::Result<Value> {
    read_json_file(file_path)
}

pub fn read_response_json(file_path: &str) -> io::Result<Value> {
    read_json_file(file_path)
}

pub fn get_next_request_number(endpoint_id: &str) -> io::Result<i32> {
    let dir = get_endpoint_dir(endpoint_id);
    if !Path::new(&dir).exists() {
        return Ok(1);
    }
    let mut max_num = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_str = file_name.to_string_lossy();

        if file_str.starts_with("request-") && file_str.ends_with(".json") {
            if let Some(num_str) = file_str
                .strip_prefix("request-")
                .and_then(|s| s.strip_suffix(".json"))
            {
                if let Ok(num) = num_str.parse::<i32>() {
                    max_num = max_num.max(num);
                }
            }
        }
    }

    Ok(max_num + 1)
}

pub fn create_endpoint_dir(endpoint_id: &str) -> io::Result<()> {
    fs::create_dir_all(get_endpoint_dir(endpoint_id))?;
    Ok(())
}

pub fn endpoint_dir_exists(endpoint_id: &str) -> bool {
    Path::new(&get_endpoint_dir(endpoint_id)).exists()
}
pub fn list_endpoint_files(endpoint_id: &str) -> io::Result<Vec<String>> {
    let dir = get_endpoint_dir(endpoint_id);
    let mut files = Vec::new();

    if !Path::new(&dir).exists() {
        return Ok(files);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        files.push(file_name.to_string_lossy().to_string());
    }

    files.sort();
    Ok(files)
}
pub fn delete_endpoint_files(endpoint_id: &str) -> io::Result<()> {
    let dir = get_endpoint_dir(endpoint_id);
    if Path::new(&dir).exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}
