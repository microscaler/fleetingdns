# FleetingDNS Crossplane Infrastructure

This directory contains the complete Crossplane infrastructure-as-code implementation for FleetingDNS, providing enterprise-grade global infrastructure with advanced security, observability, and networking capabilities.

## 🏗️ Architecture Overview

### Infrastructure Components

- **Projects**: 8 GCP projects for different environments and purposes
- **Networking**: Global load balancers, anycast IPs, SSL certificates, CDN
- **Security**: Cloud Armor WAF, IAP, Binary Authorization, Security Command Center
- **Observability**: BigQuery data warehouse, Cloud Logging, Cloud Monitoring
- **Compute**: Multi-region GKE clusters (control-plane and workload separation)

### Global Infrastructure

```
FleetingDNS Global Infrastructure
├── Control Plane (fleetingdns-infra)
│   ├── Global Load Balancers (HTTPS + TCP)
│   ├── Cloud Armor WAF Policies
│   ├── SSL Certificate Management
│   └── Global IP Addresses (IPv4 + IPv6)
├── Analytics (fleetingdns-analytics)
│   ├── BigQuery Data Warehouse
│   ├── ML Pipelines & Models
│   └── Real-time Analytics
├── Security (fleetingdns-security)
│   ├── Security Command Center
│   ├── Binary Authorization
│   └── Vulnerability Scanning
└── Multi-Region Workloads
    ├── Europe (fleetingdns-prod-eu)
    ├── Americas (fleetingdns-prod-us)
    └── Asia-Pacific (fleetingdns-prod-apac)
```

## 📁 Directory Structure

```
k8s-tilt/infra/
├── base/                           # Core Crossplane configuration
│   ├── provider-config.yaml       # GCP provider setup
│   ├── xrds/                      # Custom Resource Definitions
│   │   ├── xproject.yaml          # Project provisioning
│   │   ├── xloadbalancer.yaml     # Load balancer management
│   │   └── xsecurity.yaml         # Security configuration
│   └── compositions/               # Reusable compositions
│       ├── project-composition.yaml
│       ├── loadbalancer-composition.yaml
│       └── security-composition.yaml
├── org/                           # Organization structure
│   └── projects/                  # GCP project definitions
│       └── fleetingdns-projects.yaml
├── networking/                    # Global networking
│   ├── global-ips/               # Anycast IP addresses
│   ├── ssl-certificates/         # Google-managed SSL certs
│   └── load-balancers/           # Global LB configurations
├── security/                     # Security policies
│   └── cloud-armor/              # WAF and DDoS protection
└── observability/                # Analytics and monitoring
    └── bigquery/                 # Data warehouse setup
```

## 🚀 Quick Start

### Prerequisites

1. **Crossplane Installation**: Ensure Crossplane is installed in your cluster
2. **GCP Credentials**: Configure service account with appropriate permissions
3. **Provider Installation**: GCP provider must be installed and configured

### Deployment Order

1. **Base Configuration**:
   ```bash
   kubectl apply -k k8s-tilt/infra/base/
   ```

2. **Organization & Projects**:
   ```bash
   kubectl apply -k k8s-tilt/infra/org/
   ```

3. **Security Policies**:
   ```bash
   kubectl apply -k k8s-tilt/infra/security/
   ```

4. **Networking Infrastructure**:
   ```bash
   kubectl apply -k k8s-tilt/infra/networking/
   ```

5. **Observability Stack**:
   ```bash
   kubectl apply -k k8s-tilt/infra/observability/
   ```

### All-in-One Deployment

```bash
kubectl apply -k k8s-tilt/infra/
```

## 🔧 Configuration

### Required Configuration Updates

Before deployment, update the following placeholders in `org/projects/fleetingdns-projects.yaml`:

```yaml
# Replace these values with your actual GCP organization details
billingAccount: "YOUR-BILLING-ACCOUNT-ID"
organizationId: "YOUR-ORGANIZATION-ID"
folderId: "folders/YOUR-FOLDER-ID"
```

