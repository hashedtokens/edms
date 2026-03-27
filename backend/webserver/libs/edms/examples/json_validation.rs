// cargo run --example json_validation

use edms::file_io::*;
use edms::json_validator::*;
use edms::sqlite_utils::*;
use edms::EdmsResult;
use serde_json::json;

fn main() -> EdmsResult<()> {
    let _ = std::fs::remove_dir_all("edms_data");
    let _ = std::fs::remove_file("example_validation.db");
    println!("1. JSON Format Validation:");
    println!("   Testing various JSON inputs...\n");
    let test_cases = vec![
        (r#"{"name": "John", "age": 30}"#, "Valid object"),
        (r#"[1, 2, 3, 4, 5]"#, "Valid array"),
        (
            r#"{"endpoint": "/api/users", "method": "GET"}"#,
            "Valid endpoint",
        ),
        (r#"{name: test}"#, "Invalid - missing quotes"),
        (r#"{"key": }"#, "Invalid - missing value"),
        ("not json at all", "Invalid - not JSON"),
        ("", "Invalid - empty string"),
    ];

    for (input, description) in test_cases {
        let code = check_json_format(input);
        let status = if code == 0 {
            "[PASS] VALID"
        } else {
            "[FAIL] INVALID"
        };
        println!("   {} | {} | Code: {}", status, description, code);
    }
    println!();
    println!("2. JSON Parsing with Result:");
    let valid_json = r#"{"endpoint": "/api/users", "version": "v1"}"#;
    match is_valid_json(valid_json) {
        Ok(value) => {
            println!("[PASS] Parsed successfully:");
            println!("     {}", value);
        }
        Err(e) => println!("   [FAIL] Error: {}", e),
    }
    println!();
    println!("3. File I/O Operations:");
    let request_data = json!({
        "method": "POST",
        "path": "/api/users",
        "headers": {
            "Content-Type": "application/json",
            "Authorization": "Bearer token123"
        },
        "body": {
            "name": "Alice",
            "email": "alice@example.com"
        }
    });
    println!("   Writing request JSON...");
    let request_path = write_request_json("E001", 1, &request_data)?;
    println!("[PASS] Saved to: {}", request_path);
    let response_data = json!({
        "status": 201,
        "message": "User created successfully",
        "data": {
            "id": "user_001",
            "name": "Alice",
            "email": "alice@example.com",
            "created_at": "2025-01-01T00:00:00Z"
        }
    });
    println!("   Writing response JSON...");
    let response_path = write_response_json("E001", 1, &response_data)?;
    println!("[PASS] Saved to: {}", response_path);
    println!("\n   Validating saved files...");
    let req_valid = check_json_file_format(&request_path);
    let res_valid = check_json_file_format(&response_path);
    println!(
        "   - Request file: {}",
        if req_valid == 0 { "✓" } else { "✗" }
    );
    println!(
        "   - Response file: {}",
        if res_valid == 0 { "✓" } else { "✗" }
    );
    println!("\n   Reading back files...");
    let read_request = read_request_json(&request_path)?;
    println!("[PASS] Request: {}", read_request["method"]);
    let read_response = read_response_json(&response_path)?;
    println!("[PASS] Response status: {}", read_response["status"]);
    println!();
    println!("4. Directory Operations:");
    for i in 2..=5 {
        let data = json!({"request_number": i, "method": "GET"});
        write_request_json("E001", i, &data)?;
    }
    println!("[PASS] Created 4 additional request files");
    let files = list_endpoint_files("E001")?;
    println!("   Files in E001 directory: {}", files.len());
    for file in &files {
        println!("     - {}", file);
    }
    let next_num = get_next_request_number("E001")?;
    println!("   Next request number: {}", next_num);
    let exists = endpoint_dir_exists("E001");
    println!("   E001 directory exists: {}", exists);
    let not_exists = endpoint_dir_exists("E999");
    println!("   E999 directory exists: {}", not_exists);
    println!();
    println!("5. SQLite Utility Functions:");
    println!("   Initializing database...");
    let init_code = initialize_sqlite_schema_code("example_validation.db");
    println!(
        "   - Schema initialization: {}",
        if init_code == 0 {
            "[PASS] Success"
        } else {
            "[FAIL] Failed"
        }
    );

    let exists_code = check_sqlite_exists_code("example_validation.db");
    println!(
        "   - Database exists: {}",
        if exists_code == 0 {
            "[PASS] Yes"
        } else {
            "[FAIL] No"
        }
    );

    let lock_code = check_sqlite_lock_code("example_validation.db");
    println!(
        "   - Database locked: {}",
        if lock_code == 0 {
            "[PASS] Not locked"
        } else {
            "[FAIL] Locked"
        }
    );

    let consistency_code = check_sqlite_consistency_code("example_validation.db");
    println!(
        "   - Database consistency: {}",
        if consistency_code == 0 {
            "[PASS] OK"
        } else {
            "[FAIL] Failed"
        }
    );

    let _ = std::fs::remove_file("nonexistent.db");
    let not_exists_code = check_sqlite_exists_code("nonexistent.db");
    println!(
        "   - Non-existent DB check: {}",
        if not_exists_code == -1 {
            "[PASS] Correctly returned -1"
        } else {
            "[FAIL] Unexpected result"
        }
    );
    println!();
    println!("6. Return Code Patterns Summary:");
    println!("   ┌──────────────────────────┬─────────────┐");
    println!("   │ Operation                │ Return Code │");
    println!("   ├──────────────────────────┼─────────────┤");
    println!("   │ Valid JSON               │      0      │");
    println!("   │ Invalid JSON             │     -1      │");
    println!("   │ Database exists          │      0      │");
    println!("   │ Database not found       │     -1      │");
    println!("   │ Database not locked      │      0      │");
    println!("   │ Database locked          │     -1      │");
    println!("   │ Schema init success      │      0      │");
    println!("   │ Schema init failed       │     -1      │");
    println!("   │ Query success            │      0      │");
    println!("   │ Query N failed           │   100+N     │");
    println!("   └──────────────────────────┴─────────────┘");
    println!();
    println!("7. Summary:");
    println!("[PASS] JSON validation: Working");
    println!("[PASS] File I/O: Working");
    println!("[PASS] Directory operations: Working");
    println!("[PASS] SQLite utilities: Working");
    println!("[PASS] Return codes: 0 (success), -1 (failure), 100+ (query error)");
    println!();
    println!("Files saved in: edms_data/E001/");
    println!("Database saved as: example_validation.db");

    Ok(())
}
