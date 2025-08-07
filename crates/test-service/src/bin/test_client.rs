use reqwest;
use serde_json::{json, Value};
use tokio;

const BASE_URL: &str = "http://localhost:8001";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing FleetingDNS Rust Test Service...");
    println!("{}", "=".repeat(50));

    let client = reqwest::Client::new();
    let mut tests_passed = 0;
    let total_tests = 6;

    // Test 1: Health endpoint
    if test_health_endpoint(&client).await? {
        tests_passed += 1;
    }

    // Test 2: Public endpoint
    if test_public_endpoint(&client).await? {
        tests_passed += 1;
    }

    // Test 3: Status endpoint
    if test_status_endpoint(&client).await? {
        tests_passed += 1;
    }

    // Test 4: Login
    let (login_success, token) = test_login(&client).await?;
    if login_success {
        tests_passed += 1;
    }

    // Test 5: Authenticated hello (only if login succeeded)
    if login_success {
        if test_authenticated_hello(&client, &token).await? {
            tests_passed += 1;
        }
    }

    // Test 6: Logout (only if login succeeded)
    if login_success {
        if test_logout(&client, &token).await? {
            tests_passed += 1;
        }
    }

    println!("{}", "=".repeat(50));
    println!("📊 Test Results: {}/{} tests passed", tests_passed, total_tests);

    if tests_passed == total_tests {
        println!("🎉 All tests passed!");
        Ok(())
    } else {
        println!("❌ Some tests failed!");
        Err("Some tests failed".into())
    }
}

async fn test_health_endpoint(client: &reqwest::Client) -> Result<bool, Box<dyn std::error::Error>> {
    let response = client.get(&format!("{}/", BASE_URL)).send().await?;
    
    if response.status().is_success() {
        let data: Value = response.json().await?;
        println!("✅ Health check: {}", serde_json::to_string_pretty(&data)?);
        Ok(true)
    } else {
        println!("❌ Health check failed: {}", response.status());
        Ok(false)
    }
}

async fn test_public_endpoint(client: &reqwest::Client) -> Result<bool, Box<dyn std::error::Error>> {
    let response = client.get(&format!("{}/public", BASE_URL)).send().await?;
    
    if response.status().is_success() {
        let data: Value = response.json().await?;
        println!("✅ Public endpoint: {}", serde_json::to_string_pretty(&data)?);
        Ok(true)
    } else {
        println!("❌ Public endpoint failed: {}", response.status());
        Ok(false)
    }
}

async fn test_status_endpoint(client: &reqwest::Client) -> Result<bool, Box<dyn std::error::Error>> {
    let response = client.get(&format!("{}/status", BASE_URL)).send().await?;
    
    if response.status().is_success() {
        let data: Value = response.json().await?;
        println!("✅ Status endpoint: {}", serde_json::to_string_pretty(&data)?);
        Ok(true)
    } else {
        println!("❌ Status endpoint failed: {}", response.status());
        Ok(false)
    }
}

async fn test_login(client: &reqwest::Client) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let login_data = json!({
        "username": "testuser",
        "password": "testpass"
    });

    let response = client
        .post(&format!("{}/login", BASE_URL))
        .json(&login_data)
        .send()
        .await?;

    if response.status().is_success() {
        let data: Value = response.json().await?;
        println!("✅ Login successful: {}", serde_json::to_string_pretty(&data)?);
        
        let token = data["access_token"].as_str().unwrap_or("").to_string();
        Ok((true, token))
    } else {
        println!("❌ Login failed: {} - {}", response.status(), response.text().await?);
        Ok((false, String::new()))
    }
}

async fn test_authenticated_hello(
    client: &reqwest::Client,
    token: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/hello", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if response.status().is_success() {
        let data: Value = response.json().await?;
        println!("✅ Authenticated hello: {}", serde_json::to_string_pretty(&data)?);
        Ok(true)
    } else {
        println!(
            "❌ Authenticated hello failed: {} - {}",
            response.status(),
            response.text().await?
        );
        Ok(false)
    }
}

async fn test_logout(
    client: &reqwest::Client,
    token: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/logout", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if response.status().is_success() {
        let data: Value = response.json().await?;
        println!("✅ Logout successful: {}", serde_json::to_string_pretty(&data)?);
        Ok(true)
    } else {
        println!(
            "❌ Logout failed: {} - {}",
            response.status(),
            response.text().await?
        );
        Ok(false)
    }
} 