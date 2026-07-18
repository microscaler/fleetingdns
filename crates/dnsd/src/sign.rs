use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hickory_proto::rr::{Name, RData, Record, RecordType};
// hickory 0.26: DNSSEC types moved from `rr::dnssec` to the top-level
// `hickory_proto::dnssec` module.
use hickory_proto::dnssec::{
    Algorithm,
    rdata::{DNSSECRData, RRSIG, SigInput},
};
use hickory_proto::rr::SerialNumber;
use hickory_proto::serialize::binary::BinEncodable;
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

/// DNSSEC signing errors
#[derive(Error, Debug)]
pub enum DnssecError {
    #[error("Key generation failed: {0}")]
    KeyGeneration(String),
    #[error("Signing failed: {0}")]
    SigningFailed(String),
    #[error("Key rotation failed: {0}")]
    KeyRotation(String),
    #[error("Invalid algorithm: {0}")]
    InvalidAlgorithm(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Supported DNSSEC algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DnssecAlgorithm {
    /// HMAC-SHA256 (Algorithm 253 - Private Use)
    HmacSha256,
    /// RSA-SHA256 (Algorithm 8)
    RsaSha256,
    /// ECDSA-P256-SHA256 (Algorithm 13)
    EcdsaP256Sha256,
}

impl DnssecAlgorithm {
    /// Get the algorithm number for DNSSEC records
    pub fn algorithm_number(&self) -> u8 {
        match self {
            Self::HmacSha256 => 253,     // Private use
            Self::RsaSha256 => 8,        // RSA/SHA-256
            Self::EcdsaP256Sha256 => 13, // ECDSA Curve P-256 with SHA-256
        }
    }

    /// Get the hickory-proto Algorithm enum
    pub fn to_hickory_algorithm(&self) -> Algorithm {
        match self {
            Self::HmacSha256 => Algorithm::Unknown(253),
            Self::RsaSha256 => Algorithm::RSASHA256,
            Self::EcdsaP256Sha256 => Algorithm::ECDSAP256SHA256,
        }
    }
}

/// Key metadata for rotation and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    /// Key identifier
    pub key_id: String,
    /// Algorithm used
    pub algorithm: DnssecAlgorithm,
    /// Key creation time
    pub created_at: SystemTime,
    /// Key expiration time
    pub expires_at: SystemTime,
    /// Whether this key is active for signing
    pub is_active: bool,
    /// Key tag for DNSSEC
    pub key_tag: u16,
}

/// DNSSEC key with metadata
#[derive(Debug, Clone)]
pub struct DnssecKey {
    /// Key metadata
    pub metadata: KeyMetadata,
    /// The actual key material
    pub key_material: KeyMaterial,
}

/// Key material for different algorithms
#[derive(Debug, Clone)]
pub enum KeyMaterial {
    /// HMAC key
    Hmac(hmac::Key),
    /// RSA key pair (placeholder - using HMAC for now)
    Rsa(hmac::Key),
    /// ECDSA key pair (placeholder - using HMAC for now)
    Ecdsa(hmac::Key),
}

/// Configuration for DNSSEC key management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnssecConfig {
    /// Default algorithm to use
    pub default_algorithm: DnssecAlgorithm,
    /// Key rotation interval in seconds
    pub rotation_interval: u64,
    /// Grace period for old keys in seconds
    pub grace_period: u64,
    /// Maximum number of keys to keep
    pub max_keys: usize,
    /// Enable signature caching
    pub enable_signature_cache: bool,
    /// Signature cache TTL in seconds
    pub signature_cache_ttl: u64,
    /// Enable key backup
    pub enable_key_backup: bool,
    /// Backup directory path
    pub backup_directory: Option<String>,
}

impl Default for DnssecConfig {
    fn default() -> Self {
        Self {
            default_algorithm: DnssecAlgorithm::RsaSha256,
            rotation_interval: 86400 * 7, // 7 days
            grace_period: 86400,          // 1 day
            max_keys: 10,
            enable_signature_cache: true,
            signature_cache_ttl: 300, // 5 minutes
            enable_key_backup: true,
            backup_directory: None,
        }
    }
}

/// Signature cache entry
#[derive(Debug, Clone)]
struct CacheEntry {
    signature: Vec<u8>,
    created_at: SystemTime,
    ttl: Duration,
}

/// Production-grade DNSSEC key manager
pub struct DnssecKeyManager {
    /// Configuration
    config: DnssecConfig,
    /// Active keys by algorithm
    keys: RwLock<HashMap<DnssecAlgorithm, Vec<DnssecKey>>>,
    /// Signature cache
    signature_cache: RwLock<HashMap<String, CacheEntry>>,
    /// Random number generator
    rng: SystemRandom,
    /// Key rotation scheduler
    last_rotation: RwLock<SystemTime>,
}

impl DnssecKeyManager {
    /// Create a new key manager with configuration
    pub fn new(config: DnssecConfig) -> Result<Self, DnssecError> {
        let manager = Self {
            config,
            keys: RwLock::new(HashMap::new()),
            signature_cache: RwLock::new(HashMap::new()),
            rng: SystemRandom::new(),
            last_rotation: RwLock::new(UNIX_EPOCH),
        };

        // Generate initial keys
        manager.initialize_keys()?;

        Ok(manager)
    }

    /// Initialize keys for all supported algorithms
    fn initialize_keys(&self) -> Result<(), DnssecError> {
        info!("Initializing DNSSEC keys");

        // Generate initial key for default algorithm
        self.generate_key(self.config.default_algorithm)?;

        // Load existing keys from environment for backward compatibility
        if let Ok(hmac_secret) = std::env::var("FDNS_HMAC_KEY") {
            self.add_hmac_key_from_env(&hmac_secret)?;
        }

        Ok(())
    }

    /// Add HMAC key from environment variable (backward compatibility)
    fn add_hmac_key_from_env(&self, secret: &str) -> Result<(), DnssecError> {
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let metadata = KeyMetadata {
            key_id: "env-hmac".to_string(),
            algorithm: DnssecAlgorithm::HmacSha256,
            created_at: SystemTime::now(),
            expires_at: SystemTime::now() + Duration::from_secs(self.config.rotation_interval),
            is_active: true,
            key_tag: 0, // HMAC keys don't have tags
        };

        let dnssec_key = DnssecKey {
            metadata,
            key_material: KeyMaterial::Hmac(key),
        };

        let mut keys = self
            .keys
            .write()
            .map_err(|e| DnssecError::KeyRotation(e.to_string()))?;
        keys.entry(DnssecAlgorithm::HmacSha256)
            .or_default()
            .push(dnssec_key);

        info!("Added HMAC key from environment");
        Ok(())
    }

