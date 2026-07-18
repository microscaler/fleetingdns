#!/usr/bin/env python3
"""
Kind cluster setup script for FleetingDNS development.
Creates a Kind cluster with local registry and proper networking.
"""

import subprocess
import sys
import time
import json
import socket
from pathlib import Path


def run_command(cmd, check=True, capture_output=True):
    """Run a shell command and return the result."""
    print(f"🔧 Running: {' '.join(cmd) if isinstance(cmd, list) else cmd}")
    try:
        if isinstance(cmd, str):
            cmd = cmd.split()
        result = subprocess.run(cmd, check=check, capture_output=capture_output, text=True)
        if result.stdout and capture_output:
            print(f"✅ Output: {result.stdout.strip()}")
        return result
    except subprocess.CalledProcessError as e:
        print(f"❌ Error: {e}")
        if e.stdout:
            print(f"📤 Stdout: {e.stdout}")
        if e.stderr:
            print(f"📥 Stderr: {e.stderr}")
        if check:
            raise
        return e


def check_dependencies():
    """Check if required tools are installed."""
    print("🔍 Checking dependencies...")
    
    dependencies = {
        'kind': 'kind version',
        'docker': 'docker version',
        'kubectl': 'kubectl version --client',
        'tilt': 'tilt version'
    }
    
    missing = []
    for tool, cmd in dependencies.items():
        try:
            run_command(cmd.split(), capture_output=True)
            print(f"✅ {tool} is installed")
        except subprocess.CalledProcessError:
            print(f"❌ {tool} is not installed or not in PATH")
            missing.append(tool)
    
    if missing:
        print(f"\n❌ Missing dependencies: {', '.join(missing)}")
        print("\n📦 Install missing tools:")
        if 'kind' in missing:
            print("  • Kind: https://kind.sigs.k8s.io/docs/user/quick-start/#installation")
        if 'docker' in missing:
            print("  • Docker: https://docs.docker.com/get-docker/")
        if 'kubectl' in missing:
            print("  • kubectl: https://kubernetes.io/docs/tasks/tools/install-kubectl/")
        if 'tilt' in missing:
            print("  • Tilt: https://docs.tilt.dev/install.html")
        sys.exit(1)


def check_port_available(port):
    """Check if a port is available."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        try:
            s.bind(('localhost', port))
            return True
        except OSError:
            return False


def setup_local_registry():
    """Set up local Docker registry for Kind."""
    print("🐳 Setting up local Docker registry...")
    
    # Check if registry is already running
    try:
        result = run_command(['docker', 'ps', '--filter', 'name=kind-registry', '--format', '{{.Names}}'])
        if 'kind-registry' in result.stdout:
            print("✅ Local registry already running")
            return
    except subprocess.CalledProcessError:
        pass
    
    # Check if port 5001 is available
    if not check_port_available(5001):
        print("❌ Port 5001 is not available for local registry")
        sys.exit(1)
    
    # Start local registry
    registry_cmd = [
        'docker', 'run', '-d',
        '--restart=always',
        '-p', '127.0.0.1:5001:5000',
        '--name', 'kind-registry',
        'registry:2'
    ]
    
    try:
        run_command(registry_cmd)
        print("✅ Local registry started on localhost:5001")
    except subprocess.CalledProcessError:
        print("❌ Failed to start local registry")
        sys.exit(1)


def create_kind_cluster():
    """Create Kind cluster with proper configuration."""
    print("🎯 Creating Kind cluster...")
    
    cluster_name = "fleetingdns-dev"
    
    # Check if cluster already exists
    try:
        result = run_command(['kind', 'get', 'clusters'])
        if cluster_name in result.stdout:
            print(f"✅ Cluster '{cluster_name}' already exists")
            return
    except subprocess.CalledProcessError:
        pass
    
    # Create cluster
    config_path = Path(__file__).parent.parent / "kind-config.yaml"
    if not config_path.exists():
        print(f"❌ Kind config not found at {config_path}")
        sys.exit(1)
    
    create_cmd = [
        'kind', 'create', 'cluster',
        '--config', str(config_path),
        '--wait', '60s'
    ]
    
    try:
        run_command(create_cmd, capture_output=False)
        print(f"✅ Cluster '{cluster_name}' created successfully")
    except subprocess.CalledProcessError:
        print("❌ Failed to create Kind cluster")
        sys.exit(1)


def connect_registry_to_cluster():
    """Connect local registry to Kind cluster."""
    print("🔗 Connecting registry to cluster...")
    
    # Connect registry to cluster network
    try:
        run_command([
            'docker', 'network', 'connect',
            'kind', 'kind-registry'
        ])
        print("✅ Registry connected to Kind network")
    except subprocess.CalledProcessError:
        # Registry might already be connected
        print("ℹ️  Registry already connected to Kind network")
    
    # Apply registry config to cluster
    registry_config = """
