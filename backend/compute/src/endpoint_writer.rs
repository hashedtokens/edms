use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub fn write_endpoint_file<P: AsRef<Path>>(
    repo_path: P,
    eid: &str,
    page_index: usize,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let filename = format!("{}-{:03}.md", eid, page_index);
    let file = File::create(repo_path.as_ref().join(filename))?;
    let mut writer = BufWriter::new(file);
    writer.write_all(content.as_bytes())?;
    writer.flush()?;
    Ok(())
}

pub fn write_request_file<P: AsRef<Path>>(
    repo_path: P,
    eid: &str,
    req_index: usize,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let filename = format!("{}-{}-request.md", eid, req_index);
    let file = File::create(repo_path.as_ref().join(filename))?;
    let mut writer = BufWriter::new(file);
    writer.write_all(content.as_bytes())?;
    writer.flush()?;
    Ok(())
}

pub fn write_response_file<P: AsRef<Path>>(
    repo_path: P,
    eid: &str,
    res_index: usize,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let filename = format!("{}-{}-response.md", eid, res_index);
    let file = File::create(repo_path.as_ref().join(filename))?;
    let mut writer = BufWriter::new(file);
    writer.write_all(content.as_bytes())?;
    writer.flush()?;
    Ok(())
}
