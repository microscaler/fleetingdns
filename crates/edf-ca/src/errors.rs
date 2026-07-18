//! Error types for the FleetingDNS Certificate Authority

use common::error::{CommonResult, FleetingDnsError};

/// Type alias for CA errors using the common error system
pub type CaError = FleetingDnsError;
pub type CaResult<T> = CommonResult<T>;