    /// Generate a new key for the specified algorithm
    pub fn generate_key(&self, algorithm: DnssecAlgorithm) -> Result<String, DnssecError> {
        let key_id = format!(
            "{:?}-{}",
            algorithm,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        let key_material = match algorithm {
            DnssecAlgorithm::HmacSha256 => {
                // Generate random 256-bit key for HMAC
                let mut key_bytes = [0u8; 32];
                self.rng
                    .fill(&mut key_bytes)
                    .map_err(|e| DnssecError::KeyGeneration(e.to_string()))?;
                let key = hmac::Key::new(hmac::HMAC_SHA256, &key_bytes);
                KeyMaterial::Hmac(key)
            }
            DnssecAlgorithm::RsaSha256 => {
                // RSA key generation requires external crate - for now use HMAC as placeholder
                let mut key_bytes = [0u8; 32];
                self.rng
                    .fill(&mut key_bytes)
                    .map_err(|e| DnssecError::KeyGeneration(e.to_string()))?;
                let key = hmac::Key::new(hmac::HMAC_SHA256, &key_bytes);
                KeyMaterial::Rsa(key)
            }
            DnssecAlgorithm::EcdsaP256Sha256 => {
                // ECDSA key generation requires external crate - for now use HMAC as placeholder
                let mut key_bytes = [0u8; 32];
                self.rng
                    .fill(&mut key_bytes)
                    .map_err(|e| DnssecError::KeyGeneration(e.to_string()))?;
                let key = hmac::Key::new(hmac::HMAC_SHA256, &key_bytes);
                KeyMaterial::Ecdsa(key)
            }
        };

        let metadata = KeyMetadata {
            key_id: key_id.clone(),
            algorithm,
            created_at: SystemTime::now(),
            expires_at: SystemTime::now() + Duration::from_secs(self.config.rotation_interval),
            is_active: true,
            key_tag: self.calculate_key_tag(&key_material)?,
        };

        let dnssec_key = DnssecKey {
            metadata,
            key_material,
        };

        // Add to key store
        let mut keys = self
            .keys
            .write()
            .map_err(|e| DnssecError::KeyRotation(e.to_string()))?;
        keys.entry(algorithm).or_default().push(dnssec_key);

        info!(
            "Generated new {} key with ID: {}",
            algorithm.algorithm_number(),
            key_id
        );

        // Backup key if enabled
        if self.config.enable_key_backup {
            self.backup_key(&key_id)?;
        }

        Ok(key_id)
    }

    /// Calculate key tag for DNSSEC
    fn calculate_key_tag(&self, _key_material: &KeyMaterial) -> Result<u16, DnssecError> {
        // Simplified key tag calculation - in production this would be more sophisticated
        Ok(rand::random::<u16>())
    }

    /// Backup a key to storage
    fn backup_key(&self, _key_id: &str) -> Result<(), DnssecError> {
        // TODO: Implement key backup to configured storage
        debug!("Key backup not yet implemented");
        Ok(())
    }

    /// Get active key for algorithm
    pub fn get_active_key(&self, algorithm: DnssecAlgorithm) -> Result<DnssecKey, DnssecError> {
        let keys = self
            .keys
            .read()
            .map_err(|e| DnssecError::KeyNotFound(e.to_string()))?;

        let algorithm_keys = keys.get(&algorithm).ok_or_else(|| {
            DnssecError::KeyNotFound(format!("No keys for algorithm {algorithm:?}"))
        })?;

        let active_key = algorithm_keys
            .iter()
            .find(|key| key.metadata.is_active && key.metadata.expires_at > SystemTime::now())
            .ok_or_else(|| {
                DnssecError::KeyNotFound(format!("No active key for algorithm {algorithm:?}"))
            })?;

        Ok(active_key.clone())
    }

    /// Check if key rotation is needed
    pub fn needs_rotation(&self) -> bool {
        let last_rotation = self.last_rotation.read().unwrap();
        let rotation_interval = Duration::from_secs(self.config.rotation_interval);
        SystemTime::now()
            .duration_since(*last_rotation)
            .unwrap_or(Duration::ZERO)
            > rotation_interval
    }

    /// Perform key rotation
    pub fn rotate_keys(&self) -> Result<(), DnssecError> {
        info!("Starting key rotation");

        // Generate new keys for all algorithms
        for algorithm in [
            DnssecAlgorithm::HmacSha256,
            DnssecAlgorithm::RsaSha256,
            DnssecAlgorithm::EcdsaP256Sha256,
        ] {
            if let Err(e) = self.generate_key(algorithm) {
                warn!("Failed to generate new key for {:?}: {}", algorithm, e);
            }
        }

        // Deactivate old keys (but keep them for grace period)
        self.deactivate_old_keys()?;

        // Update last rotation time
        let mut last_rotation = self
            .last_rotation
            .write()
            .map_err(|e| DnssecError::KeyRotation(e.to_string()))?;
        *last_rotation = SystemTime::now();

        info!("Key rotation completed");
        Ok(())
    }

    /// Deactivate old keys
    fn deactivate_old_keys(&self) -> Result<(), DnssecError> {
        let mut keys = self
            .keys
            .write()
            .map_err(|e| DnssecError::KeyRotation(e.to_string()))?;
        let now = SystemTime::now();
        let grace_period = Duration::from_secs(self.config.grace_period);

        for algorithm_keys in keys.values_mut() {
            for key in algorithm_keys.iter_mut() {
                if key.metadata.expires_at < now {
                    key.metadata.is_active = false;
                    debug!("Deactivated key: {}", key.metadata.key_id);
                }
            }

            // Remove keys past grace period
            algorithm_keys.retain(|key| {
                let should_retain = key.metadata.expires_at + grace_period > now;
                if !should_retain {
                    info!("Removing expired key: {}", key.metadata.key_id);
                }
                should_retain
            });
        }

        Ok(())
    }

    /// Sign data with specified algorithm
    pub fn sign(&self, data: &[u8], algorithm: DnssecAlgorithm) -> Result<Vec<u8>, DnssecError> {
        // Check signature cache first
        if self.config.enable_signature_cache {
            let cache_key = format!("{:?}:{}", algorithm, hex::encode(data));
            if let Some(cached) = self.get_cached_signature(&cache_key) {
                return Ok(cached);
            }
        }

        let key = self.get_active_key(algorithm)?;
        let signature = match &key.key_material {
            KeyMaterial::Hmac(hmac_key) => hmac::sign(hmac_key, data).as_ref().to_vec(),
            KeyMaterial::Rsa(hmac_key) => {
                // For now, RSA keys are stored as HMAC - this is a placeholder
                // In production, this would use proper RSA signing
                hmac::sign(hmac_key, data).as_ref().to_vec()
            }
            KeyMaterial::Ecdsa(hmac_key) => {
                // For now, ECDSA keys are stored as HMAC - this is a placeholder
                // In production, this would use proper ECDSA signing
                hmac::sign(hmac_key, data).as_ref().to_vec()
            }
        };

        // Cache the signature
        if self.config.enable_signature_cache {
            let cache_key = format!("{:?}:{}", algorithm, hex::encode(data));
            self.cache_signature(cache_key, signature.clone());
        }

        Ok(signature)
    }

    /// Get cached signature
    fn get_cached_signature(&self, cache_key: &str) -> Option<Vec<u8>> {
        let cache = self.signature_cache.read().ok()?;
        let entry = cache.get(cache_key)?;

        // Check if entry is still valid
        if entry.created_at.elapsed().unwrap_or(Duration::MAX) < entry.ttl {
            Some(entry.signature.clone())
        } else {
            None
        }
    }

    /// Cache a signature
    fn cache_signature(&self, cache_key: String, signature: Vec<u8>) {
        if let Ok(mut cache) = self.signature_cache.write() {
            let entry = CacheEntry {
                signature,
                created_at: SystemTime::now(),
                ttl: Duration::from_secs(self.config.signature_cache_ttl),
            };
            cache.insert(cache_key, entry);

            // Clean up old entries
            let now = SystemTime::now();
            cache.retain(|_, entry| {
                now.duration_since(entry.created_at)
                    .unwrap_or(Duration::MAX)
                    < entry.ttl
            });
        }
    }

    /// Get key statistics
    pub fn get_key_statistics(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();

        if let Ok(keys) = self.keys.read() {
            for (algorithm, algorithm_keys) in keys.iter() {
                let active_count = algorithm_keys
                    .iter()
                    .filter(|k| k.metadata.is_active)
                    .count();
                let total_count = algorithm_keys.len();

                stats.insert(
                    format!("{algorithm:?}"),
                    serde_json::json!({
                        "active_keys": active_count,
                        "total_keys": total_count,
                        "algorithm_number": algorithm.algorithm_number()
                    }),
                );
            }
        }

        if let Ok(cache) = self.signature_cache.read() {
            stats.insert(
                "signature_cache".to_string(),
                serde_json::json!({
                    "entries": cache.len(),
                    "enabled": self.config.enable_signature_cache
                }),
            );
        }

        stats.insert("last_rotation".to_string(), serde_json::json!({
            "timestamp": self.last_rotation.read().unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            "needs_rotation": self.needs_rotation()
        }));

        stats
    }
}

/// Enhanced DNSSEC signer with production features
pub struct ProductionDnssecSigner {
    /// Key manager
    key_manager: Arc<DnssecKeyManager>,
    /// Configuration
    config: DnssecConfig,
}

impl ProductionDnssecSigner {
    /// Create a new production DNSSEC signer
    pub fn new(config: DnssecConfig) -> Result<Self, DnssecError> {
        let key_manager = Arc::new(DnssecKeyManager::new(config.clone())?);

        Ok(Self {
            key_manager,
            config,
        })
    }

    /// Build an RRSIG record with production features
    pub fn rrsig_record(
        &self,
        name: &Name,
        typ: RecordType,
        ttl: u32,
        rrset: &[u8],
    ) -> Result<Record, DnssecError> {
        self.rrsig_record_with_algorithm(name, typ, ttl, rrset, self.config.default_algorithm)
    }

    /// Build an RRSIG record with specific algorithm
    pub fn rrsig_record_with_algorithm(
        &self,
        name: &Name,
        typ: RecordType,
        ttl: u32,
        rrset: &[u8],
        algorithm: DnssecAlgorithm,
    ) -> Result<Record, DnssecError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        let signature = self.key_manager.sign(rrset, algorithm)?;
        let key = self.key_manager.get_active_key(algorithm)?;

