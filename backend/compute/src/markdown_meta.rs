use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub fn create_markdown_meta<P: AsRef<Path>>(
    repo_path: P,
    eids: impl Iterator<Item = String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(repo_path.as_ref().join("endpoint-data.md"))?;
    let mut writer = BufWriter::new(file);

    for eid in eids {
        writeln!(writer, "- {}", eid)?;
    }

    writer.flush()?;
    Ok(())
}