apiVersion: v1
kind: ConfigMap
metadata:
  name: local-registry-hosting
  namespace: kube-public
data:
  localRegistryHosting.v1: |
    host: "localhost:5001"
    help: "https://kind.sigs.k8s.io/docs/user/local-registry/"
"""
    
    try:
        proc = subprocess.run(
            ['kubectl', 'apply', '-f', '-'],
            input=registry_config,
            text=True,
            check=True
        )
        print("✅ Registry configuration applied to cluster")
    except subprocess.CalledProcessError:
        print("❌ Failed to apply registry configuration")
        sys.exit(1)


def verify_setup():
    """Verify the setup is working correctly."""
    print("🔍 Verifying setup...")
    
    # Check cluster status
    try:
        result = run_command(['kubectl', 'cluster-info', '--context', 'kind-fleetingdns-dev'])
        print("✅ Cluster is accessible")
    except subprocess.CalledProcessError:
        print("❌ Cluster is not accessible")
        sys.exit(1)
    
    # Check nodes
    try:
        result = run_command(['kubectl', 'get', 'nodes'])
        print("✅ Nodes are ready")
    except subprocess.CalledProcessError:
        print("❌ Nodes are not ready")
        sys.exit(1)
    
    # Check registry
    try:
        result = run_command(['curl', '-s', 'http://localhost:5001/v2/_catalog'], check=False)
        if result.returncode == 0:
            print("✅ Local registry is accessible")
        else:
            print("⚠️  Local registry might not be fully ready yet")
    except FileNotFoundError:
        print("ℹ️  curl not available, skipping registry check")


def main():
    """Main setup function."""
    print("🚀 Setting up FleetingDNS development environment with Kind...")
    print("")
    
    try:
        check_dependencies()
        setup_local_registry()
        create_kind_cluster()
        connect_registry_to_cluster()
        
        # Wait a moment for everything to settle
        print("⏳ Waiting for cluster to stabilize...")
        time.sleep(10)
        
        verify_setup()
        
        print("")
        print("🎉 Setup complete!")
        print("")
        print("🔧 Next steps:")
        print("  1. Run 'tilt up' to start the development environment")
        print("  2. Access services at:")
        print("     • DNS Server: localhost:5353 (UDP)")
        print("     • EdgeHub: localhost:2222 (TCP)")
        print("     • Backend API: localhost:8880 (HTTP)")
        print("     • Grafana: localhost:3000 (HTTP)")
        print("     • Prometheus: localhost:9090 (HTTP)")
        print("")
        print("🧹 To clean up:")
        print("  • Run 'kind delete cluster --name fleetingdns-dev'")
        print("  • Run 'docker stop kind-registry && docker rm kind-registry'")
        print("")
        
    except KeyboardInterrupt:
        print("\n❌ Setup interrupted by user")
        sys.exit(1)
    except Exception as e:
        print(f"\n❌ Setup failed: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main() 