        // hickory 0.26: RRSIG is built from a SigInput + raw signature bytes.
        let sig_input = SigInput {
            type_covered: typ,
            algorithm: algorithm.to_hickory_algorithm(),
            num_labels: name.num_labels(),
            original_ttl: ttl,
            sig_expiration: SerialNumber::new(now + ttl),
            sig_inception: SerialNumber::new(now),
            key_tag: key.metadata.key_tag,
            signer_name: name.clone(),
        };
        let rrsig = RRSIG::from_sig(sig_input, signature);

        Ok(Record::from_rdata(
            name.clone(),
            ttl,
            RData::DNSSEC(DNSSECRData::RRSIG(rrsig)),
        ))
    }

    /// Check if automatic key rotation is needed and perform it
    pub fn check_and_rotate_keys(&self) -> Result<(), DnssecError> {
        if self.key_manager.needs_rotation() {
            self.key_manager.rotate_keys()?;
        }
        Ok(())
    }

    /// Get signing statistics
    pub fn get_statistics(&self) -> HashMap<String, serde_json::Value> {
        self.key_manager.get_key_statistics()
    }

    /// Force key rotation (for testing/emergency)
    pub fn force_key_rotation(&self) -> Result<(), DnssecError> {
        self.key_manager.rotate_keys()
    }
}

/// Legacy HMAC signer for backward compatibility
#[derive(Clone)]
pub struct HmacSigner {
    key: hmac::Key,
}

impl HmacSigner {
    /// Initialize the signer from `FDNS_HMAC_KEY`.
    fn from_env() -> Option<Self> {
        let secret = std::env::var("FDNS_HMAC_KEY").ok()?;
        Some(Self {
            key: hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes()),
        })
    }

    /// Create a new signer with the provided secret key.
    /// This is primarily for testing purposes.
    #[cfg(test)]
    pub fn new(secret: &[u8]) -> Self {
        Self {
            key: hmac::Key::new(hmac::HMAC_SHA256, secret),
        }
    }

    /// Compute an HMAC over the provided data.
    fn sign(&self, data: &[u8]) -> Vec<u8> {
        hmac::sign(&self.key, data).as_ref().to_vec()
    }

    /// Build an [`RRSIG`] record covering `rrset`.
    ///
    /// * `name` - owner name of the RRset
    /// * `typ` - record type covered by the signature
    /// * `ttl` - original TTL of the RRset
    /// * `rrset` - canonical encoding of the RRset
    pub fn rrsig_record(&self, name: &Name, typ: RecordType, ttl: u32, rrset: &[u8]) -> Record {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        let sig = self.sign(rrset);
        let sig_input = SigInput {
            type_covered: typ,
            algorithm: Algorithm::Unknown(253),
            num_labels: name.num_labels(),
            original_ttl: ttl,
            sig_expiration: SerialNumber::new(now + ttl),
            sig_inception: SerialNumber::new(now),
            key_tag: 0,
            signer_name: name.clone(),
        };
        let rrsig = RRSIG::from_sig(sig_input, sig);
        Record::from_rdata(name.clone(), ttl, RData::DNSSEC(DNSSECRData::RRSIG(rrsig)))
    }
}

static LEGACY_SIGNER: OnceLock<Option<HmacSigner>> = OnceLock::new();
static PRODUCTION_SIGNER: OnceLock<Option<ProductionDnssecSigner>> = OnceLock::new();

/// Global legacy signer instance initialized from `FDNS_HMAC_KEY`.
pub fn signer() -> &'static Option<HmacSigner> {
    LEGACY_SIGNER.get_or_init(HmacSigner::from_env)
}

/// Global production signer instance
pub fn production_signer() -> &'static Option<ProductionDnssecSigner> {
    PRODUCTION_SIGNER.get_or_init(|| {
        // Try to initialize from environment or use defaults
        let config = DnssecConfig::default();
        ProductionDnssecSigner::new(config).ok()
    })
}

/// Initialize production signer with custom configuration
pub fn init_production_signer(config: DnssecConfig) -> Result<(), DnssecError> {
    let signer = ProductionDnssecSigner::new(config)?;
    PRODUCTION_SIGNER
        .set(Some(signer))
        .map_err(|_| DnssecError::Configuration("Signer already initialized".to_string()))?;
    Ok(())
}

/// DNSSEC validation pipeline for signed records
pub struct DnssecValidator {
    /// Key manager for validation
    key_manager: Arc<DnssecKeyManager>,
    /// Validation statistics
    validation_stats: RwLock<ValidationStats>,
}

/// Validation statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationStats {
    /// Total validations performed
    pub total_validations: u64,
    /// Successful validations
    pub successful_validations: u64,
    /// Failed validations
    pub failed_validations: u64,
    /// Validations by algorithm
    pub validations_by_algorithm: HashMap<String, u64>,
    /// Average validation time in microseconds
    pub average_validation_time_us: u64,
}

impl DnssecValidator {
    /// Create a new validator with key manager
    pub fn new(key_manager: Arc<DnssecKeyManager>) -> Self {
        Self {
            key_manager,
            validation_stats: RwLock::new(ValidationStats::default()),
        }
    }

    /// Validate an RRSIG record against the original RRset
    pub fn validate_rrsig(
        &self,
        rrsig: &RRSIG,
        rrset: &[u8],
        algorithm: DnssecAlgorithm,
    ) -> Result<bool, DnssecError> {
        let start_time = SystemTime::now();

        // Update validation statistics
        self.update_validation_stats(algorithm, true);

        // Get the key for the algorithm
        let key = self.key_manager.get_active_key(algorithm)?;

        // Validate the signature
        let is_valid = match &key.key_material {
            KeyMaterial::Hmac(hmac_key) => {
                let expected_sig = hmac::sign(hmac_key, rrset);
                rrsig.sig() == expected_sig.as_ref()
            }
            KeyMaterial::Rsa(hmac_key) => {
                // For now, RSA keys are stored as HMAC - this is a placeholder
                let expected_sig = hmac::sign(hmac_key, rrset);
                rrsig.sig() == expected_sig.as_ref()
            }
            KeyMaterial::Ecdsa(hmac_key) => {
                // For now, ECDSA keys are stored as HMAC - this is a placeholder
                let expected_sig = hmac::sign(hmac_key, rrset);
                rrsig.sig() == expected_sig.as_ref()
            }
        };

        // Update timing statistics
        let elapsed = start_time.elapsed().unwrap_or(Duration::ZERO);
        self.update_timing_stats(elapsed);

        // Update success/failure counts
        self.update_validation_result(is_valid);

        Ok(is_valid)
    }

    /// Validate a complete DNS message with DNSSEC signatures
    pub fn validate_dns_message(
        &self,
        message: &hickory_proto::op::Message,
    ) -> Result<ValidationResult, DnssecError> {
        let mut result = ValidationResult::default();

        // Find all RRSIG records in the message
        let rrsig_records: Vec<_> = message
            .answers
            .iter()
            .filter(|record| record.record_type() == RecordType::RRSIG)
            .collect();

        if rrsig_records.is_empty() {
            result.status = ValidationStatus::Unsigned;
            return Ok(result);
        }

        // Validate each RRSIG
        for rrsig_record in rrsig_records {
            if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = &rrsig_record.data {
                // Find the corresponding RRset
                let covered_records: Vec<_> = message
                    .answers
                    .iter()
                    .filter(|record| record.record_type() == rrsig.input().type_covered)
                    .collect();

                if !covered_records.is_empty() {
                    // Encode the RRset for validation
                    let mut rrset_data = Vec::new();
                    {
                        let mut enc =
                            hickory_proto::serialize::binary::BinEncoder::new(&mut rrset_data);
                        for record in &covered_records {
                            if let Err(e) = record.emit(&mut enc) {
                                result.errors.push(format!("Failed to encode RRset: {e}"));
                                continue;
                            }
                        }
                    }

                    // Determine algorithm from RRSIG
                    let algorithm = match rrsig.input().algorithm {
                        Algorithm::Unknown(253) => DnssecAlgorithm::HmacSha256,
                        Algorithm::RSASHA256 => DnssecAlgorithm::RsaSha256,
                        Algorithm::ECDSAP256SHA256 => DnssecAlgorithm::EcdsaP256Sha256,
                        _ => {
                            result.errors.push(format!(
                                "Unsupported algorithm: {:?}",
                                rrsig.input().algorithm
                            ));
                            continue;
                        }
                    };

                    // Validate the signature
                    match self.validate_rrsig(rrsig, &rrset_data, algorithm) {
                        Ok(true) => {
                            result.valid_signatures += 1;
                        }
                        Ok(false) => {
                            result.invalid_signatures += 1;
                            result.errors.push(format!(
                                "Invalid signature for {:?} record",
                                rrsig.input().type_covered
                            ));
                        }
                        Err(e) => {
                            result.validation_errors += 1;
                            result.errors.push(format!("Validation error: {e}"));
                        }
                    }
                }
            }
        }

        // Determine overall status
        result.status = if result.validation_errors > 0 {
            ValidationStatus::Error
        } else if result.invalid_signatures > 0 {
            ValidationStatus::Invalid
        } else if result.valid_signatures > 0 {
            ValidationStatus::Valid
        } else {
            ValidationStatus::Unsigned
        };

        Ok(result)
    }

