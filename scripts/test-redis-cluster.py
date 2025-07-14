#!/usr/bin/env python3
"""
Redis Cluster Integration Test Runner

This script sets up and runs Redis cluster integration tests using testcontainers.
It handles Docker environment setup and runs comprehensive cluster tests.
"""

import subprocess
import sys
import os
import time
from pathlib import Path

def check_docker():
    """Check if Docker is available and running"""
    try:
        result = subprocess.run(
            ["docker", "--version"], 
            capture_output=True, 
            text=True, 
            timeout=5
        )
        if result.returncode != 0:
            return False
        
        # Check if Docker daemon is running
        result = subprocess.run(
            ["docker", "ps"], 
            capture_output=True, 
            text=True, 
            timeout=5
        )
        return result.returncode == 0
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return False

def pull_redis_image():
    """Pull the Redis Docker image"""
    print("🐳 Pulling Redis Docker image...")
    try:
        result = subprocess.run(
            ["docker", "pull", "redis:7.0-alpine"],
            capture_output=True,
            text=True,
            timeout=300  # 5 minutes timeout
        )
        if result.returncode == 0:
            print("✅ Redis image pulled successfully")
            return True
        else:
            print(f"❌ Failed to pull Redis image: {result.stderr}")
            return False
    except subprocess.TimeoutExpired:
        print("❌ Timeout pulling Redis image")
        return False

def run_cluster_tests():
    """Run the Redis cluster integration tests"""
    print("🧪 Running Redis cluster integration tests...")
    
    # Change to project root
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    os.chdir(project_root)
    
    # Set environment variables
    env = os.environ.copy()
    env["RUST_LOG"] = "info"
    env["RUST_BACKTRACE"] = "1"
    
    # Run the tests
    cmd = [
        "cargo", "test", 
        "-p", "dnsd",
        "--test", "redis_cluster_integration",
        "--features", "redis-cluster-integration",
        "--", "--nocapture"
    ]
    
    try:
        result = subprocess.run(
            cmd,
            env=env,
            timeout=600  # 10 minutes timeout
        )
        return result.returncode == 0
    except subprocess.TimeoutExpired:
        print("❌ Tests timed out after 10 minutes")
        return False

def run_performance_benchmark():
    """Run performance benchmark tests"""
    print("📊 Running Redis cluster performance benchmark...")
    
    cmd = [
        "cargo", "test", 
        "-p", "dnsd",
        "--test", "redis_cluster_integration",
        "--features", "redis-cluster-integration",
        "test_cluster_performance_benchmark",
        "--", "--nocapture"
    ]
    
    try:
        result = subprocess.run(
            cmd,
            timeout=300  # 5 minutes timeout
        )
        return result.returncode == 0
    except subprocess.TimeoutExpired:
        print("❌ Performance benchmark timed out")
        return False

def main():
    """Main test runner"""
    print("🚀 Redis Cluster Integration Test Runner")
    print("=" * 50)
    
    # Check Docker availability
    if not check_docker():
        print("❌ Docker is not available or not running")
        print("Please install Docker and ensure it's running")
        sys.exit(1)
    
    print("✅ Docker is available and running")
    
    # Pull Redis image
    if not pull_redis_image():
        print("❌ Failed to pull Redis image")
        sys.exit(1)
    
    # Run cluster tests
    success = run_cluster_tests()
    
    if success:
        print("\n🎉 All Redis cluster integration tests passed!")
        
        # Run performance benchmark
        print("\n" + "=" * 50)
        if run_performance_benchmark():
            print("🎉 Performance benchmark completed successfully!")
        else:
            print("⚠️  Performance benchmark failed or timed out")
            
    else:
        print("\n❌ Some Redis cluster integration tests failed")
        sys.exit(1)

if __name__ == "__main__":
    main() 