use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

pub struct EndpointRecord {
    pub eid: u64,
    pub eid_string: String,
    pub endpoint_type: String,
    pub request_type: String,
    pub annotation: String,
    pub tags: String,
    pub req_count: u32,
    pub res_count: u32,
}

pub fn create_markdown<I>(
    repo_path: &Path,
    per_md_file: usize,
    endpoints: I,
) -> Result<(), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = EndpointRecord>,
{
    if repo_path.exists() {
        fs::remove_dir_all(repo_path)?;
    }
    fs::create_dir_all(repo_path)?;

    let mut file_index = 1;
    let mut counter = 0;

    let mut writer = create_new_file(repo_path, file_index)?;

    for endpoint in endpoints {
        if counter > 0 && counter % per_md_file == 0 {
            writer.flush()?;
            file_index += 1;
            writer = create_new_file(repo_path, file_index)?;
        }

        writeln!(
            writer,
            "| {} | {} | {} | {} | {} | {} | {}/{} |",
            endpoint.eid,
            endpoint.eid_string,
            endpoint.endpoint_type,
            endpoint.request_type,
            endpoint.annotation,
            endpoint.tags,
            endpoint.req_count,
            endpoint.res_count
        )?;

        counter += 1;
    }

    writer.flush()?;
    Ok(())
}



fn create_new_file(
    repo_path: &Path,
    index: usize,
) -> Result<BufWriter<File>, Box<dyn std::error::Error>> {
    let filename = format!("endpoints-{:03}.md", index);
    let file = File::create(repo_path.join(filename))?;
    let mut writer = BufWriter::new(file);

    writeln!(
        writer,
        "| EID | EID String | Type | Request Type | Annotation | Tags | Req/Res |"
    )?;
    writeln!(
        writer,
        "|-----|------------|------|--------------|------------|------|---------|"
    )?;

    Ok(writer)
}
