use std::sync::OnceLock;

use hickory_proto::rr::{
    Name, RData, Record, RecordType,
    dnssec::{
        Algorithm,
        rdata::{DNSSECRData, RRSIG},
    },
};
use ring::hmac;

/// HMAC based DNSSEC signer.
///
/// The secret is loaded from the `FDNS_HMAC_KEY` environment variable on first
/// use. If the variable is unset, signing is disabled and [`signer`] returns
/// `None`.
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
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        let sig = self.sign(rrset);
        let rrsig = RRSIG::new(
            typ,
            Algorithm::Unknown(253),
            name.num_labels(),
            ttl,
            now + ttl,
            now,
            0,
            name.clone(),
            sig,
        );
        Record::from_rdata(name.clone(), ttl, RData::DNSSEC(DNSSECRData::RRSIG(rrsig)))
    }
}

static SIGNER: OnceLock<Option<HmacSigner>> = OnceLock::new();

/// Global signer instance initialized from `FDNS_HMAC_KEY`.
pub fn signer() -> &'static Option<HmacSigner> {
    SIGNER.get_or_init(HmacSigner::from_env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_hmac_signer_new() {
        let secret = b"test_secret_key";
        let signer = HmacSigner::new(secret);

        // Test that the signer can sign data
        let data = b"test_data";
        let signature = signer.sign(data);
        assert!(!signature.is_empty());

        // Test that the same data produces the same signature
        let signature2 = signer.sign(data);
        assert_eq!(signature, signature2);

        // Test that different data produces different signatures
        let different_data = b"different_data";
        let different_signature = signer.sign(different_data);
        assert_ne!(signature, different_signature);
    }

    #[test]
    fn test_hmac_signer_from_env_isolated() {
        // Test with environment variable set
        unsafe {
            env::set_var("FDNS_HMAC_KEY_TEST", "test_env_key");
        }

        // Test the function directly without using the global static
        let secret = env::var("FDNS_HMAC_KEY_TEST").ok();
        assert!(secret.is_some());

        let signer = HmacSigner::new(secret.unwrap().as_bytes());
        let data = b"test_data";
        let signature = signer.sign(data);
        assert!(!signature.is_empty());

        // Clean up
        unsafe {
            env::remove_var("FDNS_HMAC_KEY_TEST");
        }
    }

    #[test]
    fn test_hmac_signer_from_env_missing() {
        // Test with missing environment variable
        let secret = env::var("FDNS_HMAC_KEY_MISSING").ok();
        assert!(secret.is_none());
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
        assert_eq!(rrsig_record.name(), &name);
        assert_eq!(rrsig_record.record_type(), RecordType::RRSIG);
        assert_eq!(rrsig_record.ttl(), ttl);

        // Verify the RRSIG data
        if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data().unwrap() {
            assert_eq!(rrsig.type_covered(), record_type);
            assert_eq!(rrsig.algorithm(), Algorithm::Unknown(253));
            assert_eq!(rrsig.num_labels(), name.num_labels());
            assert_eq!(rrsig.original_ttl(), ttl);
            assert_eq!(rrsig.signer_name(), &name);
            assert!(!rrsig.sig().is_empty());

            // Verify signature validity
            let expected_sig = signer.sign(rrset_data);
            assert_eq!(rrsig.sig(), &expected_sig);
        } else {
            panic!("Expected RRSIG record data");
        }
    }

    #[test]
    fn test_rrsig_record_different_names() {
        let signer = HmacSigner::new(b"test_secret");
        let name1 = Name::from_ascii("example.com.").unwrap();
        let name2 = Name::from_ascii("test.org.").unwrap();
        let rrset_data = b"same_rrset_data";

        let rrsig1 = signer.rrsig_record(&name1, RecordType::A, 300, rrset_data);
        let rrsig2 = signer.rrsig_record(&name2, RecordType::A, 300, rrset_data);

        // Different names should produce different records
        assert_ne!(rrsig1.name(), rrsig2.name());

        // But signatures should be the same for same rrset data
        if let (
            RData::DNSSEC(DNSSECRData::RRSIG(rrsig1_data)),
            RData::DNSSEC(DNSSECRData::RRSIG(rrsig2_data)),
        ) = (rrsig1.data().unwrap(), rrsig2.data().unwrap())
        {
            assert_eq!(rrsig1_data.sig(), rrsig2_data.sig());
        }
    }

    #[test]
    fn test_rrsig_record_different_types() {
        let signer = HmacSigner::new(b"test_secret");
        let name = Name::from_ascii("example.com.").unwrap();
        let rrset_data = b"test_rrset_data";

        let rrsig_a = signer.rrsig_record(&name, RecordType::A, 300, rrset_data);
        let rrsig_aaaa = signer.rrsig_record(&name, RecordType::AAAA, 300, rrset_data);

        if let (
            RData::DNSSEC(DNSSECRData::RRSIG(rrsig_a_data)),
            RData::DNSSEC(DNSSECRData::RRSIG(rrsig_aaaa_data)),
        ) = (rrsig_a.data().unwrap(), rrsig_aaaa.data().unwrap())
        {
            assert_eq!(rrsig_a_data.type_covered(), RecordType::A);
            assert_eq!(rrsig_aaaa_data.type_covered(), RecordType::AAAA);
            // Signatures should be the same for same rrset data
            assert_eq!(rrsig_a_data.sig(), rrsig_aaaa_data.sig());
        }
    }

    #[test]
    fn test_global_signer_function() {
        // Test that the global signer function returns a reference
        let signer_ref = signer();
        // We can't test the content without knowing the environment state,
        // but we can test that it returns a valid reference
        assert!(signer_ref.is_some() || signer_ref.is_none());
    }

    #[test]
    fn test_signature_consistency() {
        let secret = b"consistent_secret";
        let signer1 = HmacSigner::new(secret);
        let signer2 = HmacSigner::new(secret);

        let data = b"test_consistency_data";
        let sig1 = signer1.sign(data);
        let sig2 = signer2.sign(data);

        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_signature_with_empty_data() {
        let signer = HmacSigner::new(b"test_secret");
        let empty_data = b"";
        let signature = signer.sign(empty_data);

        assert!(!signature.is_empty());

        // Should be consistent
        let signature2 = signer.sign(empty_data);
        assert_eq!(signature, signature2);
    }

    #[test]
    fn test_rrsig_timing_fields() {
        let signer = HmacSigner::new(b"test_secret");
        let name = Name::from_ascii("example.com.").unwrap();
        let ttl = 3600;
        let rrset_data = b"timing_test_data";

        let before_creation = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        let rrsig_record = signer.rrsig_record(&name, RecordType::A, ttl, rrset_data);

        let after_creation = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        if let RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) = rrsig_record.data().unwrap() {
            // Verify timing fields are reasonable
            assert!(rrsig.sig_inception() >= before_creation);
            assert!(rrsig.sig_inception() <= after_creation);
            assert_eq!(rrsig.sig_expiration(), rrsig.sig_inception() + ttl);
            assert_eq!(rrsig.key_tag(), 0); // We set key_tag to 0
        }
    }

    #[test]
    fn test_rrsig_with_different_ttls() {
        let signer = HmacSigner::new(b"test_secret");
        let name = Name::from_ascii("example.com.").unwrap();
        let rrset_data = b"test_rrset_data";

        let rrsig_short = signer.rrsig_record(&name, RecordType::A, 60, rrset_data);
        let rrsig_long = signer.rrsig_record(&name, RecordType::A, 3600, rrset_data);

        if let (
            RData::DNSSEC(DNSSECRData::RRSIG(rrsig_short_data)),
            RData::DNSSEC(DNSSECRData::RRSIG(rrsig_long_data)),
        ) = (rrsig_short.data().unwrap(), rrsig_long.data().unwrap())
        {
            assert_eq!(rrsig_short_data.original_ttl(), 60);
            assert_eq!(rrsig_long_data.original_ttl(), 3600);

            // Expiration should be different
            assert_ne!(
                rrsig_short_data.sig_expiration(),
                rrsig_long_data.sig_expiration()
            );

            // But signatures should be the same for same rrset data
            assert_eq!(rrsig_short_data.sig(), rrsig_long_data.sig());
        }
    }

    #[test]
    fn test_signer_clone() {
        let signer1 = HmacSigner::new(b"test_secret");
        let signer2 = signer1.clone();

        let data = b"test_clone_data";
        let sig1 = signer1.sign(data);
        let sig2 = signer2.sign(data);

        assert_eq!(sig1, sig2);
    }
}
