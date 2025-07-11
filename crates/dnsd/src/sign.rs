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
