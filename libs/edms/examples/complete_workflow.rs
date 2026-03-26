// cargo run --example complete_workflow

use edms::ops::{endpoint_ops::EndpointOps, request_ops::RequestOps};
use edms::ops::{response_ops::ResponseOps, tag_ops::TagOps};
use edms::schema::initialize_schema_from_core;
use edms::EdmsResult;

fn main() -> EdmsResult<()> {
    let _ = std::fs::remove_file("example_workflow.db");

    println!("1. Initializing operations...");
    let endpoint_ops = EndpointOps::new("example_workflow.db");
    let request_ops = RequestOps::new("example_workflow.db");
    let response_ops = ResponseOps::new("example_workflow.db");
    let tag_ops = TagOps::new("example_workflow.db");

    endpoint_ops.initialize()?;
    request_ops.initialize()?;
    response_ops.initialize()?;
    tag_ops.initialize()?;
    initialize_schema_from_core(&endpoint_ops.core)?;
    println!("[PASS] All operations initialized\n");

    println!("2. Creating endpoints...");
    endpoint_ops.insert("E001", "/api/v1/users", Some("User CRUD operations"))?;
    endpoint_ops.insert("E002", "/api/v1/auth/login", Some("User authentication"))?;
    endpoint_ops.insert("E003", "/api/v1/posts", Some("Blog post management"))?;
    println!("[PASS] Created 3 endpoints\n");

    println!("3. Tagging endpoints...");
    tag_ops.add("E001", "rest")?;
    tag_ops.add("E001", "v1")?;
    tag_ops.add("E001", "crud")?;
    tag_ops.add("E001", "users")?;

    tag_ops.add("E002", "rest")?;
    tag_ops.add("E002", "v1")?;
    tag_ops.add("E002", "auth")?;

    tag_ops.add("E003", "rest")?;
    tag_ops.add("E003", "v1")?;
    tag_ops.add("E003", "crud")?;
    println!("[PASS] Tags assigned\n");

    println!("4. Tracking requests...");
    request_ops.insert("E001", 1, "E001/request-001.json", "GET")?;
    request_ops.insert("E001", 2, "E001/request-002.json", "POST")?;
    request_ops.insert("E001", 3, "E001/request-003.json", "PUT")?;
    request_ops.insert("E001", 4, "E001/request-004.json", "DELETE")?;

    request_ops.insert("E002", 1, "E002/request-001.json", "POST")?;
    request_ops.insert("E002", 2, "E002/request-002.json", "POST")?;

    request_ops.insert("E003", 1, "E003/request-001.json", "GET")?;
    println!("[PASS] 7 requests tracked\n");

    println!("5. Tracking responses...");
    response_ops.insert("E001", 1, "E001/response-001.json", 200, Some(150))?;
    response_ops.insert("E001", 2, "E001/response-002.json", 201, Some(200))?;
    response_ops.insert("E001", 3, "E001/response-003.json", 200, Some(180))?;
    response_ops.insert("E001", 4, "E001/response-004.json", 204, Some(120))?;

    response_ops.insert("E002", 1, "E002/response-001.json", 200, Some(100))?;
    response_ops.insert("E002", 2, "E002/response-002.json", 401, Some(50))?;

    response_ops.insert("E003", 1, "E003/response-001.json", 500, Some(1000))?;
    println!("[PASS] 7 responses tracked\n");

    println!("6. Request Analytics:");
    let e001_req_count = request_ops.count("E001")?;
    println!("   - E001 requests: {}", e001_req_count);

    let get_requests = request_ops.get_by_method("GET")?;
    println!("   - GET requests: {}", get_requests.len());

    let post_requests = request_ops.get_by_method("POST")?;
    println!("   - POST requests: {}", post_requests.len());

    let next_num = request_ops.get_next_number("E001")?;
    println!("   - Next request number for E001: {}\n", next_num);

    println!("7. Response Analytics:");
    let avg_time = response_ops.avg_response_time("E001")?;
    println!(
        "   - E001 average response time: {:.2}ms",
        avg_time.unwrap_or(0.0)
    );

    let errors = response_ops.get_all_errors()?;
    println!("   - Total errors (status >= 400): {}", errors.len());

    let error_counts = response_ops.count_errors_by_endpoint()?;
    println!("   - Errors by endpoint:");
    for (endpoint_id, count) in error_counts {
        println!("     • {}: {} error(s)", endpoint_id, count);
    }
    println!();

    println!("8. Tag Analytics:");
    let popular_tags = tag_ops.get_popular_tags()?;
    println!("   - Popular tags:");
    for (tag, count) in popular_tags.iter().take(5) {
        println!("     • {}: {} endpoint(s)", tag, count);
    }

    let rest_endpoints = tag_ops.get_endpoints_by_tag("rest")?;
    println!("   - REST endpoints: {:?}", rest_endpoints);

    let v1_endpoints = tag_ops.get_endpoints_by_tag("v1")?;
    println!("   - V1 endpoints: {:?}\n", v1_endpoints);

    println!("9. Summary:");
    let total_endpoints = endpoint_ops.count()?;
    println!("   - Total endpoints: {}", total_endpoints);

    let all_endpoints = endpoint_ops.get_all()?;
    for (id, path, _) in all_endpoints {
        let req_count = request_ops.count(&id)?;
        let res_count = response_ops.count(&id)?;
        let tag_count = tag_ops.count(&id)?;
        println!(
            "   - {}: {} ({} requests, {} responses, {} tags)",
            id, path, req_count, res_count, tag_count
        );
    }
    println!();

    println!("10. Shutting down...");
    endpoint_ops.shutdown()?;
    request_ops.shutdown()?;
    response_ops.shutdown()?;
    tag_ops.shutdown()?;
    println!("[PASS] All operations closed\n");

    println!("Database saved as: example_workflow.db");

    Ok(())
}
