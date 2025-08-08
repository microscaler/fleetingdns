//! Comprehensive test suite for database entities
//! 
//! Uses test containers for each test to ensure isolation.

use sea_orm::*;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;
use chrono::Utc;

use crate::{
    entities::*,
    repository::*,
    ModelError, ModelResult,
};

/// Test database setup and teardown
pub struct TestDatabase {
    pub db: DatabaseConnection,
    _container: testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
}

impl TestDatabase {
    /// Initialize test database with migrations
    pub async fn new() -> ModelResult<Self> {
        let postgres_image = Postgres::default()
            .with_tag("17.5-alpine")
            .with_env_var("POSTGRES_DB", "fleetingdns_test")
            .with_env_var("POSTGRES_USER", "postgres")
            .with_env_var("POSTGRES_PASSWORD", "postgres");
        let container = postgres_image.start().await
            .map_err(|e| ModelError::DatabaseError(format!("Failed to start Postgres container: {}", e)))?;
        
        let host_port = container.get_host_port_ipv4(5432).await
            .map_err(|e| ModelError::DatabaseError(format!("Failed to get host port: {}", e)))?;
        let database_url = format!(
            "postgresql://postgres:postgres@localhost:{}/fleetingdns_test",
            host_port
        );
        
        // Connect to database with retry logic
        let mut retries = 30;
        let db = loop {
            match Database::connect(&database_url).await {
                Ok(db) => break db,
                Err(_) if retries > 0 => {
                    retries -= 1;
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                }
                Err(e) => return Err(ModelError::DatabaseError(format!("Failed to connect to database after retries: {}", e))),
            }
        };
        
        // Run migrations
        Self::run_migrations(&db).await?;
        
        Ok(Self { 
            db,
            _container: container,
        })
    }
    
