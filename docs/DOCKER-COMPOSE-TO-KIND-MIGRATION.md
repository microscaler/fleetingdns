# Docker Compose to Kind/Tilt Migration Guide

## 🔄 Quick Migration Checklist

### 1. Prerequisites
- [ ] Install Docker, Kind, kubectl, Tilt
- [ ] Run `just check-deps` to verify installations

### 2. Setup
```bash
# Old workflow
docker-compose up

# New workflow
just dev  # or: just setup && just up
```

### 3. Daily Development

| Task | Docker Compose | Kind + Tilt |
|------|----------------|-------------|
| **Start environment** | `docker-compose up` | `make up` or `tilt up` |
| **Stop environment** | `docker-compose down` | `make down` or `tilt down` |
| **View logs** | `docker-compose logs dnsd` | `make logs-dnsd` |
| **Restart service** | `docker-compose restart dnsd` | `make restart-dnsd` |
| **Build service** | `docker-compose build dnsd` | Auto-rebuild on code changes |
| **Clean everything** | `docker-compose down -v` | `make clean` |

### 4. Port Mappings

| Service | Docker Compose | Kind + Tilt |
|---------|----------------|-------------|
| DNS Server | `localhost:5353` | `localhost:5353` ✅ |
| EdgeHub | `localhost:2222` | `localhost:2222` ✅ |
| API | `localhost:8880` | `localhost:8880` ✅ |
| Redis | `localhost:6379` | `localhost:6379` ✅ |
| PostgreSQL | `localhost:5432` | `localhost:5432` ✅ |
| Grafana | `localhost:3000` | `localhost:3000` ✅ |
| Prometheus | `localhost:9090` | `localhost:9090` ✅ |

### 5. Environment Variables

Most environment variables remain the same, but service discovery is now Kubernetes-native:

```bash
# Old
REDIS_URL=redis://redis:6379

# New (same, but with K8s DNS)
REDIS_URL=redis://redis:6379  # Works within cluster
```

### 6. Development Workflow Changes

#### Code Changes
- **Docker Compose**: Manual rebuild required
- **Kind + Tilt**: Automatic rebuild and redeploy on file changes

#### Debugging
```bash
# Old
docker exec -it fleetingdns_dnsd_1 /bin/sh

# New
make shell-dnsd
# or: kubectl exec -it -n fleetingdns deployment/dnsd -- /bin/sh
```

#### Logs
```bash
# Old
docker-compose logs -f dnsd

# New
make logs-dnsd
# or: tilt logs dnsd
```

### 7. New Features Available

#### Observability
- **Grafana**: Pre-configured with dashboards
- **Prometheus**: Automatic service discovery
- **Loki**: Centralized logging
- **OpenTelemetry**: Distributed tracing

#### Development Tools
- **Tilt UI**: Visual service management at `localhost:10350`
- **Live Reload**: Instant code changes without manual rebuilds
- **Resource Management**: Proper CPU/memory limits
- **Health Checks**: Kubernetes-native liveness/readiness probes

### 8. Common Commands

```bash
# Quick start everything
make dev

# Check status
make status

# View all URLs
make urls

# Open Grafana
make grafana

# Run tests
make test

# Reset everything
make reset
```

### 9. Troubleshooting

#### Port Conflicts
```bash
# Check what's using a port
lsof -i :5353

# Kill process
kill -9 $(lsof -t -i:5353)
```

#### Service Not Starting
```bash
# Check pod status
kubectl get pods -n fleetingdns

# Describe pod
make describe-dnsd

# View events
kubectl get events -n fleetingdns
```

#### Build Issues
```bash
# Force rebuild
make restart-dnsd

# Check build logs
tilt logs dnsd
```

### 10. Performance Comparison

| Aspect | Docker Compose | Kind + Tilt |
|--------|----------------|-------------|
| **Cold Start** | ~30 seconds | ~60 seconds (first time) |
| **Hot Reload** | Manual rebuild | ~5-10 seconds |
| **Resource Usage** | Higher (no limits) | Lower (with limits) |
| **Production Parity** | Low | High |
| **Debugging** | Basic | Advanced |

### 11. Migration Benefits

✅ **Production Parity**: Same environment as production  
✅ **Better Observability**: Built-in monitoring stack  
✅ **Faster Development**: Live reload and incremental builds  
✅ **Resource Efficiency**: Proper resource limits  
✅ **Better Testing**: Kubernetes-native testing  
✅ **Team Consistency**: Same environment for everyone  

### 12. What's Removed

❌ **Docker Compose files**: No longer needed  
❌ **Manual port management**: Handled by Kubernetes  
❌ **Manual service dependencies**: Handled by Tilt  
❌ **Manual health checks**: Kubernetes probes  

## 🚀 Ready to Switch?

1. **Backup current work**: Commit any uncommitted changes
2. **Run setup**: `make dev`
3. **Verify services**: `make status`
4. **Test functionality**: Run your usual development tasks
5. **Update workflows**: Use new commands from this guide

## 🆘 Need Help?

- Check `KIND-TILT-SETUP.md` for detailed documentation
- Run `make help` for all available commands
- Use `make health` to diagnose issues
- Check Tilt UI at `localhost:10350` for visual debugging 