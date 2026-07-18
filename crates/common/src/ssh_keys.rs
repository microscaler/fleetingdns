//! Ephemeral SSH keypair generation for tunnel sessions (TDP-13).
//!
//! The control plane (`backendapi`) mints a real Ed25519 keypair per tunnel,
//! returns it to the CLI, and stores the public key's fingerprint in Redis so
//! the hub can authenticate the SSH connection against the issued key instead
//! of accepting any key (the former "Phase 0" behaviour).

use russh::keys::{Algorithm, HashAlg, PrivateKey};

/// A freshly generated Ed25519 SSH keypair in the formats the tunnel flow needs.
#[derive(Debug, Clone)]
pub struct GeneratedKeyPair {
    /// OpenSSH-format PEM private key (`-----BEGIN OPENSSH PRIVATE KEY-----`).
    pub private_key_openssh: String,
    /// OpenSSH `authorized_keys`-format public key (`ssh-ed25519 AAAA... `).
    pub public_key_openssh: String,
    /// SHA-256 fingerprint (`SHA256:...`) — the value the hub compares against.
    pub fingerprint: String,
}

/// Generate a new ephemeral Ed25519 SSH keypair.
///
/// Uses the ssh-key API re-exported by russh 0.60, so the fingerprint here
/// matches exactly what `RedisAuthHandler::compute_fingerprint` derives on the
/// hub from the presented public key.
pub fn generate_ed25519_keypair() -> anyhow::Result<GeneratedKeyPair> {
    let key = PrivateKey::random(&mut rand_key::rng(), Algorithm::Ed25519)?;

    let private_key_openssh = key
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)?
        .to_string();
    let public_key_openssh = key.public_key().to_openssh()?;
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();

    Ok(GeneratedKeyPair {
        private_key_openssh,
        public_key_openssh,
        fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_a_usable_ed25519_keypair() {
        let kp = generate_ed25519_keypair().expect("keygen");
        assert!(kp.private_key_openssh.contains("BEGIN OPENSSH PRIVATE KEY"));
        assert!(kp.public_key_openssh.starts_with("ssh-ed25519 "));
        assert!(kp.fingerprint.starts_with("SHA256:"));

        // The private key round-trips through the same decoder the CLI uses,
        // and its public key's fingerprint matches the reported one.
        let decoded = russh::keys::decode_secret_key(&kp.private_key_openssh, None)
            .expect("private key must decode");
        assert_eq!(
            decoded
                .public_key()
                .fingerprint(HashAlg::Sha256)
                .to_string(),
            kp.fingerprint
        );
    }

    #[test]
    fn keys_are_unique_per_call() {
        let a = generate_ed25519_keypair().unwrap();
        let b = generate_ed25519_keypair().unwrap();
        assert_ne!(a.fingerprint, b.fingerprint);
    }
}