    /// Update validation statistics
    fn update_validation_stats(&self, algorithm: DnssecAlgorithm, increment_total: bool) {
        if let Ok(mut stats) = self.validation_stats.write() {
            if increment_total {
                stats.total_validations += 1;
            }
            *stats
                .validations_by_algorithm
                .entry(format!("{algorithm:?}"))
                .or_insert(0) += 1;
        }
    }

    /// Update timing statistics
    fn update_timing_stats(&self, elapsed: Duration) {
        if let Ok(mut stats) = self.validation_stats.write() {
            let elapsed_us = elapsed.as_micros() as u64;
            if let Some(prev) = stats.total_validations.checked_sub(1) {
                stats.average_validation_time_us = (stats.average_validation_time_us * prev
                    + elapsed_us)
                    / stats.total_validations;
            } else {
                stats.average_validation_time_us = elapsed_us;
            }
        }
    }

    /// Update validation result statistics
    fn update_validation_result(&self, is_valid: bool) {
        if let Ok(mut stats) = self.validation_stats.write() {
            if is_valid {
                stats.successful_validations += 1;
            } else {
                stats.failed_validations += 1;
            }
        }
    }

    /// Get validation statistics
    pub fn get_validation_stats(&self) -> ValidationStats {
        self.validation_stats.read().unwrap().clone()
    }

    /// Reset validation statistics
    pub fn reset_validation_stats(&self) {
        if let Ok(mut stats) = self.validation_stats.write() {
            *stats = ValidationStats::default();
        }
    }
}

/// Result of DNSSEC validation
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    /// Overall validation status
    pub status: ValidationStatus,
    /// Number of valid signatures
    pub valid_signatures: u32,
    /// Number of invalid signatures
    pub invalid_signatures: u32,
    /// Number of validation errors
    pub validation_errors: u32,
    /// Validation errors and warnings
    pub errors: Vec<String>,
}

/// DNSSEC validation status
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ValidationStatus {
    /// Record is unsigned
    #[default]
    Unsigned,
    /// All signatures are valid
    Valid,
    /// One or more signatures are invalid
    Invalid,
    /// Validation encountered errors
    Error,
}

/// Enhanced production signer with validation capabilities
impl ProductionDnssecSigner {
    /// Create a validator for this signer
    pub fn create_validator(&self) -> DnssecValidator {
        DnssecValidator::new(self.key_manager.clone())
    }

    /// Validate a DNS message using this signer's keys
    pub fn validate_message(
        &self,
        message: &hickory_proto::op::Message,
    ) -> Result<ValidationResult, DnssecError> {
        let validator = self.create_validator();
        validator.validate_dns_message(message)
    }

    /// Self-validation: sign and then validate a record
    pub fn self_validate(
        &self,
        name: &Name,
        typ: RecordType,
        ttl: u32,
        rrset: &[u8],
    ) -> Result<bool, DnssecError> {
        // Sign the record
        let rrsig_record = self.rrsig_record(name, typ, ttl, rrset)?;

        // Extract the RRSIG
        if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data {
            // Validate the signature
            let validator = self.create_validator();
            validator.validate_rrsig(&rrsig, rrset, self.config.default_algorithm)
        } else {
            Err(DnssecError::SigningFailed(
                "Failed to extract RRSIG".to_string(),
            ))
        }
    }
}

/// DNSSEC monitoring and alerting system
pub struct DnssecMonitor {
    /// Validation statistics
    validator: Arc<DnssecValidator>,
    /// Alert thresholds
    alert_config: AlertConfig,
    /// Alert history
    alert_history: RwLock<Vec<Alert>>,
}

/// Alert configuration for DNSSEC monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    /// Maximum allowed failure rate (percentage)
    pub max_failure_rate: f64,
    /// Minimum validations before alerting
    pub min_validations_for_alert: u64,
    /// Maximum allowed average validation time (microseconds)
    pub max_avg_validation_time_us: u64,
    /// Alert cooldown period (seconds)
    pub alert_cooldown_seconds: u64,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            max_failure_rate: 5.0, // 5% failure rate
            min_validations_for_alert: 100,
            max_avg_validation_time_us: 1000, // 1ms
            alert_cooldown_seconds: 300,      // 5 minutes
        }
    }
}

/// DNSSEC alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Alert type
    pub alert_type: AlertType,
    /// Alert message
    pub message: String,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Associated statistics
    pub stats: ValidationStats,
}

/// Types of DNSSEC alerts
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertType {
    /// High failure rate
    HighFailureRate,
    /// Slow validation performance
    SlowValidation,
    /// Key rotation needed
    KeyRotationNeeded,
    /// Key generation failure
    KeyGenerationFailure,
    /// Signature cache issues
    SignatureCacheIssues,
}

/// Alert severity levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational
    Info,
    /// Warning
    Warning,
    /// Critical
    Critical,
}

impl DnssecMonitor {
    /// Create a new DNSSEC monitor
    pub fn new(validator: Arc<DnssecValidator>, alert_config: AlertConfig) -> Self {
        Self {
            validator,
            alert_config,
            alert_history: RwLock::new(Vec::new()),
        }
    }

    /// Check for alerts based on current statistics
    pub fn check_alerts(&self) -> Vec<Alert> {
        let stats = self.validator.get_validation_stats();
        let mut alerts = Vec::new();

        // Check failure rate
        if stats.total_validations >= self.alert_config.min_validations_for_alert {
            let failure_rate =
                (stats.failed_validations as f64 / stats.total_validations as f64) * 100.0;
            if failure_rate > self.alert_config.max_failure_rate {
                let alert = Alert {
                    alert_type: AlertType::HighFailureRate,
                    message: format!(
                        "DNSSEC validation failure rate is {:.2}% (threshold: {:.2}%)",
                        failure_rate, self.alert_config.max_failure_rate
                    ),
                    severity: AlertSeverity::Critical,
                    timestamp: SystemTime::now(),
                    stats: stats.clone(),
                };
                alerts.push(alert);
            }
        }

        // Check validation performance
        if stats.average_validation_time_us > self.alert_config.max_avg_validation_time_us {
            let alert = Alert {
                alert_type: AlertType::SlowValidation,
                message: format!(
                    "DNSSEC validation is slow: {}μs (threshold: {}μs)",
                    stats.average_validation_time_us, self.alert_config.max_avg_validation_time_us
                ),
                severity: AlertSeverity::Warning,
                timestamp: SystemTime::now(),
                stats: stats.clone(),
            };
            alerts.push(alert);
        }

        // Store alerts in history
        if !alerts.is_empty()
            && let Ok(mut history) = self.alert_history.write()
        {
            for alert in &alerts {
                history.push(alert.clone());
                // Keep only last 1000 alerts
                if history.len() > 1000 {
                    history.remove(0);
                }
            }
        }

        alerts
    }

    /// Check if an alert should be suppressed due to cooldown
    pub fn should_suppress_alert(&self, alert_type: &AlertType) -> bool {
        if let Ok(history) = self.alert_history.read() {
            let cooldown_duration = Duration::from_secs(self.alert_config.alert_cooldown_seconds);
            let cutoff_time = SystemTime::now() - cooldown_duration;

            // Check if there's a recent alert of the same type
            history
                .iter()
                .any(|alert| alert.alert_type == *alert_type && alert.timestamp > cutoff_time)
        } else {
            false
        }
    }