### Provider Configuration

The base provider configuration expects a secret named `gcp-credentials` in the `crossplane-system` namespace:

```bash
kubectl create secret generic gcp-credentials \
  --from-file=credentials=path/to/service-account.json \
  -n crossplane-system
```

## 🛡️ Security Features

### Cloud Armor WAF Policies

- **Main Policy**: General web application protection
- **DNS Policy**: DNS-specific DDoS protection
- **Admin Policy**: Strict access control for admin interfaces
- **Bot Protection**: Advanced bot detection and mitigation

### Identity-Aware Proxy (IAP)

- OAuth 2.0 integration with Google Workspace
- Multi-factor authentication enforcement
- Fine-grained access control policies

### Binary Authorization

- Container image verification
- Policy-based deployment controls
- Attestation-based security

## 🌐 Global Load Balancing

### HTTPS Load Balancers

- **Main LB**: Web services with Cloud CDN
- **API LB**: API endpoints with enhanced security
- **Admin LB**: Management interfaces with IAP

### TCP Load Balancer

- **DNS LB**: Global DNS traffic distribution
- **Health Checks**: Application-aware monitoring

### SSL Certificate Management

- Google-managed certificates with automatic renewal
- Wildcard and specific domain certificates
- Regional certificate distribution

## 📊 Observability & Analytics

### BigQuery Data Warehouse

- **Raw Data**: 90-day retention for ingestion
- **Processed Data**: 180-day retention for analytics
- **Analytics Data**: 365-day retention for ML models
- **Security Data**: Long-term threat intelligence storage

### Data Pipeline

- Real-time log ingestion from Cloud Logging
- Automated data transformation and enrichment
- ML-powered anomaly detection and alerting

## 💰 Cost Optimization

### Estimated Monthly Costs (Production)

| Component | Cost (EUR/month) |
|-----------|------------------|
| Infrastructure | €306.00 |
| Observability | €269.00 |
| Security | €259.00 |
| ML/Analytics | €250.00 |
| **Total** | **€1,084.00** |

### Cost Attribution

- Automatic cost labeling by environment, team, and component
- BigQuery cost analysis and optimization recommendations
- Budget alerts and spending notifications

## 🔄 Maintenance & Operations

### Monitoring

All resources include comprehensive labels and annotations for:
- Cost attribution and tracking
- Security policy enforcement
- Operational monitoring and alerting

### Updates

The infrastructure follows GitOps principles:
1. Update manifests in this repository
2. Apply changes via kubectl or Flux
3. Crossplane reconciles the desired state

### Backup & Recovery

- Infrastructure state stored in Kubernetes etcd
- Configuration versioned in Git
- Automated backup of critical data to Cloud Storage

## 🚨 Security Considerations

### Network Security

- VPC Flow Logs enabled for all networks
- Private Google Access for secure API communication
- Firewall rules following least-privilege principle

### Data Protection

- Encryption at rest for all data stores
- Encryption in transit with TLS 1.3
- GDPR compliance with data residency controls

### Access Control

- Workload Identity for secure service-to-service communication
- IAM policies following principle of least privilege
- Regular access reviews and audit logging

## 📚 Additional Resources

- [Crossplane Implementation PRD](./crossplane_implementation_prd.md)
- [FleetingDNS Architecture Documentation](../docs/)
- [Security Policies and Procedures](./security/)
- [Monitoring and Alerting Setup](./observability/)

## 🤝 Contributing

When adding new infrastructure components:

1. Create appropriate XRDs for new resource types
2. Implement compositions for reusable patterns
3. Add comprehensive labels and annotations
4. Update documentation and cost estimates
5. Test in development environment first

## 📞 Support

For infrastructure issues or questions:
- Create an issue in this repository
- Contact the Platform Engineering team
- Check the monitoring dashboards for system status

---

**⚡ FleetingDNS Infrastructure - Built with Crossplane for Enterprise Scale** 