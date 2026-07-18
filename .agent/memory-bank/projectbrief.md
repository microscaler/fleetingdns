# FleetingDNS Project Brief

## Project Overview
FleetingDNS is an ephemeral DNS forwarder and threat intelligence platform built in Rust. It provides secure, temporary DNS tunneling capabilities with integrated honeypot functionality for threat detection and analysis.

## Core Components

### DNS Infrastructure
- **dnsd**: Core DNS server with Redis caching and DNSSEC signing
- **edgehub**: Edge proxy for tunnel management and connection handling
- **common**: Shared utilities including graceful shutdown framework and TLS handling

### Data Pipeline
- **feed_grpc**: gRPC-based threat intelligence feed service
- **feed_webhook**: Webhook-based data ingestion
- **intake_collector**: Data collection and processing
- **ml_scorer**: Machine learning-based threat scoring

### Supporting Services
- **auth**: Authentication and authorization
- **backendapi**: Backend API services
- **metrics_client**: Observability and metrics collection
- **feature_pipe**: Feature processing pipeline

## Binary Applications
- **dnsd-bin**: Configurable DNS server binary
- **edgehub-bin**: Edge hub server binary
- **fleetingdns-ctl**: Control interface for operational management
- **slot-setter**: Redis slot management utility
- **api-bin**: Main API server binary

## Key Features
- Ephemeral DNS forwarding with automatic cleanup
- Secure tunneling with TLS/DoT support
- Redis-based caching and state management
- DNSSEC signing capabilities
- Graceful shutdown with POSIX-compliant Unix socket control
- Comprehensive observability and metrics
- Threat intelligence collection and analysis
- Machine learning-based threat scoring

## Technology Stack
- **Language**: Rust (stable toolchain)
- **Async Runtime**: Tokio
- **DNS**: Custom implementation with hickory-proto
- **Cache**: Redis with bb8 connection pooling
- **TLS**: rustls for secure connections
- **Metrics**: Prometheus-compatible metrics
- **Testing**: Comprehensive unit and integration tests

## Architecture
- Microservices architecture with clear separation of concerns
- Event-driven design with async/await patterns
- Modular crate structure for reusability
- Docker containerization for deployment
- Kubernetes-ready with Crossplane infrastructure

## Development Status
- Active development with TDD approach
- Current coverage: 46.25% (target: 65% minimum, 80% goal)
- Comprehensive CI/CD pipeline
- GitOps-based infrastructure deployment
- Production-ready with enterprise security features