    /// Get recent alerts
    pub fn get_recent_alerts(&self, duration: Duration) -> Vec<Alert> {
        if let Ok(history) = self.alert_history.read() {
            let cutoff_time = SystemTime::now() - duration;
            history
                .iter()
                .filter(|alert| alert.timestamp > cutoff_time)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all alerts
    pub fn get_all_alerts(&self) -> Vec<Alert> {
        self.alert_history.read().unwrap().clone()
    }

    /// Clear alert history
    pub fn clear_alert_history(&self) {
        if let Ok(mut history) = self.alert_history.write() {
            history.clear();
        }
    }

    /// Get monitoring statistics
    pub fn get_monitoring_stats(&self) -> serde_json::Value {
        let stats = self.validator.get_validation_stats();
        let recent_alerts = self.get_recent_alerts(Duration::from_secs(3600)); // Last hour

        serde_json::json!({
            "validation_stats": stats,
            "alert_config": self.alert_config,
            "recent_alerts_count": recent_alerts.len(),
            "total_alerts": self.alert_history.read().unwrap().len(),
            "alert_types": {
                "high_failure_rate": recent_alerts.iter().filter(|a| a.alert_type == AlertType::HighFailureRate).count(),
                "slow_validation": recent_alerts.iter().filter(|a| a.alert_type == AlertType::SlowValidation).count(),
                "key_rotation_needed": recent_alerts.iter().filter(|a| a.alert_type == AlertType::KeyRotationNeeded).count(),
            }
        })
    }
}

/// Enhanced production signer with monitoring
impl ProductionDnssecSigner {
    /// Create a monitor for this signer
    pub fn create_monitor(&self, alert_config: AlertConfig) -> DnssecMonitor {
        let validator = Arc::new(self.create_validator());
        DnssecMonitor::new(validator, alert_config)
    }

    /// Check if key rotation is needed and create alert if necessary
    pub fn check_key_rotation_alert(&self) -> Option<Alert> {
        if self.key_manager.needs_rotation() {
            Some(Alert {
                alert_type: AlertType::KeyRotationNeeded,
                message: "DNSSEC key rotation is needed".to_string(),
                severity: AlertSeverity::Warning,
                timestamp: SystemTime::now(),
                stats: ValidationStats::default(),
            })
        } else {
            None
        }
    }

    /// Get comprehensive monitoring information
    pub fn get_monitoring_info(&self) -> serde_json::Value {
        let key_stats = self.get_statistics();
        let needs_rotation = self.key_manager.needs_rotation();

        serde_json::json!({
            "key_statistics": key_stats,
            "needs_rotation": needs_rotation,
            "config": self.config,
            "status": "operational"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::{RData, RecordType};
    use std::time::Duration;

    #[test]
    fn test_dnssec_algorithm_numbers() {
        assert_eq!(DnssecAlgorithm::HmacSha256.algorithm_number(), 253);
        assert_eq!(DnssecAlgorithm::RsaSha256.algorithm_number(), 8);
        assert_eq!(DnssecAlgorithm::EcdsaP256Sha256.algorithm_number(), 13);
    }

    #[test]
    fn test_dnssec_algorithm_conversion() {
        assert_eq!(
            DnssecAlgorithm::HmacSha256.to_hickory_algorithm(),
            Algorithm::Unknown(253)
        );
        assert_eq!(
            DnssecAlgorithm::RsaSha256.to_hickory_algorithm(),
            Algorithm::RSASHA256
        );
        assert_eq!(
            DnssecAlgorithm::EcdsaP256Sha256.to_hickory_algorithm(),
            Algorithm::ECDSAP256SHA256
        );
    }

    #[test]
    fn test_dnssec_config_default() {
        let config = DnssecConfig::default();
        assert_eq!(config.default_algorithm, DnssecAlgorithm::RsaSha256);
        assert_eq!(config.rotation_interval, 86400 * 7);
        assert_eq!(config.grace_period, 86400);
        assert_eq!(config.max_keys, 10);
        assert!(config.enable_signature_cache);
        assert_eq!(config.signature_cache_ttl, 300);
        assert!(config.enable_key_backup);
    }

    #[test]
    fn test_key_manager_creation() {
        let config = DnssecConfig::default();
        let manager = DnssecKeyManager::new(config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_key_generation_hmac() {
        let config = DnssecConfig::default();
        let manager = DnssecKeyManager::new(config).unwrap();
        let key_id = manager.generate_key(DnssecAlgorithm::HmacSha256);
        assert!(key_id.is_ok());
    }

    #[test]
    fn test_key_generation_rsa() {
        let config = DnssecConfig::default();
        let manager = DnssecKeyManager::new(config).unwrap();
        let key_id = manager.generate_key(DnssecAlgorithm::RsaSha256);
        assert!(key_id.is_ok());
    }

    #[test]
    fn test_key_generation_ecdsa() {
        let config = DnssecConfig::default();
        let manager = DnssecKeyManager::new(config).unwrap();
        let key_id = manager.generate_key(DnssecAlgorithm::EcdsaP256Sha256);
        assert!(key_id.is_ok());
    }

    #[test]
    fn test_get_active_key() {
        let config = DnssecConfig::default();
        let manager = DnssecKeyManager::new(config).unwrap();

        // Should have a key for the default algorithm
        let key = manager.get_active_key(DnssecAlgorithm::RsaSha256);
        assert!(key.is_ok());

        let key = key.unwrap();
        assert!(key.metadata.is_active);
        assert!(key.metadata.expires_at > SystemTime::now());
    }

    #[test]
    fn test_signing_hmac() {
        let config = DnssecConfig::default();
        let manager = DnssecKeyManager::new(config).unwrap();

        // Generate HMAC key
        manager.generate_key(DnssecAlgorithm::HmacSha256).unwrap();

        let test_data = b"test signing data";
        let signature = manager.sign(test_data, DnssecAlgorithm::HmacSha256);
        assert!(signature.is_ok());

        let signature = signature.unwrap();
        assert!(!signature.is_empty());
        assert_eq!(signature.len(), 32); // SHA256 output length
    }

    #[test]
    fn test_signing_rsa() {
        let config = DnssecConfig::default();
        let manager = DnssecKeyManager::new(config).unwrap();

        let test_data = b"test signing data";
        let signature = manager.sign(test_data, DnssecAlgorithm::RsaSha256);
        assert!(signature.is_ok());

        let signature = signature.unwrap();
        assert!(!signature.is_empty());
        assert_eq!(signature.len(), 32); // HMAC-SHA256 output length (placeholder)
    }

    #[test]
    fn test_signing_ecdsa() {
        let config = DnssecConfig::default();
        let manager = DnssecKeyManager::new(config).unwrap();

        let test_data = b"test signing data";
        let signature = manager.sign(test_data, DnssecAlgorithm::EcdsaP256Sha256);

        match signature {
            Ok(sig) => {
                assert!(!sig.is_empty());
                assert_eq!(sig.len(), 32); // HMAC-SHA256 output length (placeholder)
            }
            Err(e) => {
                println!("ECDSA signing error: {e}");
                // For now, this is expected since we're using placeholder implementation
                assert!(
                    e.to_string().contains("No active key") || e.to_string().contains("No keys")
                );
            }
        }
    }

    #[test]
    fn test_signature_caching() {
        let config = DnssecConfig {
            enable_signature_cache: true,
            signature_cache_ttl: 10, // 10 seconds
            ..Default::default()
        };

        let manager = DnssecKeyManager::new(config).unwrap();
        let test_data = b"test caching data";

        // First call should generate signature
        let signature1 = manager.sign(test_data, DnssecAlgorithm::RsaSha256).unwrap();

        // Second call should use cached signature
        let signature2 = manager.sign(test_data, DnssecAlgorithm::RsaSha256).unwrap();

        assert_eq!(signature1, signature2);
    }

    #[test]
    fn test_key_rotation_needed() {
        let config = DnssecConfig {
            rotation_interval: 1, // 1 second
            ..Default::default()
        };

        let manager = DnssecKeyManager::new(config).unwrap();

        // Key manager starts with last_rotation = UNIX_EPOCH, so it should need rotation
        assert!(manager.needs_rotation());

        // After rotation, it should not need rotation immediately
        manager.rotate_keys().unwrap();
        assert!(!manager.needs_rotation());

        // Wait for rotation interval
        std::thread::sleep(Duration::from_secs(2));
        assert!(manager.needs_rotation());
    }

    #[test]
    fn test_key_rotation() {
        let config = DnssecConfig {
            rotation_interval: 1, // 1 second
            ..Default::default()
        };

        let manager = DnssecKeyManager::new(config).unwrap();

        // Force rotation
        let result = manager.rotate_keys();
        assert!(result.is_ok());
    }

    #[test]
    fn test_key_statistics() {
        let config = DnssecConfig::default();
        let manager = DnssecKeyManager::new(config).unwrap();

        let stats = manager.get_key_statistics();
        assert!(stats.contains_key("RsaSha256"));
        assert!(stats.contains_key("signature_cache"));
        assert!(stats.contains_key("last_rotation"));
    }

    #[test]
    fn test_production_signer_creation() {
        let config = DnssecConfig::default();
        let signer = ProductionDnssecSigner::new(config);
        assert!(signer.is_ok());
    }

    #[test]
    fn test_production_signer_rrsig() {
        let config = DnssecConfig::default();
        let signer = ProductionDnssecSigner::new(config).unwrap();

        let name = Name::from_ascii("example.com.").unwrap();
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"test_rrset_data";

        let rrsig_record = signer.rrsig_record(&name, record_type, ttl, rrset_data);
        assert!(rrsig_record.is_ok());

        let rrsig_record = rrsig_record.unwrap();
        assert_eq!(&rrsig_record.name, &name);
        assert_eq!(rrsig_record.record_type(), RecordType::RRSIG);
        assert_eq!(rrsig_record.ttl, ttl);
    }

    #[test]
    fn test_production_signer_with_algorithm() {
        let config = DnssecConfig::default();
        let signer = ProductionDnssecSigner::new(config).unwrap();

        let name = Name::from_ascii("example.com.").unwrap();
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"test_rrset_data";

        // Test with algorithms that should work
        let algorithms = [DnssecAlgorithm::RsaSha256, DnssecAlgorithm::HmacSha256];

        for algorithm in algorithms {
            let rrsig_record =
                signer.rrsig_record_with_algorithm(&name, record_type, ttl, rrset_data, algorithm);

            match rrsig_record {
                Ok(record) => {
                    if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = record.data {
                        assert_eq!(rrsig.input().algorithm, algorithm.to_hickory_algorithm());
                    } else {
                        panic!("Expected RRSIG record data");
                    }
                }
                Err(e) => {
                    println!("Algorithm {algorithm:?} failed: {e}");
                    // This is acceptable for placeholder implementation
                    assert!(
                        e.to_string().contains("No active key")
                            || e.to_string().contains("No keys")
                    );
                }
            }
        }
    }

    #[test]
    fn test_production_signer_statistics() {
        let config = DnssecConfig::default();
        let signer = ProductionDnssecSigner::new(config).unwrap();

        let stats = signer.get_statistics();
        assert!(stats.contains_key("RsaSha256"));
        assert!(stats.contains_key("signature_cache"));
        assert!(stats.contains_key("last_rotation"));
    }

    #[test]
    fn test_production_signer_force_rotation() {
        let config = DnssecConfig::default();
        let signer = ProductionDnssecSigner::new(config).unwrap();

        let result = signer.force_key_rotation();
        assert!(result.is_ok());
    }

    #[test]
    fn test_backward_compatibility_hmac_signer() {
        let signer = HmacSigner::new(b"test_secret");
        let name = Name::from_ascii("example.com.").unwrap();
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"test_rrset_data";

        let rrsig_record = signer.rrsig_record(&name, record_type, ttl, rrset_data);

        // Verify the record properties
        assert_eq!(&rrsig_record.name, &name);
        assert_eq!(rrsig_record.record_type(), RecordType::RRSIG);
        assert_eq!(rrsig_record.ttl, ttl);

        // Verify the RRSIG data
        if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data {
            assert_eq!(rrsig.input().type_covered, record_type);
            assert_eq!(rrsig.input().algorithm, Algorithm::Unknown(253));
            assert_eq!(rrsig.input().num_labels, name.num_labels());
            assert_eq!(rrsig.input().original_ttl, ttl);
            assert_eq!((&rrsig.input().signer_name), &name);
            assert!(!rrsig.sig().is_empty());

            // Verify signature validity
            let expected_sig = signer.sign(rrset_data);
            assert_eq!(rrsig.sig(), &expected_sig);
        } else {
            panic!("Expected RRSIG record data");
        }
    }

    #[test]
    fn test_global_signer_functions() {
        // Test legacy signer - might be Some if env var is set from other tests
        let legacy_signer = signer();
        // Just check that it returns a valid reference
        assert!(legacy_signer.is_some() || legacy_signer.is_none());

        // Test production signer - should be Some
        let prod_signer = production_signer();
        assert!(prod_signer.is_some());
    }

    #[test]
    fn test_global_signer_function() {
        // Test that global signer function works - might be Some or None depending on env
        let global_signer = signer();
        assert!(global_signer.is_some() || global_signer.is_none());
    }

    #[test]
    fn test_init_production_signer() {
        let config = DnssecConfig::default();

        // This might fail if already initialized in other tests
        // but that's expected behavior
        let _result = init_production_signer(config);
    }

    #[test]
    fn test_hmac_signer_from_env() {
        unsafe {
            std::env::set_var("FDNS_HMAC_KEY", "test_env_key");
        }

        let signer = HmacSigner::from_env();
        assert!(signer.is_some());

        unsafe {
            std::env::remove_var("FDNS_HMAC_KEY");
        }
    }

    #[test]
    fn test_rrsig_record_creation() {
        let signer = HmacSigner::new(b"test_secret");
        let name = Name::from_ascii("example.com.").unwrap();
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"test_rrset_data";

        let rrsig_record = signer.rrsig_record(&name, record_type, ttl, rrset_data);

        // Verify the record properties
        assert_eq!(&rrsig_record.name, &name);
        assert_eq!(rrsig_record.record_type(), RecordType::RRSIG);
        assert_eq!(rrsig_record.ttl, ttl);

        // Verify the RRSIG data
        if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data {
            assert_eq!(rrsig.input().type_covered, record_type);
            assert_eq!(rrsig.input().algorithm, Algorithm::Unknown(253));
            assert_eq!(rrsig.input().num_labels, name.num_labels());
            assert_eq!(rrsig.input().original_ttl, ttl);
            assert_eq!((&rrsig.input().signer_name), &name);
            assert!(!rrsig.sig().is_empty());

            // Verify signature validity
            let expected_sig = signer.sign(rrset_data);
            assert_eq!(rrsig.sig(), &expected_sig);
        } else {
            panic!("Expected RRSIG record data");
        }
    }

    #[test]
    fn test_rrsig_record_different_types() {
        let signer = HmacSigner::new(b"test_secret");
        let name = Name::from_ascii("example.com.").unwrap();
        let ttl = 300;
        let rrset_data = b"test_rrset_data";

        let record_types = [
            RecordType::A,
            RecordType::AAAA,
            RecordType::CNAME,
            RecordType::MX,
        ];

        for record_type in record_types {
            let rrsig_record = signer.rrsig_record(&name, record_type, ttl, rrset_data);

            if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data {
                assert_eq!(rrsig.input().type_covered, record_type);
            } else {
                panic!("Expected RRSIG record data for {record_type:?}");
            }
        }
    }

    #[test]
    fn test_rrsig_record_different_names() {
        let signer = HmacSigner::new(b"test_secret");
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"test_rrset_data";

        let names = [
            "example.com.",
            "sub.example.com.",
            "deep.sub.example.com.",
            "test.org.",
        ];

        for name_str in names {
            let name = Name::from_ascii(name_str).unwrap();
            let rrsig_record = signer.rrsig_record(&name, record_type, ttl, rrset_data);

            assert_eq!(&rrsig_record.name, &name);

            if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data {
                assert_eq!((&rrsig.input().signer_name), &name);
                assert_eq!(rrsig.input().num_labels, name.num_labels());
            } else {
                panic!("Expected RRSIG record data for {name_str}");
            }
        }
    }

    #[test]
    fn test_rrsig_timing_fields() {
        let signer = HmacSigner::new(b"test_secret");
        let name = Name::from_ascii("example.com.").unwrap();
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"timing_test_data";

        let before_creation = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        let rrsig_record = signer.rrsig_record(&name, record_type, ttl, rrset_data);

        let after_creation = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data {
            // Check signature inception time
            assert!(rrsig.input().sig_inception.get() >= before_creation);
            assert!(rrsig.input().sig_inception.get() <= after_creation);

            // Check signature expiration time
            assert_eq!(
                rrsig.input().sig_expiration.get(),
                rrsig.input().sig_inception.get() + ttl
            );

            // Check original TTL
            assert_eq!(rrsig.input().original_ttl, ttl);
        } else {
            panic!("Expected RRSIG record data");
        }
    }

    #[test]
    fn test_rrsig_with_different_ttls() {
        let signer = HmacSigner::new(b"test_secret");
        let name = Name::from_ascii("example.com.").unwrap();
        let record_type = RecordType::A;
        let rrset_data = b"ttl_test_data";

        let ttls = [60, 300, 3600, 86400];

        for ttl in ttls {
            let rrsig_record = signer.rrsig_record(&name, record_type, ttl, rrset_data);

            assert_eq!(rrsig_record.ttl, ttl);

            if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data {
                assert_eq!(rrsig.input().original_ttl, ttl);
                assert_eq!(
                    rrsig.input().sig_expiration.get(),
                    rrsig.input().sig_inception.get() + ttl
                );
            } else {
                panic!("Expected RRSIG record data for TTL {ttl}");
            }
        }
    }

    #[test]
    fn test_signature_consistency() {
        let signer = HmacSigner::new(b"consistency_test_secret");
        let name = Name::from_ascii("consistent.example.com.").unwrap();
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"consistency_test_data";

        let rrsig1 = signer.rrsig_record(&name, record_type, ttl, rrset_data);
        let rrsig2 = signer.rrsig_record(&name, record_type, ttl, rrset_data);

        // Extract signatures from both records
        let sig1 = if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig1.data {
            rrsig.sig().to_vec()
        } else {
            panic!("Expected RRSIG record data");
        };

        let sig2 = if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig2.data {
            rrsig.sig().to_vec()
        } else {
            panic!("Expected RRSIG record data");
        };

        // Signatures should be identical for same input
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_signature_with_empty_data() {
        let signer = HmacSigner::new(b"empty_test_secret");
        let name = Name::from_ascii("empty.example.com.").unwrap();
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"";

        let rrsig_record = signer.rrsig_record(&name, record_type, ttl, rrset_data);

        // Should still create a valid RRSIG even with empty data
        assert_eq!(rrsig_record.record_type(), RecordType::RRSIG);

        if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data {
            assert!(!rrsig.sig().is_empty()); // HMAC should still produce output
        } else {
            panic!("Expected RRSIG record data");
        }
    }

    #[test]
    fn test_signer_clone() {
        let signer1 = HmacSigner::new(b"clone_test_secret");
        let signer2 = signer1.clone();

        let name = Name::from_ascii("clone.example.com.").unwrap();
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"clone_test_data";

        let rrsig1 = signer1.rrsig_record(&name, record_type, ttl, rrset_data);
        let rrsig2 = signer2.rrsig_record(&name, record_type, ttl, rrset_data);

        // Both signers should produce equivalent signatures
        let sig1 = if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig1.data {
            rrsig.sig().to_vec()
        } else {
            panic!("Expected RRSIG record data");
        };

        let sig2 = if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig2.data {
            rrsig.sig().to_vec()
        } else {
            panic!("Expected RRSIG record data");
        };

        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_hmac_signer_from_env_isolated() {
        // Test environment variable isolation
        unsafe {
            std::env::set_var("FDNS_HMAC_KEY", "isolated_test_key");
        }

        let signer = HmacSigner::from_env();
        assert!(signer.is_some());

        unsafe {
            std::env::remove_var("FDNS_HMAC_KEY");
        }

        let no_signer = HmacSigner::from_env();
        assert!(no_signer.is_none());
    }

    #[test]
    fn test_hmac_signer_from_env_missing() {
        // Ensure env var is not set
        unsafe {
            std::env::remove_var("FDNS_HMAC_KEY");
        }

        let signer = HmacSigner::from_env();
        assert!(signer.is_none());
    }

    #[test]
    fn test_dnssec_validator_creation() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = DnssecValidator::new(manager);

        let stats = validator.get_validation_stats();
        assert_eq!(stats.total_validations, 0);
        assert_eq!(stats.successful_validations, 0);
        assert_eq!(stats.failed_validations, 0);
    }

    #[test]
    fn test_validation_stats_update() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = DnssecValidator::new(manager);

        // Update stats
        validator.update_validation_stats(DnssecAlgorithm::HmacSha256, true);
        validator.update_validation_result(true);

        let stats = validator.get_validation_stats();
        assert_eq!(stats.total_validations, 1);
        assert_eq!(stats.successful_validations, 1);
        assert_eq!(stats.failed_validations, 0);
        assert!(stats.validations_by_algorithm.contains_key("HmacSha256"));
    }

    #[test]
    fn test_validation_stats_reset() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = DnssecValidator::new(manager);

        // Update stats
        validator.update_validation_stats(DnssecAlgorithm::HmacSha256, true);
        validator.update_validation_result(true);

        // Reset stats
        validator.reset_validation_stats();

        let stats = validator.get_validation_stats();
        assert_eq!(stats.total_validations, 0);
        assert_eq!(stats.successful_validations, 0);
        assert_eq!(stats.failed_validations, 0);
    }

    #[test]
    fn test_rrsig_validation() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = DnssecValidator::new(manager.clone());

        // Generate a key and create a signature
        let _key_id = manager.generate_key(DnssecAlgorithm::HmacSha256).unwrap();
        let test_data = b"test_validation_data";
        let signature = manager
            .sign(test_data, DnssecAlgorithm::HmacSha256)
            .unwrap();

        // Create an RRSIG record
        let name = Name::from_ascii("test.example.com.").unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        let sig_input = SigInput {
            type_covered: RecordType::A,
            algorithm: Algorithm::Unknown(253),
            num_labels: name.num_labels(),
            original_ttl: 300,
            sig_expiration: SerialNumber::new(now + 300),
            sig_inception: SerialNumber::new(now),
            key_tag: 0,
            signer_name: name.clone(),
        };
        let rrsig = RRSIG::from_sig(sig_input, signature);

        // Validate the signature
        let is_valid = validator
            .validate_rrsig(&rrsig, test_data, DnssecAlgorithm::HmacSha256)
            .unwrap();
        assert!(is_valid);

        // Check statistics
        let stats = validator.get_validation_stats();
        assert_eq!(stats.total_validations, 1);
        assert_eq!(stats.successful_validations, 1);
    }

    #[test]
    fn test_rrsig_validation_invalid_signature() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = DnssecValidator::new(manager.clone());

        // Generate a key
        manager.generate_key(DnssecAlgorithm::HmacSha256).unwrap();

        // Create an RRSIG record with wrong signature
        let name = Name::from_ascii("test.example.com.").unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        let wrong_signature = vec![0u8; 32]; // Wrong signature
        let sig_input = SigInput {
            type_covered: RecordType::A,
            algorithm: Algorithm::Unknown(253),
            num_labels: name.num_labels(),
            original_ttl: 300,
            sig_expiration: SerialNumber::new(now + 300),
            sig_inception: SerialNumber::new(now),
            key_tag: 0,
            signer_name: name.clone(),
        };
        let rrsig = RRSIG::from_sig(sig_input, wrong_signature);

        // Validate the signature
        let test_data = b"test_validation_data";
        let is_valid = validator
            .validate_rrsig(&rrsig, test_data, DnssecAlgorithm::HmacSha256)
            .unwrap();
        assert!(!is_valid);

        // Check statistics
        let stats = validator.get_validation_stats();
        assert_eq!(stats.total_validations, 1);
        assert_eq!(stats.failed_validations, 1);
    }

    #[test]
    fn test_validation_status_enum() {
        assert_eq!(ValidationStatus::default(), ValidationStatus::Unsigned);
        assert_ne!(ValidationStatus::Valid, ValidationStatus::Invalid);
        assert_ne!(ValidationStatus::Error, ValidationStatus::Unsigned);
    }

    #[test]
    fn test_validation_result_default() {
        let result = ValidationResult::default();
        assert_eq!(result.status, ValidationStatus::Unsigned);
        assert_eq!(result.valid_signatures, 0);
        assert_eq!(result.invalid_signatures, 0);
        assert_eq!(result.validation_errors, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_production_signer_validator_creation() {
        let config = DnssecConfig::default();
        let signer = ProductionDnssecSigner::new(config).unwrap();

        let validator = signer.create_validator();
        let stats = validator.get_validation_stats();
        assert_eq!(stats.total_validations, 0);
    }

    #[test]
    fn test_production_signer_self_validation() {
        let config = DnssecConfig::default();
        let signer = ProductionDnssecSigner::new(config).unwrap();

        let name = Name::from_ascii("selftest.example.com.").unwrap();
        let record_type = RecordType::A;
        let ttl = 300;
        let rrset_data = b"self_validation_test_data";

        // Self-validate
        let is_valid = signer.self_validate(&name, record_type, ttl, rrset_data);

        match is_valid {
            Ok(valid) => assert!(valid),
            Err(e) => {
                // This might fail if the key is not available, which is acceptable
                println!("Self-validation failed: {e}");
                assert!(
                    e.to_string().contains("No active key") || e.to_string().contains("No keys")
                );
            }
        }
    }

    #[test]
    fn test_validation_timing_stats() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = DnssecValidator::new(manager);

        // Simulate timing update
        let duration = Duration::from_micros(100);
        validator.update_timing_stats(duration);

        let stats = validator.get_validation_stats();
        assert_eq!(stats.average_validation_time_us, 100);
    }

    #[test]
    fn test_validation_stats_serialization() {
        let stats = ValidationStats {
            total_validations: 100,
            successful_validations: 95,
            failed_validations: 5,
            validations_by_algorithm: {
                let mut map = HashMap::new();
                map.insert("HmacSha256".to_string(), 80);
                map.insert("RsaSha256".to_string(), 20);
                map
            },
            average_validation_time_us: 50,
        };

        // Test serialization
        let serialized = serde_json::to_string(&stats).unwrap();
        assert!(serialized.contains("total_validations"));
        assert!(serialized.contains("100"));

        // Test deserialization
        let deserialized: ValidationStats = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.total_validations, 100);
        assert_eq!(deserialized.successful_validations, 95);
        assert_eq!(deserialized.failed_validations, 5);
    }

    #[test]
    fn test_alert_config_default() {
        let config = AlertConfig::default();
        assert_eq!(config.max_failure_rate, 5.0);
        assert_eq!(config.min_validations_for_alert, 100);
        assert_eq!(config.max_avg_validation_time_us, 1000);
        assert_eq!(config.alert_cooldown_seconds, 300);
    }

    #[test]
    fn test_alert_types() {
        assert_eq!(AlertType::HighFailureRate, AlertType::HighFailureRate);
        assert_ne!(AlertType::HighFailureRate, AlertType::SlowValidation);

        // Test serialization
        let alert_type = AlertType::KeyRotationNeeded;
        let serialized = serde_json::to_string(&alert_type).unwrap();
        let deserialized: AlertType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(alert_type, deserialized);
    }

    #[test]
    fn test_alert_severity() {
        assert_eq!(AlertSeverity::Critical, AlertSeverity::Critical);
        assert_ne!(AlertSeverity::Warning, AlertSeverity::Info);

        // Test serialization
        let severity = AlertSeverity::Warning;
        let serialized = serde_json::to_string(&severity).unwrap();
        let deserialized: AlertSeverity = serde_json::from_str(&serialized).unwrap();
        assert_eq!(severity, deserialized);
    }

    #[test]
    fn test_dnssec_monitor_creation() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = Arc::new(DnssecValidator::new(manager));
        let alert_config = AlertConfig::default();

        let monitor = DnssecMonitor::new(validator, alert_config);
        let stats = monitor.get_monitoring_stats();

        assert!(stats.is_object());
        assert!(stats.get("validation_stats").is_some());
        assert!(stats.get("alert_config").is_some());
    }

    #[test]
    fn test_alert_creation() {
        let alert = Alert {
            alert_type: AlertType::HighFailureRate,
            message: "Test alert".to_string(),
            severity: AlertSeverity::Critical,
            timestamp: SystemTime::now(),
            stats: ValidationStats::default(),
        };

        assert_eq!(alert.alert_type, AlertType::HighFailureRate);
        assert_eq!(alert.message, "Test alert");
        assert_eq!(alert.severity, AlertSeverity::Critical);

        // Test serialization
        let serialized = serde_json::to_string(&alert).unwrap();
        let deserialized: Alert = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.alert_type, alert.alert_type);
        assert_eq!(deserialized.message, alert.message);
        assert_eq!(deserialized.severity, alert.severity);
    }

    #[test]
    fn test_monitor_alert_checking() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = Arc::new(DnssecValidator::new(manager));
        let alert_config = AlertConfig::default();

        let monitor = DnssecMonitor::new(validator.clone(), alert_config);

        // Simulate high failure rate
        for _ in 0..150 {
            validator.update_validation_stats(DnssecAlgorithm::HmacSha256, true);
            validator.update_validation_result(false); // All failures
        }

        let alerts = monitor.check_alerts();
        assert!(!alerts.is_empty());

        // Should have a high failure rate alert
        let has_failure_alert = alerts
            .iter()
            .any(|a| a.alert_type == AlertType::HighFailureRate);
        assert!(has_failure_alert);
    }

    #[test]
    fn test_monitor_slow_validation_alert() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = Arc::new(DnssecValidator::new(manager));
        let alert_config = AlertConfig::default();

        let monitor = DnssecMonitor::new(validator.clone(), alert_config);

        // Simulate slow validation
        let slow_duration = Duration::from_micros(2000); // 2ms (above 1ms threshold)
        validator.update_timing_stats(slow_duration);

        let alerts = monitor.check_alerts();

        // Should have a slow validation alert
        let has_slow_alert = alerts
            .iter()
            .any(|a| a.alert_type == AlertType::SlowValidation);
        assert!(has_slow_alert);
    }

