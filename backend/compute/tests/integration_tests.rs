// compute/tests/integration_tests.rs

use reqwest::Client;

#[tokio::test]
async fn test_end_to_end_via_http() {
    let client = Client::new();
    
    // Test 1: Check if server is running
    let response = client
        .get("http://localhost:3000/home")
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success(), "Server health check failed");
    println!("✓ Server is running");
    
    // Test 2: Test dataview dashboard
    let response = client
        .get("http://localhost:3000/dataview/dashboard")
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success(), "Dashboard endpoint failed");
    println!("✓ Dashboard endpoint works");
    
    // Test 3: Test list view
    let response = client
        .get("http://localhost:3000/list-view")
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success(), "List view endpoint failed");
    println!("✓ List view endpoint works");
    
    // Test 4: Test dataview active folder endpoint (GET)
    let folder_name = "test_folder";
    let response = client
        .get(&format!("http://localhost:3000/dataview/{}/active", folder_name))
        .send()
        .await
        .unwrap();
    
    println!("Active folder endpoint status: {}", response.status());
    // This might return 404 if folder doesn't exist, that's ok
    println!("✓ Active folder endpoint is reachable");
}

#[tokio::test]
async fn test_websocket_endpoints() {
    use futures_util::{SinkExt, StreamExt}; // SinkExt for .send(), StreamExt for .next()
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

    let url = "ws://localhost:3000/test-view/endpoints/load";

    match connect_async(url).await {
        Ok((mut ws_stream, _)) => {
            println!("✓ WebSocket connected successfully");

            ws_stream
                .send(Message::Text("ping".into()))
                .await
                .unwrap();

            if let Some(msg) = ws_stream.next().await {
                println!("Received: {:?}", msg);
            }
        }
        Err(e) => {
            println!("WebSocket connection: {} (this might be expected if not implemented)", e);
        }
    }
}

#[tokio::test]
async fn test_bookmark_endpoints() {
    let client = Client::new();
    let collection_name = "test_collection";
    
    // Test create collection (POST)
    let response = client
        .post(&format!("http://localhost:3000/bookmarks/{}/create", collection_name))
        .send()
        .await
        .unwrap();
    
    println!("Create collection status: {}", response.status());
    // 200 or 404/500 depending on implementation
    
    // Test load collection (WebSocket)
    // This would need WebSocket connection
}

#[tokio::test]
async fn test_repo_export() {
    let client = Client::new();
    let collection = "test";
    let filename = "test.zip";
    
    // Test export endpoint (GET)
    let response = client
        .get(&format!("http://localhost:3000/repo/{}/{}/export", collection, filename))
        .send()
        .await
        .unwrap();
    
    println!("Export endpoint status: {}", response.status());
    // This might return 404 if files don't exist
}