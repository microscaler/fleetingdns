# 🚨 DNS Zone Authority Gap Analysis

## **Critical Missing Infrastructure**

### **Problem Statement**
Our FleetingDNS DNS service is **NOT** functioning as a proper authoritative DNS server. We're missing the fundamental DNS zone management infrastructure required for subdomain delegation.

### **Current Broken State**
- ❌ **No SOA Records**: Missing Start of Authority records for `fleetingdns.run`
- ❌ **No NS Records**: Missing Name Server records for delegation  
- ❌ **No Zone Authority**: DNS server doesn't act as authoritative for the zone
- ❌ **No Subdomain Delegation**: Can't handle `casibbald.fleetingdns.run` → `127.0.0.1`

### **Required DNS Records**

#### **1. SOA (Start of Authority) Record**
```
fleetingdns.run.    IN  SOA  ns1.fleetingdns.run. admin.fleetingdns.run. (
    2025071601  ; Serial
    3600        ; Refresh
    1800        ; Retry  
    1209600     ; Expire
    300         ; Minimum TTL
)
```

#### **2. NS (Name Server) Records**
```
fleetingdns.run.    IN  NS   ns1.fleetingdns.run.
fleetingdns.run.    IN  NS   ns2.fleetingdns.run.
```

#### **3. A Records for Name Servers**
```
ns1.fleetingdns.run.    IN  A   34.102.136.180
ns2.fleetingdns.run.    IN  A   34.102.136.181
```

#### **4. Dynamic Subdomain Records**
```
casibbald.fleetingdns.run.    IN  A   127.0.0.1
test-user.fleetingdns.run.    IN  A   192.168.1.100
```

### **Implementation Requirements**

#### **Phase 1: Zone Authority Setup**
1. **SOA Record Generation**
   - Implement SOA record creation in `DnsHandler`
   - Add serial number management
   - Add zone transfer support

2. **NS Record Management**
   - Add NS record responses for zone queries
   - Implement proper delegation handling

3. **Zone Configuration**
   - Add zone configuration to DNS handler
   - Support multiple zones (fleetingdns.run, edf.run)

#### **Phase 2: Subdomain Delegation**
1. **Dynamic Record Generation**
   - Generate A/AAAA records for user subdomains
   - Support wildcard subdomains (`*.fleetingdns.run`)
   - Implement proper TTL management

2. **User Subdomain Management**
   - API endpoints for subdomain creation
   - Redis storage for subdomain mappings
   - Automatic cleanup of expired subdomains

#### **Phase 3: Advanced Features**
1. **DNSSEC Support**
   - Sign zone records with DNSSEC
   - Add RRSIG records
   - Support NSEC/NSEC3

2. **Zone Transfer**
   - Support AXFR/IXFR zone transfers
   - Implement secondary DNS servers

### **Technical Implementation**

#### **Required Code Changes**

1. **`crates/dnsd/src/zone_manager.rs`** (NEW)
   ```rust
   pub struct ZoneManager {
       zones: HashMap<String, ZoneConfig>,
       serial_numbers: Arc<RwLock<HashMap<String, u32>>>,
   }
   
   pub struct ZoneConfig {
       pub name: String,
       pub soa_record: SOARecord,
       pub ns_records: Vec<NSRecord>,
       pub ttl: u32,
   }
   ```

2. **`crates/dnsd/src/dns_handler.rs`** (UPDATE)
   - Add zone authority checking
   - Add SOA/NS record responses
   - Add subdomain record generation

3. **`crates/backendapi/src/handlers/subdomains.rs`** (NEW)
   - POST `/v1/subdomains` - Create subdomain
   - GET `/v1/subdomains/{username}` - Get user subdomains
   - DELETE `/v1/subdomains/{id}` - Delete subdomain

