// cargo run --example tag_search

use edms::ops::endpoint_ops::EndpointOps;
use edms::ops::tag_ops::TagOps;
use edms::schema::initialize_schema_from_core;
use edms::EdmsResult;

fn main() -> EdmsResult<()> {
    let _ = std::fs::remove_file("example_tags.db");
    let endpoint_ops = EndpointOps::new("example_tags.db");
    let tag_ops = TagOps::new("example_tags.db");

    endpoint_ops.initialize()?;
    tag_ops.initialize()?;
    initialize_schema_from_core(&endpoint_ops.core)?;

    println!("1. Creating diverse API endpoints...\n");

    endpoint_ops.insert("E001", "/api/v1/users", Some("User management"))?;
    endpoint_ops.insert("E002", "/api/v1/posts", Some("Blog posts"))?;
    endpoint_ops.insert("E003", "/api/v1/comments", Some("Post comments"))?;

    endpoint_ops.insert("E004", "/graphql", Some("GraphQL endpoint"))?;
    endpoint_ops.insert("E005", "/graphql/playground", Some("GraphQL UI"))?;
    endpoint_ops.insert("E006", "/ws/chat", Some("Chat websocket"))?;
    endpoint_ops.insert("E007", "/ws/notifications", Some("Notification stream"))?;
    endpoint_ops.insert("E008", "/auth/login", Some("User login"))?;
    endpoint_ops.insert("E009", "/auth/register", Some("User registration"))?;
    endpoint_ops.insert("E010", "/auth/refresh", Some("Token refresh"))?;

    println!("[PASS] Created 10 endpoints\n");

    println!("2. Tagging endpoints by characteristics...\n");

    tag_ops.add("E001", "rest")?;
    tag_ops.add("E001", "v1")?;
    tag_ops.add("E001", "crud")?;
    tag_ops.add("E001", "users")?;
    tag_ops.add("E001", "public")?;

    tag_ops.add("E002", "rest")?;
    tag_ops.add("E002", "v1")?;
    tag_ops.add("E002", "crud")?;
    tag_ops.add("E002", "blog")?;
    tag_ops.add("E002", "public")?;

    tag_ops.add("E003", "rest")?;
    tag_ops.add("E003", "v1")?;
    tag_ops.add("E003", "crud")?;
    tag_ops.add("E003", "blog")?;
    tag_ops.add("E003", "public")?;

    tag_ops.add("E004", "graphql")?;
    tag_ops.add("E004", "api")?;
    tag_ops.add("E004", "public")?;

    tag_ops.add("E005", "graphql")?;
    tag_ops.add("E005", "dev")?;
    tag_ops.add("E005", "tools")?;

    tag_ops.add("E006", "websocket")?;
    tag_ops.add("E006", "realtime")?;
    tag_ops.add("E006", "chat")?;
    tag_ops.add("E006", "public")?;

    tag_ops.add("E007", "websocket")?;
    tag_ops.add("E007", "realtime")?;
    tag_ops.add("E007", "notifications")?;
    tag_ops.add("E007", "private")?;

    tag_ops.add("E008", "auth")?;
    tag_ops.add("E008", "security")?;
    tag_ops.add("E008", "public")?;

    tag_ops.add("E009", "auth")?;
    tag_ops.add("E009", "security")?;
    tag_ops.add("E009", "public")?;

    tag_ops.add("E010", "auth")?;
    tag_ops.add("E010", "security")?;
    tag_ops.add("E010", "private")?;

    println!("[PASS] Tags assigned\n");

    println!("3. Searching by Protocol:");
    let rest_endpoints = tag_ops.get_endpoints_by_tag("rest")?;
    println!("   REST endpoints ({}):", rest_endpoints.len());
    for id in &rest_endpoints {
        let info = endpoint_ops.get_by_id(id)?;
        if let Some((_, path, _)) = info.first() {
            println!("     - {}: {}", id, path);
        }
    }

    let graphql_endpoints = tag_ops.get_endpoints_by_tag("graphql")?;
    println!("\n   GraphQL endpoints ({}):", graphql_endpoints.len());
    for id in &graphql_endpoints {
        let info = endpoint_ops.get_by_id(id)?;
        if let Some((_, path, _)) = info.first() {
            println!("     - {}: {}", id, path);
        }
    }

    let ws_endpoints = tag_ops.get_endpoints_by_tag("websocket")?;
    println!("\n   WebSocket endpoints ({}):", ws_endpoints.len());
    for id in &ws_endpoints {
        let info = endpoint_ops.get_by_id(id)?;
        if let Some((_, path, _)) = info.first() {
            println!("     - {}: {}", id, path);
        }
    }
    println!();

    println!("4. Searching by Access Level:");
    let public_endpoints = tag_ops.get_endpoints_by_tag("public")?;
    println!("   Public endpoints ({}):", public_endpoints.len());
    for id in &public_endpoints {
        let info = endpoint_ops.get_by_id(id)?;
        if let Some((_, path, _)) = info.first() {
            println!("     - {}", path);
        }
    }

    let private_endpoints = tag_ops.get_endpoints_by_tag("private")?;
    println!("\n   Private endpoints ({}):", private_endpoints.len());
    for id in &private_endpoints {
        let info = endpoint_ops.get_by_id(id)?;
        if let Some((_, path, _)) = info.first() {
            println!("     - {}", path);
        }
    }
    println!();

    println!("5. Searching by Feature:");
    let auth_endpoints = tag_ops.get_endpoints_by_tag("auth")?;
    println!("   Authentication endpoints ({}):", auth_endpoints.len());
    for id in &auth_endpoints {
        let info = endpoint_ops.get_by_id(id)?;
        if let Some((_, path, annot)) = info.first() {
            println!("     - {}: {}", path, annot.as_deref().unwrap_or(""));
        }
    }

    let crud_endpoints = tag_ops.get_endpoints_by_tag("crud")?;
    println!("\n   CRUD endpoints ({}):", crud_endpoints.len());
    for id in &crud_endpoints {
        let info = endpoint_ops.get_by_id(id)?;
        if let Some((_, path, _)) = info.first() {
            println!("     - {}", path);
        }
    }
    println!();

    println!("6. Popular Tags Analysis:");
    let popular = tag_ops.get_popular_tags()?;
    println!("   ┌────────────────┬───────────┐");
    println!("   │ Tag            │ Endpoints │");
    println!("   ├────────────────┼───────────┤");
    for (tag, count) in &popular {
        println!("   │ {:14} │ {:9} │", tag, count);
    }
    println!("   └────────────────┴───────────┘\n");

    println!("7. Tag Distribution per Endpoint:");
    let all_endpoints = endpoint_ops.get_all()?;
    for (id, path, _) in all_endpoints {
        let tags = tag_ops.get_by_endpoint(&id)?;
        println!("   {} ({} tags): {:?}", path, tags.len(), tags);
    }
    println!();

    println!("8. Advanced Search - Multi-tag Filter:");
    println!("   Finding endpoints with both 'rest' AND 'crud'...");

    let rest = tag_ops.get_endpoints_by_tag("rest")?;
    let crud = tag_ops.get_endpoints_by_tag("crud")?;
    let rest_and_crud: Vec<_> = rest.iter().filter(|id| crud.contains(id)).collect();

    println!("   Found {} endpoints:", rest_and_crud.len());
    for id in rest_and_crud {
        let info = endpoint_ops.get_by_id(id)?;
        if let Some((_, path, _)) = info.first() {
            let tags = tag_ops.get_by_endpoint(id)?;
            println!("     - {}: {:?}", path, tags);
        }
    }
    println!();

    println!("9. Tag Management Operations:");
    println!("   Adding 'production' tag to auth endpoints...");
    for id in &auth_endpoints {
        tag_ops.add(id, "production")?;
    }
    println!("[PASS] Tag added to {} endpoints", auth_endpoints.len());

    let tag_count = tag_ops.count("E001")?;
    println!("   E001 has {} tags", tag_count);

    println!("   Removing 'dev' tag from E005...");
    tag_ops.remove("E005", "dev")?;
    let e005_tags = tag_ops.get_by_endpoint("E005")?;
    println!("[PASS] E005 now has tags: {:?}", e005_tags);
    println!();

    println!("10. Summary:");
    println!("   Total endpoints: {}", endpoint_ops.count()?);
    let all_tags = tag_ops.get_popular_tags()?;
    println!("   Unique tags: {}", all_tags.len());

    let total_tag_count: i32 = all_tags.iter().map(|(_, count)| count).sum();
    println!("   Total tag assignments: {}", total_tag_count);

    let avg_tags = total_tag_count as f64 / endpoint_ops.count()? as f64;
    println!("   Average tags per endpoint: {:.1}", avg_tags);
    println!();

    endpoint_ops.shutdown()?;
    tag_ops.shutdown()?;

    println!("Database saved as: example_tags.db");

    Ok(())
}