    /// Run database migrations
    async fn run_migrations(db: &DatabaseConnection) -> ModelResult<()> {
        // Create tables based on our entities
        let sql = r#"
            -- Enable UUID extension
            CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
            
            -- Users table
            CREATE TABLE IF NOT EXISTS users (
                id VARCHAR(255) PRIMARY KEY,
                github_user_id VARCHAR(255) UNIQUE NOT NULL,
                login VARCHAR(255) NOT NULL,
                name VARCHAR(255),
                email VARCHAR(255),
                avatar_url TEXT,
                public_repos INTEGER,
                followers INTEGER,
                following INTEGER,
                created_at TIMESTAMPTZ,
                updated_at TIMESTAMPTZ,
                created_at_db TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
            );
            
            -- Service plans table
            CREATE TABLE IF NOT EXISTS service_plans (
                id VARCHAR(255) PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                api_rate_limit INTEGER NOT NULL,
                tunnel_creation_limit INTEGER NOT NULL,
                dns_provisioning_limit INTEGER NOT NULL,
                max_concurrent_tunnels INTEGER NOT NULL,
                features_json TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            -- User service plans table
            CREATE TABLE IF NOT EXISTS user_service_plans (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                user_id VARCHAR(255) REFERENCES users(id),
                service_plan_id VARCHAR(255) REFERENCES service_plans(id),
                start_date TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                end_date TIMESTAMPTZ,
                is_active BOOLEAN NOT NULL DEFAULT true,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            -- Auth tokens table
            CREATE TABLE IF NOT EXISTS auth_tokens (
                token VARCHAR(255) PRIMARY KEY,
                token_type VARCHAR(50) NOT NULL,
                expires_at TIMESTAMPTZ NOT NULL,
                user_id VARCHAR(255) REFERENCES users(id),
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            -- SSH key pairs table
            CREATE TABLE IF NOT EXISTS ssh_key_pairs (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                private_key TEXT NOT NULL,
                public_key TEXT NOT NULL,
                fingerprint VARCHAR(255) NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            -- Certificate info table
            CREATE TABLE IF NOT EXISTS certificate_info (
                serial VARCHAR(255) PRIMARY KEY,
                certificate TEXT NOT NULL,
                private_key TEXT NOT NULL,
                fingerprint VARCHAR(255) NOT NULL,
                issued_at TIMESTAMPTZ NOT NULL,
                expires_at TIMESTAMPTZ NOT NULL,
                subject VARCHAR(255) NOT NULL
            );
            
            -- Tunnels table
            CREATE TABLE IF NOT EXISTS tunnels (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                github_user_id VARCHAR(255) REFERENCES users(github_user_id),
                github_username VARCHAR(255) NOT NULL,
                subdomain VARCHAR(255) NOT NULL,
                fqdn VARCHAR(255) NOT NULL,
                local_port INTEGER NOT NULL,
                slot INTEGER NOT NULL,
                certificate_serial VARCHAR(255) REFERENCES certificate_info(serial),
                ssh_key_pair_id UUID REFERENCES ssh_key_pairs(id),
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                expires_at TIMESTAMPTZ NOT NULL,
                status VARCHAR(50) NOT NULL DEFAULT 'active',
                bytes_transferred BIGINT NOT NULL DEFAULT 0,
                request_count INTEGER NOT NULL DEFAULT 0
            );
            
            -- API stats table
            CREATE TABLE IF NOT EXISTS api_stats (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                active_tunnels INTEGER NOT NULL DEFAULT 0,
                tunnels_created_today INTEGER NOT NULL DEFAULT 0,
                bytes_transferred_today BIGINT NOT NULL DEFAULT 0,
                uptime_seconds BIGINT NOT NULL DEFAULT 0,
                ca_stats_id UUID,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            -- CA stats table
            CREATE TABLE IF NOT EXISTS ca_stats (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                certificates_issued INTEGER NOT NULL DEFAULT 0,
                active_certificates INTEGER NOT NULL DEFAULT 0,
                expired_certificates INTEGER NOT NULL DEFAULT 0,
                issuance_rate DOUBLE PRECISION NOT NULL DEFAULT 0.0,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            -- Payment info table
            CREATE TABLE IF NOT EXISTS payment_info (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                user_id VARCHAR(255) REFERENCES users(id),
                stripe_customer_id VARCHAR(255),
                stripe_subscription_id VARCHAR(255),
                last_payment_date TIMESTAMPTZ,
                next_payment_date TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            -- User usage table
            CREATE TABLE IF NOT EXISTS user_usage (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                user_id VARCHAR(255) REFERENCES users(id),
                period_start TIMESTAMPTZ NOT NULL,
                api_calls_count INTEGER NOT NULL DEFAULT 0,
                tunnels_created_count INTEGER NOT NULL DEFAULT 0,
                dns_operations_count INTEGER NOT NULL DEFAULT 0,
                active_tunnels_count INTEGER NOT NULL DEFAULT 0,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            -- Audit log table
            CREATE TABLE IF NOT EXISTS audit_log (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                user_id VARCHAR(255) REFERENCES users(id),
                action VARCHAR(255) NOT NULL,
                resource VARCHAR(255) NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                details_json TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            
            -- Billing events table
            CREATE TABLE IF NOT EXISTS billing_event (
                id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                user_id VARCHAR(255) REFERENCES users(id),
                service_plan_id VARCHAR(255) REFERENCES service_plans(id),
                event_type VARCHAR(255) NOT NULL,
                amount DOUBLE PRECISION NOT NULL,
                event_time TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                details_json TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
        "#;
        
        // Execute migration SQL
        db.execute_unprepared(sql).await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    

}

/// Test utilities for creating test data
pub struct TestData;

impl TestData {
    /// Create a test user
    pub fn create_test_user() -> user::ActiveModel {
        user::ActiveModel {
            id: Set("test-user-1".to_string()),
            github_user_id: Set("12345".to_string()),
            login: Set("testuser".to_string()),
            name: Set(Some("Test User".to_string())),
            email: Set(Some("test@example.com".to_string())),
            avatar_url: Set(Some("https://example.com/avatar.jpg".to_string())),
            public_repos: Set(Some(10)),
            followers: Set(Some(5)),
            following: Set(Some(3)),
            created_at: Set(Some(Utc::now())),
            updated_at: Set(Some(Utc::now())),
            created_at_db: Set(Utc::now()),
            ..Default::default()
        }
    }
    
    /// Create a test service plan
    pub fn create_test_service_plan() -> service_plan::ActiveModel {
        service_plan::ActiveModel {
            id: Set("basic".to_string()),
            name: Set("Basic Plan".to_string()),
            api_rate_limit: Set(1000),
            tunnel_creation_limit: Set(10),
            dns_provisioning_limit: Set(50),
            max_concurrent_tunnels: Set(5),
            features_json: Set(Some(r#"{"feature1": true, "feature2": false}"#.to_string())),
            created_at: Set(Utc::now()),
            ..Default::default()
        }
    }
    
    /// Create a test tunnel
    pub fn create_test_tunnel() -> tunnel::ActiveModel {
        tunnel::ActiveModel {
            id: Set(Uuid::new_v4()),
            github_user_id: Set("12345".to_string()),
            github_username: Set("testuser".to_string()),
            subdomain: Set("test-tunnel".to_string()),
            fqdn: Set("test-tunnel.fleetingdns.com".to_string()),
            local_port: Set(8080),
            slot: Set(1),
            certificate_serial: Set(None),
            ssh_key_pair_id: Set(None),
            created_at: Set(Utc::now()),
            expires_at: Set(Utc::now() + chrono::Duration::hours(1)),
            status: Set("active".to_string()),
            bytes_transferred: Set(0),
            request_count: Set(0),
            ..Default::default()
        }
    }
    
    /// Create a test auth token
    pub fn create_test_auth_token() -> auth_token::ActiveModel {
        auth_token::ActiveModel {
            token: Set("test-token-123".to_string()),
            token_type: Set("Bearer".to_string()),
            expires_at: Set(Utc::now() + chrono::Duration::hours(24)),
            user_id: Set("test-user-1".to_string()),
            created_at: Set(Utc::now()),
            ..Default::default()
        }
    }
}

/// Test suite for User entity
#[tokio::test]
async fn test_user_entity() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    let repo = SeaOrmUserRepository::new(db.db.clone());
    
    // Test creating a user
    let user_model = TestData::create_test_user();
    let created_user = repo.create(user_model).await.expect("Failed to create user");
    
    assert_eq!(created_user.id, "test-user-1");
    assert_eq!(created_user.github_user_id, "12345");
    assert_eq!(created_user.login, "testuser");
    
    // Test finding user by GitHub ID
    let found_user = repo.find_by_github_user_id("12345").await.expect("Failed to find user");
    assert!(found_user.is_some());
    let found_user = found_user.unwrap();
    assert_eq!(found_user.id, "test-user-1");
    
    // Test finding non-existent user
    let not_found = repo.find_by_github_user_id("99999").await.expect("Failed to query");
    assert!(not_found.is_none());
    
    // Test updating user
    let update_model = user::ActiveModel {
        id: Set("test-user-1".to_string()),
        name: Set(Some("Updated Test User".to_string())),
        ..Default::default()
    };
    let updated_user = repo.update(update_model).await.expect("Failed to update user");
    assert_eq!(updated_user.name, Some("Updated Test User".to_string()));
    
    // Test deleting user
    let deleted = repo.delete("test-user-1").await.expect("Failed to delete user");
    assert!(deleted);
    
    // Verify user is deleted
    let found_user = repo.find_by_github_user_id("12345").await.expect("Failed to query");
    assert!(found_user.is_none());
}

/// Test suite for ServicePlan entity
#[tokio::test]
async fn test_service_plan_entity() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    let repo = SeaOrmServicePlanRepository::new(db.db.clone());
    
    // Test creating a service plan
    let plan_model = TestData::create_test_service_plan();
    let created_plan = repo.create(plan_model).await.expect("Failed to create service plan");
    
    assert_eq!(created_plan.id, "basic");
    assert_eq!(created_plan.name, "Basic Plan");
    assert_eq!(created_plan.api_rate_limit, 1000);
    
    // Test finding service plan by ID
    let found_plan = repo.find_by_id("basic").await.expect("Failed to find service plan");
    assert!(found_plan.is_some());
    let found_plan = found_plan.unwrap();
    assert_eq!(found_plan.id, "basic");
    
    // Test finding all service plans
    let all_plans = repo.find_all().await.expect("Failed to find all service plans");
    assert!(!all_plans.is_empty());
    assert!(all_plans.iter().any(|p| p.id == "basic"));
    
    // Test updating service plan
    let update_model = service_plan::ActiveModel {
        id: Set("basic".to_string()),
        api_rate_limit: Set(2000),
        ..Default::default()
    };
    let updated_plan = repo.update(update_model).await.expect("Failed to update service plan");
    assert_eq!(updated_plan.api_rate_limit, 2000);
}

/// Test suite for Tunnel entity
#[tokio::test]
async fn test_tunnel_entity() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    let repo = SeaOrmTunnelRepository::new(db.db.clone());
    
    // Create a user first (required for foreign key)
    let user_repo = SeaOrmUserRepository::new(db.db.clone());
    let user_model = TestData::create_test_user();
    user_repo.create(user_model).await.expect("Failed to create user");
    
    // Test creating a tunnel
    let tunnel_model = TestData::create_test_tunnel();
    let created_tunnel = repo.create(tunnel_model).await.expect("Failed to create tunnel");
    
    assert_eq!(created_tunnel.github_user_id, "12345");
    assert_eq!(created_tunnel.subdomain, "test-tunnel");
    assert_eq!(created_tunnel.status, "active");
    
    // Test finding tunnel by ID
    let found_tunnel = repo.find_by_id(created_tunnel.id).await.expect("Failed to find tunnel");
    assert!(found_tunnel.is_some());
    let found_tunnel = found_tunnel.unwrap();
    assert_eq!(found_tunnel.id, created_tunnel.id);
    
    // Test finding tunnels by GitHub user ID
    let user_tunnels = repo.find_by_github_user_id("12345").await.expect("Failed to find user tunnels");
    assert!(!user_tunnels.is_empty());
    assert!(user_tunnels.iter().any(|t| t.id == created_tunnel.id));
    
    // Test finding active tunnels
    let active_tunnels = repo.find_active_by_github_user_id("12345").await.expect("Failed to find active tunnels");
    assert!(!active_tunnels.is_empty());
    assert!(active_tunnels.iter().all(|t| t.status == "active"));
    
    // Test updating tunnel
    let update_model = tunnel::ActiveModel {
        id: Set(created_tunnel.id),
        status: Set("inactive".to_string()),
        ..Default::default()
    };
    let updated_tunnel = repo.update(update_model).await.expect("Failed to update tunnel");
    assert_eq!(updated_tunnel.status, "inactive");
    
    // Test deleting tunnel
    let deleted = repo.delete(created_tunnel.id).await.expect("Failed to delete tunnel");
    assert!(deleted);
    
    // Verify tunnel is deleted
    let found_tunnel = repo.find_by_id(created_tunnel.id).await.expect("Failed to query");
    assert!(found_tunnel.is_none());
}

/// Test suite for AuthToken entity
#[tokio::test]
async fn test_auth_token_entity() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    
    // Create a user first (required for foreign key)
    let user_repo = SeaOrmUserRepository::new(db.db.clone());
    let user_model = TestData::create_test_user();
    user_repo.create(user_model).await.expect("Failed to create user");
    
    // Test creating an auth token
    let token_model = TestData::create_test_auth_token();
    let created_token = token_model.insert(&db.db).await.expect("Failed to create auth token");
    
    assert_eq!(created_token.token, "test-token-123");
    assert_eq!(created_token.token_type, "Bearer");
    assert_eq!(created_token.user_id, "test-user-1");
    
    // Test finding token
    let found_token = auth_token::Entity::find_by_id("test-token-123")
        .one(&db.db)
        .await
        .expect("Failed to find auth token")
        .expect("Auth token not found");
    
    assert_eq!(found_token.token, "test-token-123");
    
    // Test deleting token
    let deleted = auth_token::Entity::delete_by_id("test-token-123")
        .exec(&db.db)
        .await
        .expect("Failed to delete auth token");
    
    assert_eq!(deleted.rows_affected, 1);
}

/// Test suite for relationships
#[tokio::test]
async fn test_entity_relationships() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    
    // Create test data
    let user_repo = SeaOrmUserRepository::new(db.db.clone());
    let user_model = TestData::create_test_user();
    let user = user_repo.create(user_model).await.expect("Failed to create user");
    
    let plan_repo = SeaOrmServicePlanRepository::new(db.db.clone());
    let plan_model = TestData::create_test_service_plan();
    let plan = plan_repo.create(plan_model).await.expect("Failed to create service plan");
    
    // Test user-service plan relationship
    let user_plan = user_service_plan::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id.clone()),
        service_plan_id: Set(plan.id.clone()),
        start_date: Set(Utc::now()),
        end_date: Set(None),
        is_active: Set(true),
        created_at: Set(Utc::now()),
        ..Default::default()
    };
    
    let created_user_plan = user_plan.insert(&db.db).await.expect("Failed to create user service plan");
    assert_eq!(created_user_plan.user_id, user.id);
    assert_eq!(created_user_plan.service_plan_id, plan.id);
    
    // Test finding related data
    let user_with_plans = user::Entity::find_by_id(&user.id)
        .find_also_related(user_service_plan::Entity)
        .all(&db.db)
        .await
        .expect("Failed to find user with plans");
    
    assert!(!user_with_plans.is_empty());
}

/// Test suite for error handling
#[tokio::test]
async fn test_error_handling() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    let repo = SeaOrmUserRepository::new(db.db.clone());
    
    // Test creating user with duplicate ID
    let user1 = TestData::create_test_user();
    repo.create(user1).await.expect("Failed to create first user");
    
    let user2 = TestData::create_test_user(); // Same ID
    let result = repo.create(user2).await;
    assert!(result.is_err()); // Should fail due to duplicate primary key
    
    // Test finding non-existent user
    let result = repo.find_by_github_user_id("non-existent").await;
    assert!(result.is_ok());
    let found = result.unwrap();
    assert!(found.is_none());
}

/// Test suite for data validation
#[tokio::test]
async fn test_data_validation() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    
    // Test creating tunnel with invalid user ID
    let tunnel_model = tunnel::ActiveModel {
        id: Set(Uuid::new_v4()),
        github_user_id: Set("non-existent-user".to_string()),
        github_username: Set("testuser".to_string()),
        subdomain: Set("test-tunnel".to_string()),
        fqdn: Set("test-tunnel.fleetingdns.com".to_string()),
        local_port: Set(8080),
        slot: Set(1),
        certificate_serial: Set(None),
        ssh_key_pair_id: Set(None),
        created_at: Set(Utc::now()),
        expires_at: Set(Utc::now() + chrono::Duration::hours(1)),
        status: Set("active".to_string()),
        bytes_transferred: Set(0),
        request_count: Set(0),
        ..Default::default()
    };
    
    let result = tunnel_model.insert(&db.db).await;
    assert!(result.is_err()); // Should fail due to foreign key constraint
}

/// Test suite for concurrent operations
#[tokio::test]
async fn test_concurrent_operations() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    let repo = SeaOrmUserRepository::new(db.db.clone());
    
    // Create multiple users concurrently
    let mut handles = vec![];
    
    for i in 1..=5 {
        let user_model = user::ActiveModel {
            id: Set(format!("concurrent-user-{}", i)),
            github_user_id: Set(format!("github-{}", i)),
            login: Set(format!("user{}", i)),
            name: Set(Some(format!("Concurrent User {}", i))),
            email: Set(Some(format!("user{}@example.com", i))),
            avatar_url: Set(Some("https://example.com/avatar.jpg".to_string())),
            public_repos: Set(Some(10)),
            followers: Set(Some(5)),
            following: Set(Some(3)),
            created_at: Set(Some(Utc::now())),
            updated_at: Set(Some(Utc::now())),
            created_at_db: Set(Utc::now()),
            ..Default::default()
        };
        
        let repo_clone = SeaOrmUserRepository::new(db.db.clone());
        let handle = tokio::spawn(async move {
            repo_clone.create(user_model).await
        });
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    let results = futures::future::join_all(handles).await;
    
    // Verify all operations succeeded
    for result in results {
        assert!(result.unwrap().is_ok());
    }
    
    // Verify all users were created
    let all_users = repo.find_all().await.expect("Failed to find all users");
    assert!(all_users.len() >= 5);
}

/// Test suite for performance
#[tokio::test]
async fn test_performance() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    let repo = SeaOrmUserRepository::new(db.db.clone());
    
    let start = std::time::Instant::now();
    
    // Create 100 users
    for i in 1..=100 {
        let user_model = user::ActiveModel {
            id: Set(format!("perf-user-{}", i)),
            github_user_id: Set(format!("github-perf-{}", i)),
            login: Set(format!("perfuser{}", i)),
            name: Set(Some(format!("Performance User {}", i))),
            email: Set(Some(format!("perfuser{}@example.com", i))),
            avatar_url: Set(Some("https://example.com/avatar.jpg".to_string())),
            public_repos: Set(Some(10)),
            followers: Set(Some(5)),
            following: Set(Some(3)),
            created_at: Set(Some(Utc::now())),
            updated_at: Set(Some(Utc::now())),
            created_at_db: Set(Utc::now()),
            ..Default::default()
        };
        
        repo.create(user_model).await.expect(&format!("Failed to create user {}", i));
    }
    
    let duration = start.elapsed();
    println!("Created 100 users in {:?}", duration);
    
    // Performance should be reasonable (less than 10 seconds for 100 users)
    assert!(duration.as_secs() < 10);
}

/// Test suite for cleanup
#[tokio::test]
async fn test_cleanup() {
    let db = TestDatabase::new().await.expect("Failed to initialize test database");
    
    // Create some test data
    let user_repo = SeaOrmUserRepository::new(db.db.clone());
    let user_model = TestData::create_test_user();
    let user = user_repo.create(user_model).await.expect("Failed to create user");
    
    // Verify data exists
    let found_user = user_repo.find_by_github_user_id("12345").await.expect("Failed to find user");
    assert!(found_user.is_some());
    
    // Clean up
    let deleted = user_repo.delete(&user.id).await.expect("Failed to delete user");
    assert!(deleted);
    
    // Verify cleanup
    let found_user = user_repo.find_by_github_user_id("12345").await.expect("Failed to query");
    assert!(found_user.is_none());
}

// Helper function to find all users (for testing)
impl SeaOrmUserRepository {
    async fn find_all(&self) -> ModelResult<Vec<user::Model>> {
        let users = user::Entity::find()
            .all(&self.db)
            .await
            .map_err(|e| ModelError::DatabaseError(e.to_string()))?;
        
        Ok(users)
    }
} 