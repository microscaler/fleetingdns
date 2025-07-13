use backendapi;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Run the API server
    backendapi::run().await?;
    
    Ok(())
}
