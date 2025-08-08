//! FleetingDNS Database Models
//! 
//! This crate contains all database models using SeaORM entities.
//! Models are designed for both REST API and GraphQL (via Seaography) usage.

pub mod entities;
pub mod repository;

pub use entities::*;
pub use repository::*;

// Re-export commonly used types
pub use sea_orm::*;
pub use seaography::*;

// Use common error system
pub use common::error::FleetingDnsError as ModelError;
pub type ModelResult<T> = Result<T, ModelError>;

#[cfg(test)]
mod tests; 