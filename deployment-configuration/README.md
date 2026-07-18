# FleetingDNS deployment profiles

FleetingDNS product configuration lives under:

```text
deployment-configuration/profiles/<environment>/fleetingdns/<suite>/
```

The core dev profile is `deployment-configuration/profiles/dev/fleetingdns/core/`:

```text
core/
├── runtime/       # namespace-local ConfigMap + application Secrets (fleetingdns ns)
├── bootstrap/     # rerunnable database Job in the data namespace
└── kustomization.yaml
```

This mirrors the RERP pattern used by the `hauliage` product
(`hauliage/deployment-configuration/profiles/dev/hauliage/core/`).

## Secrets (SOPS + age)

Secrets live in `application.secrets.env` (dotenv) files, encrypted with
[SOPS](https://github.com/getsops/sops) using the shared-k8s Flux **age**
recipient. Non-secret config lives beside them in `application.properties`
(plaintext, rendered into a ConfigMap by Kustomize).

- **Encrypt / edit only on ms02**, where the private key lives:

  ```bash
  export SOPS_AGE_KEY_FILE=~/.config/sops/age/flux-shared-gitops
  sops deployment-configuration/profiles/dev/fleetingdns/core/runtime/application.secrets.env
  # or, for a freshly written plaintext file:
  sops --encrypt --in-place <path>/application.secrets.env
  ```

- Creation rules (recipient, path regex) are in the repo-root `.sops.yaml`.
- Kustomize `secretGenerator` reads the (decrypted-by-Flux) env file into a
  Secret; workloads consume it via `secretKeyRef`. `disableNameSuffixHash: true`
  keeps the Secret name stable for those references.

The dev values committed here are **local development defaults**, not production
credentials — but they are still SOPS-encrypted so the repo never carries a
plaintext password and secret-scanners (GitGuardian) stay green.

## Ownership (RERP pattern)

| Owner | Owns |
|-------|------|
| **Flux** | Runtime ConfigMaps/SOPS Secrets, DB bootstrap Job, HelmReleases, prune/drift |
| **Tilt** | Rust+Docker build, push dev images, local dev loop |

Migration note: the legacy inline `PGPASSWORD`/`DATABASE_URL` literals in the
`Tiltfile` and `k8s-tilt/` manifests are being moved here. Until fully migrated,
both may coexist; new secrets go through SOPS only.
