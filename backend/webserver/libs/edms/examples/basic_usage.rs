// cargo run --example basic_usage

use edms::ops::endpoint_ops::EndpointOps;
use edms::ops::tag_ops::TagOps;
use edms::schema::initialize_schema_from_core;
use edms::EdmsResult;

fn main() -> EdmsResult<()> {
    let _ = std::fs::remove_file("example_basic.db");

    println!("1. Initializing EndpointOps...");
    let endpoint_ops = EndpointOps::new("example_basic.db");
    endpoint_ops.initialize()?;
    initialize_schema_from_core(&endpoint_ops.core)?;
    println!("[PASS] Database initialized\n");

    println!("2. Adding endpoints...");
    endpoint_ops.insert("E001", "/api/users", Some("User management API"))?;
    endpoint_ops.insert("E002", "/api/posts", Some("Post management API"))?;
    endpoint_ops.insert("E003", "/api/comments", None)?;
    println!("[PASS] Added 3 endpoints\n");

    println!("3. Querying endpoints...");
    let all_endpoints = endpoint_ops.get_all()?;
    for (id, path, annotation) in all_endpoints {
        let annot = annotation.unwrap_or_else(|| "No annotation".to_string());
        println!("   - {}: {} ({})", id, path, annot);
    }
    let count = endpoint_ops.count()?;
    println!("   Total: {} endpoints\n", count);

    println!("4. Adding tags...");
    let tag_ops = TagOps::new("example_basic.db");
    tag_ops.initialize()?;

    tag_ops.add("E001", "rest")?;
    tag_ops.add("E001", "v1")?;
    tag_ops.add("E001", "crud")?;

    tag_ops.add("E002", "rest")?;
    tag_ops.add("E002", "v1")?;

    tag_ops.add("E003", "rest")?;
    println!("[PASS] Tags added\n");

    println!("5. Querying tags...");
    let e001_tags = tag_ops.get_by_endpoint("E001")?;
    println!("   E001 tags: {:?}", e001_tags);

    let rest_endpoints = tag_ops.get_endpoints_by_tag("rest")?;
    println!("   Endpoints with 'rest' tag: {:?}", rest_endpoints);

    let popular = tag_ops.get_popular_tags()?;
    println!("   Popular tags:");
    for (tag, count) in popular {
        println!("     - {}: {} endpoint(s)", tag, count);
    }
    println!();

    println!("6. Updating endpoint annotation...");
    endpoint_ops.update_annotation("Updated: User API with auth", "E001")?;
    let updated = endpoint_ops.get_by_id("E001")?;
    if let Some((_, _, annotation)) = updated.first() {
        println!("[PASS] New annotation: {:?}\n", annotation);
    }

    println!("7. Shutting down...");
    endpoint_ops.shutdown()?;
    tag_ops.shutdown()?;
    println!("[VALID] Done\n");
    println!("Database saved as: example_basic.db");

    Ok(())
}
