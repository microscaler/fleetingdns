#!/usr/bin/env python3
"""
FleetingDNS Test Client
This script allows you to test DNS queries against the FleetingDNS service
"""

import socket
import struct
import random
import sys
import argparse
import subprocess
import json

def build_dns_query(domain):
    """Build a DNS query packet for the given domain"""
    # Generate random ID
    query_id = random.randint(1, 65535)
    
    # DNS Header
    flags = 0x0100  # Standard query
    qdcount = 1     # One question
    ancount = 0     # No answers
    nscount = 0     # No authority records
    arcount = 0     # No additional records
    
    header = struct.pack('>HHHHHH', query_id, flags, qdcount, ancount, nscount, arcount)
    
    # DNS Question
    question = b''
    for part in domain.split('.'):
        question += struct.pack('B', len(part)) + part.encode('ascii')
    question += b'\x00'  # Null terminator
    question += struct.pack('>HH', 1, 1)  # Type A, Class IN
    
    return header + question, query_id

def parse_dns_response(response, query_id):
    """Parse DNS response and extract IP addresses"""
    if len(response) < 12:
        return None, "Response too short"
    
    # Parse header
    resp_id, flags, qdcount, ancount, nscount, arcount = struct.unpack('>HHHHHH', response[:12])
    
    if resp_id != query_id:
        return None, "Response ID mismatch"
    
    # Skip the question section
    offset = 12
    for _ in range(qdcount):
        while offset < len(response) and response[offset] != 0:
            offset += response[offset] + 1
        offset += 5  # Skip null terminator + type + class
    
    # Parse answers
    ips = []
    for _ in range(ancount):
        if offset >= len(response):
            break
            
        # Skip name (could be compressed)
        if response[offset] & 0xC0 == 0xC0:
            offset += 2
        else:
            while offset < len(response) and response[offset] != 0:
                offset += response[offset] + 1
            offset += 1
        
        if offset + 10 > len(response):
            break
            
        # Parse type, class, ttl, length
        rtype, rclass, ttl, rdlength = struct.unpack('>HHIH', response[offset:offset+10])
        offset += 10
        
        # If it's an A record, extract the IP
        if rtype == 1 and rdlength == 4:
            ip = '.'.join(str(b) for b in response[offset:offset+4])
            ips.append(ip)
        
        offset += rdlength
    
    return ips, None

def query_dns_docker(domain, dns_server='dnsd', port=6353):
    """Query DNS using Docker (works around macOS limitations)"""
    cmd = [
        'docker', 'run', '--rm', '--network', 'fleetingdns_default',
        'alpine', 'sh', '-c',
        f'apk add --no-cache bind-tools >/dev/null 2>&1 && dig @{dns_server} -p {port} {domain} A +short'
    ]
    
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode == 0:
        ips = [line.strip() for line in result.stdout.strip().split('\n') if line.strip()]
        return ips
    else:
        return None

def add_redis_slot(domain, ip):
    """Add a slot to Redis"""
    cmd = [
        'docker', 'compose', 'exec', '-T', 'redis',
        'redis-cli', 'SET', f'slot:{domain}', ip
    ]
    
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.returncode == 0

def main():
    parser = argparse.ArgumentParser(description='Test FleetingDNS service')
    parser.add_argument('action', choices=['query', 'add', 'test'], 
                        help='Action to perform')
    parser.add_argument('domain', nargs='?', help='Domain to query or add')
    parser.add_argument('--ip', help='IP address for add action')
    parser.add_argument('--docker', action='store_true', default=True,
                        help='Use Docker for queries (default, works on macOS)')
    
    args = parser.parse_args()
    
    if args.action == 'query':
        if not args.domain:
            print("Error: domain required for query action")
            sys.exit(1)
            
        print(f"🔍 Querying {args.domain}...")
        
        if args.docker:
            ips = query_dns_docker(args.domain)
            if ips:
                print(f"✅ Response: {', '.join(ips)}")
            else:
                print(f"❌ No response or domain not found")
        else:
            # Direct UDP query (won't work on macOS localhost)
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.settimeout(5)
            
            query, query_id = build_dns_query(args.domain)
            sock.sendto(query, ('127.0.0.1', 6353))
            
            try:
                response, addr = sock.recvfrom(1024)
                ips, error = parse_dns_response(response, query_id)
                if ips:
                    print(f"✅ Response: {', '.join(ips)}")
                else:
                    print(f"❌ Error: {error or 'No records found'}")
            except socket.timeout:
                print("❌ Query timeout")
    
    elif args.action == 'add':
        if not args.domain or not args.ip:
            print("Error: domain and --ip required for add action")
            sys.exit(1)
            
        print(f"➕ Adding {args.domain} -> {args.ip}")
        if add_redis_slot(args.domain, args.ip):
            print("✅ Added successfully")
        else:
            print("❌ Failed to add")
    
    elif args.action == 'test':
        print("🚀 Running FleetingDNS integration tests...")
        
        # Test data
        test_domains = [
            ("test.fdns.run", "192.168.1.100"),
            ("app1.fdns.run", "10.0.0.1"),
            ("app2.fdns.run", "10.0.0.2"),
            ("webhook.fdns.run", "172.16.0.50"),
        ]
        
        # Add test data
        print("\n📝 Adding test data to Redis...")
        for domain, ip in test_domains:
            if add_redis_slot(domain, ip):
                print(f"  ✅ {domain} -> {ip}")
            else:
                print(f"  ❌ Failed to add {domain}")
        
        # Query all domains
        print("\n🔍 Testing DNS queries...")
        for domain, expected_ip in test_domains:
            ips = query_dns_docker(domain)
            if ips and expected_ip in ips:
                print(f"  ✅ {domain} -> {ips[0]} (correct)")
            else:
                print(f"  ❌ {domain} -> {ips[0] if ips else 'No response'} (expected {expected_ip})")
        
        # Test non-existent domain
        print("\n🔍 Testing non-existent domain...")
        ips = query_dns_docker("nonexistent.fdns.run")
        if not ips:
            print(f"  ✅ nonexistent.fdns.run -> No response (correct)")
        else:
            print(f"  ❌ nonexistent.fdns.run -> {ips[0]} (should not exist)")
        
        print("\n✅ Tests completed!")

if __name__ == '__main__':
    main()