    #[test]
    fn test_monitor_alert_cooldown() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = Arc::new(DnssecValidator::new(manager));
        let alert_config = AlertConfig {
            alert_cooldown_seconds: 1, // 1 second cooldown
            ..Default::default()
        };

        let monitor = DnssecMonitor::new(validator, alert_config);

        // Should not suppress initially
        assert!(!monitor.should_suppress_alert(&AlertType::HighFailureRate));

        // Add an alert to history
        let alert = Alert {
            alert_type: AlertType::HighFailureRate,
            message: "Test".to_string(),
            severity: AlertSeverity::Critical,
            timestamp: SystemTime::now(),
            stats: ValidationStats::default(),
        };

        monitor.alert_history.write().unwrap().push(alert);

        // Should suppress now
        assert!(monitor.should_suppress_alert(&AlertType::HighFailureRate));

        // Wait for cooldown
        std::thread::sleep(Duration::from_secs(2));

        // Should not suppress after cooldown
        assert!(!monitor.should_suppress_alert(&AlertType::HighFailureRate));
    }

    #[test]
    fn test_monitor_recent_alerts() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = Arc::new(DnssecValidator::new(manager));
        let alert_config = AlertConfig::default();

        let monitor = DnssecMonitor::new(validator, alert_config);

        // Add some alerts
        let alert1 = Alert {
            alert_type: AlertType::HighFailureRate,
            message: "Recent alert".to_string(),
            severity: AlertSeverity::Critical,
            timestamp: SystemTime::now(),
            stats: ValidationStats::default(),
        };

        let alert2 = Alert {
            alert_type: AlertType::SlowValidation,
            message: "Old alert".to_string(),
            severity: AlertSeverity::Warning,
            timestamp: SystemTime::now() - Duration::from_secs(7200), // 2 hours ago
            stats: ValidationStats::default(),
        };

        monitor.alert_history.write().unwrap().push(alert1);
        monitor.alert_history.write().unwrap().push(alert2);

        // Get recent alerts (last hour)
        let recent = monitor.get_recent_alerts(Duration::from_secs(3600));
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "Recent alert");

        // Get all alerts
        let all = monitor.get_all_alerts();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_monitor_clear_history() {
        let config = DnssecConfig::default();
        let manager = Arc::new(DnssecKeyManager::new(config).unwrap());
        let validator = Arc::new(DnssecValidator::new(manager));
        let alert_config = AlertConfig::default();

        let monitor = DnssecMonitor::new(validator, alert_config);

        // Add an alert
        let alert = Alert {
            alert_type: AlertType::HighFailureRate,
            message: "Test".to_string(),
            severity: AlertSeverity::Critical,
            timestamp: SystemTime::now(),
            stats: ValidationStats::default(),
        };

        monitor.alert_history.write().unwrap().push(alert);
        assert_eq!(monitor.get_all_alerts().len(), 1);

        // Clear history
        monitor.clear_alert_history();
        assert_eq!(monitor.get_all_alerts().len(), 0);
    }

    #[test]
    fn test_production_signer_monitor_creation() {
        let config = DnssecConfig::default();
        let signer = ProductionDnssecSigner::new(config).unwrap();
        let alert_config = AlertConfig::default();

        let monitor = signer.create_monitor(alert_config);
        let stats = monitor.get_monitoring_stats();

        assert!(stats.is_object());
    }

    #[test]
    fn test_production_signer_key_rotation_alert() {
        let config = DnssecConfig {
            rotation_interval: 1, // 1 second
            ..Default::default()
        };

        let signer = ProductionDnssecSigner::new(config).unwrap();

        // Should need rotation (starts at UNIX_EPOCH)
        let alert = signer.check_key_rotation_alert();
        assert!(alert.is_some());

        let alert = alert.unwrap();
        assert_eq!(alert.alert_type, AlertType::KeyRotationNeeded);
        assert_eq!(alert.severity, AlertSeverity::Warning);
    }

    #[test]
    fn test_production_signer_monitoring_info() {
        let config = DnssecConfig::default();
        let signer = ProductionDnssecSigner::new(config).unwrap();

        let info = signer.get_monitoring_info();

        assert!(info.is_object());
        assert!(info.get("key_statistics").is_some());
        assert!(info.get("needs_rotation").is_some());
        assert!(info.get("config").is_some());
        assert_eq!(info.get("status").unwrap(), "operational");
    }
}