#### **Redis Schema Updates**
```redis
# Zone configuration
zone:fleetingdns.run = {
  "soa": {
    "mname": "ns1.fleetingdns.run",
    "rname": "admin.fleetingdns.run", 
    "serial": 2025071601,
    "refresh": 3600,
    "retry": 1800,
    "expire": 1209600,
    "minimum": 300
  },
  "ns": ["ns1.fleetingdns.run", "ns2.fleetingdns.run"]
}

# User subdomains
subdomain:casibbald.fleetingdns.run = {
  "ip": "127.0.0.1",
  "user_id": "user_123",
  "created_at": "2025-07-16T10:00:00Z",
  "expires_at": "2025-07-16T11:00:00Z",
  "ttl": 300
}
```

### **Testing Requirements**

#### **DNS Zone Authority Tests**
```bash
# Test SOA record
dig @localhost -p 6353 fleetingdns.run SOA

# Test NS records  
dig @localhost -p 6353 fleetingdns.run NS

# Test subdomain delegation
dig @localhost -p 6353 casibbald.fleetingdns.run A
```

#### **Integration Tests**
1. **Zone Authority Test**
   - Verify SOA record responses
   - Verify NS record responses
   - Verify zone transfer support

2. **Subdomain Delegation Test**
   - Create user subdomain via API
   - Verify DNS resolution
   - Test automatic cleanup

3. **Performance Test**
   - Test high-volume subdomain creation
   - Test concurrent DNS queries
   - Test zone transfer performance

### **Deployment Requirements**

#### **DNS Provider Configuration**
1. **Primary DNS Provider** (Cloudflare/Route53)
   - Delegate `fleetingdns.run` to our DNS servers
   - Configure NS records pointing to our servers

2. **Secondary DNS Servers**
   - Deploy multiple DNS servers for redundancy
   - Configure zone transfers between servers

#### **Infrastructure Updates**
1. **Load Balancer Configuration**
   - Configure UDP/TCP port 53 for DNS traffic
   - Set up health checks for DNS servers

2. **Monitoring & Alerting**
   - Monitor DNS query volumes
   - Alert on zone transfer failures
   - Monitor subdomain creation rates

### **Priority Implementation Order**

#### **HIGH PRIORITY (Week 1)**
1. ✅ **SOA Record Implementation**
2. ✅ **NS Record Implementation** 
3. ✅ **Zone Authority Checking**
4. ✅ **Basic Subdomain Support**

#### **MEDIUM PRIORITY (Week 2)**
1. ✅ **API Endpoints for Subdomain Management**
2. ✅ **Redis Schema for Zone/Subdomain Storage**
3. ✅ **DNSSEC Support**
4. ✅ **Zone Transfer Support**

#### **LOW PRIORITY (Week 3)**
1. ✅ **Advanced Monitoring**
2. ✅ **Performance Optimizations**
3. ✅ **Secondary DNS Servers**

### **Success Criteria**

#### **Functional Requirements**
- [ ] DNS server responds with SOA records for zone queries
- [ ] DNS server responds with NS records for zone queries  
- [ ] User subdomains resolve to correct IP addresses
- [ ] API can create/delete subdomains
- [ ] Automatic cleanup of expired subdomains

#### **Performance Requirements**
- [ ] DNS queries respond within 50ms
- [ ] Support 1000+ concurrent subdomains
- [ ] Zone transfers complete within 30 seconds
- [ ] 99.9% uptime for DNS service

#### **Security Requirements**
- [ ] DNSSEC signing for all zone records
- [ ] Rate limiting on subdomain creation
- [ ] Audit logging for all DNS operations
- [ ] Protection against DNS amplification attacks

---

## **Next Steps**

1. **Immediate**: Implement SOA and NS record responses
2. **Short-term**: Add subdomain delegation support  
3. **Medium-term**: Add DNSSEC and zone transfer support
4. **Long-term**: Deploy secondary DNS servers and monitoring

**Status**: 🚨 **CRITICAL GAP** - Must be addressed before production deployment 