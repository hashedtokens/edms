// cargo run --example analytics

use edms::ops::{endpoint_ops::EndpointOps, request_ops::RequestOps};
use edms::ops::{response_ops::ResponseOps, tag_ops::TagOps};
use edms::schema::initialize_schema_from_core;
use edms::EdmsResult;

fn main() -> EdmsResult<()> {
    let _ = std::fs::remove_file("example_analytics.db");
    let endpoint_ops = EndpointOps::new("example_analytics.db");
    let request_ops = RequestOps::new("example_analytics.db");
    let response_ops = ResponseOps::new("example_analytics.db");
    let tag_ops = TagOps::new("example_analytics.db");

    endpoint_ops.initialize()?;
    request_ops.initialize()?;
    response_ops.initialize()?;
    tag_ops.initialize()?;
    initialize_schema_from_core(&endpoint_ops.core)?;

    println!("1. Setting up test data...\n");

    endpoint_ops.insert("E001", "/api/users", Some("Fast endpoint"))?;
    endpoint_ops.insert("E002", "/api/search", Some("Slow endpoint"))?;
    endpoint_ops.insert("E003", "/api/process", Some("Unreliable endpoint"))?;

    tag_ops.initialize()?;
    tag_ops.add("E001", "fast")?;
    tag_ops.add("E001", "stable")?;
    tag_ops.add("E002", "slow")?;
    tag_ops.add("E003", "unstable")?;

    for i in 1..=10 {
        request_ops.insert("E001", i, &format!("E001/req-{:03}.json", i), "GET")?;
        let time = 50 + (i * 10); // 60-150ms
        response_ops.insert(
            "E001",
            i,
            &format!("E001/res-{:03}.json", i),
            200,
            Some(time),
        )?;
    }

    for i in 1..=8 {
        request_ops.insert("E002", i, &format!("E002/req-{:03}.json", i), "POST")?;
        let time = 800 + (i * 50); // 850-1150ms
        response_ops.insert(
            "E002",
            i,
            &format!("E002/res-{:03}.json", i),
            200,
            Some(time),
        )?;
    }

    let statuses = vec![200, 200, 500, 200, 404, 200, 500, 200, 403, 200, 500, 200];
    for (i, status) in statuses.iter().enumerate() {
        let i = i + 1;
        request_ops.insert("E003", i as i32, &format!("E003/req-{:03}.json", i), "POST")?;
        let time = if *status == 200 { 100 } else { 50 }; // Errors are faster
        response_ops.insert(
            "E003",
            i as i32,
            &format!("E003/res-{:03}.json", i),
            *status,
            Some(time),
        )?;
    }

    println!("[PASS] Created 3 endpoints with 30 requests/responses\n");

    println!("2. Response Time Analysis:");
    println!("   ┌─────────────┬──────────────┬──────────┐");
    println!("   │ Endpoint    │ Avg Time (ms)│ Requests │");
    println!("   ├─────────────┼──────────────┼──────────┤");

    let endpoints = endpoint_ops.get_all()?;
    for (id, path, _) in endpoints {
        let avg = response_ops.avg_response_time(&id)?;
        let count = request_ops.count(&id)?;
        println!(
            "   │ {:11} │ {:12.2} │ {:8} │",
            path,
            avg.unwrap_or(0.0),
            count
        );
    }
    println!("   └─────────────┴──────────────┴──────────┘\n");

    println!("3. Error Analysis:");
    println!("   ┌─────────────┬──────────┬──────────┬────────────┐");
    println!("   │ Endpoint    │ Total    │ Errors   │ Error Rate │");
    println!("   ├─────────────┼──────────┼──────────┼────────────┤");

    let error_counts = response_ops.count_errors_by_endpoint()?;
    let error_map: std::collections::HashMap<_, _> = error_counts.into_iter().collect();

    for (id, path, _) in endpoint_ops.get_all()? {
        let total = response_ops.count(&id)?;
        let errors = error_map.get(&id).copied().unwrap_or(0);
        let rate = if total > 0 {
            (errors as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "   │ {:11} │ {:8} │ {:8} │ {:9.1}% │",
            path, total, errors, rate
        );
    }
    println!("   └─────────────┴──────────┴──────────┴────────────┘\n");

    println!("4. Error Details:");
    let all_errors = response_ops.get_all_errors()?;
    let mut status_counts: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();

    for (_, _, status) in &all_errors {
        *status_counts.entry(*status).or_insert(0) += 1;
    }

    println!("   Total errors: {}", all_errors.len());
    println!("   Breakdown by status code:");
    let mut status_vec: Vec<_> = status_counts.iter().collect();
    status_vec.sort_by_key(|(status, _)| *status);

    for (status, count) in status_vec {
        let msg = match status {
            403 => "Forbidden",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Unknown",
        };
        println!("     • {} ({}): {} occurrence(s)", status, msg, count);
    }
    println!();

    println!("5. Performance by Tag:");
    let popular_tags = tag_ops.get_popular_tags()?;

    for (tag, _) in popular_tags {
        let endpoints = tag_ops.get_endpoints_by_tag(&tag)?;
        if endpoints.is_empty() {
            continue;
        }

        let mut total_time = 0.0;
        let mut total_count = 0;
        let mut total_errors = 0;

        for endpoint_id in endpoints {
            if let Some(avg) = response_ops.avg_response_time(&endpoint_id)? {
                let count = response_ops.count(&endpoint_id)?;
                total_time += avg * count as f64;
                total_count += count;
            }
            let errors = error_map.get(&endpoint_id).copied().unwrap_or(0);
            total_errors += errors;
        }

        let avg_time = if total_count > 0 {
            total_time / total_count as f64
        } else {
            0.0
        };

        let error_rate = if total_count > 0 {
            (total_errors as f64 / total_count as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "   • Tag '{}': Avg {:.2}ms, Error rate {:.1}%",
            tag, avg_time, error_rate
        );
    }
    println!();

    println!("6. Recommendations:");
    let mut slowest = (String::new(), 0.0);
    for (id, _, _) in endpoint_ops.get_all()? {
        if let Some(avg) = response_ops.avg_response_time(&id)? {
            if avg > slowest.1 {
                slowest = (id.clone(), avg);
            }
        }
    }
    if slowest.1 > 500.0 {
        println!(
            "[FAIL] Endpoint {} is slow ({:.0}ms avg) - consider optimization",
            slowest.0, slowest.1
        );
    }

    let mut highest_error = (String::new(), 0, 0);
    for (id, _, _) in endpoint_ops.get_all()? {
        let total = response_ops.count(&id)?;
        let errors = error_map.get(&id).copied().unwrap_or(0);
        if total > 0 && errors > highest_error.2 {
            highest_error = (id.clone(), total, errors);
        }
    }
    if highest_error.2 > 0 {
        let rate = (highest_error.2 as f64 / highest_error.1 as f64) * 100.0;
        println!(
            "[FAIL] Endpoint {} has high error rate ({:.1}%) - needs investigation",
            highest_error.0, rate
        );
    }

    for (id, path, _) in endpoint_ops.get_all()? {
        let total = response_ops.count(&id)?;
        let errors = error_map.get(&id).copied().unwrap_or(0);
        if total > 0 && errors == 0 {
            println!("[PASS] Endpoint {} ({}) is stable - 0 errors", id, path);
        }
    }
    println!();

    endpoint_ops.shutdown()?;
    request_ops.shutdown()?;
    response_ops.shutdown()?;
    tag_ops.shutdown()?;

    println!("Database saved as: example_analytics.db");

    Ok(())
}
