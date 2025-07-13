# FleetingDNS SSH Reverse Tunnel - Developer Guide

## 🚀 Overview

FleetingDNS provides **SSH reverse tunnels over port 443** to expose your local development services to the internet through corporate firewalls. This enables:

- **Corporate Firewall Bypass**: Uses port 443 (HTTPS) instead of blocked port 22
- **Public URL Generation**: Get `https://myservice123.fleetingdns.run` URLs instantly
- **Local Development**: Keep your service running locally while being publicly accessible
- **Zero Configuration**: No DNS setup, no port forwarding, no firewall changes

## 🔧 Quick Start

### 1. Connect to EdgeHub
```bash
# Connect through corporate firewall using port 443
ssh -R 0:localhost:8080 user@edgehub.fleetingdns.com -p 443

# Alternative with explicit service name
ssh -R 3000:localhost:3000 user@edgehub.fleetingdns.com -p 443
```

### 2. Get Your Public URL
When you connect, EdgeHub will display:
```
Reverse tunnel established: https://myservice123456.fleetingdns.run -> localhost:8080
```

### 3. Access Your Service
Your local service is now accessible at:
```
https://myservice123456.fleetingdns.run
```

## 📋 Detailed Usage

### Basic Reverse Tunnel
```bash
# Expose local port 8080 as a public HTTPS URL
ssh -R 0:localhost:8080 developer@edgehub.fleetingdns.com -p 443

# Output:
# Reverse tunnel established: https://myservice789123.fleetingdns.run -> localhost:8080
```

### Multiple Services
```bash
# Terminal 1: Expose your API
ssh -R 0:localhost:3000 developer@edgehub.fleetingdns.com -p 443

# Terminal 2: Expose your frontend  
ssh -R 0:localhost:8080 developer@edgehub.fleetingdns.com -p 443

# You'll get two different URLs:
# https://myservice123456.fleetingdns.run -> localhost:3000 (API)
# https://myservice789012.fleetingdns.run -> localhost:8080 (Frontend)
```

### Custom Service Names
```bash
# Use a specific port number to hint at service type
ssh -R 3000:localhost:3000 developer@edgehub.fleetingdns.com -p 443  # API
ssh -R 8080:localhost:8080 developer@edgehub.fleetingdns.com -p 443  # Frontend
ssh -R 5432:localhost:5432 developer@edgehub.fleetingdns.com -p 443  # Database
```

## 🌐 Architecture

```
┌─────────────────┐    HTTPS/443     ┌─────────────────┐    SSH Tunnel    ┌─────────────────┐
│   Consumer      │◄────────────────►│    EdgeHub      │◄────────────────►│   Developer     │
│   Internet      │  myservice123.   │   Port 443      │    Port 443      │   Local App     │
│   Browser/API   │  fleetingdns.run  │   Public GW     │   (Reverse)      │   Port 8080     │
└─────────────────┘                  └─────────────────┘                  └─────────────────┘
```

### How It Works
1. **Developer** creates reverse tunnel: `ssh -R 0:localhost:8080 user@edgehub:443`
2. **EdgeHub** generates unique subdomain: `myservice123456.fleetingdns.run`
3. **Consumer** accesses: `https://myservice123456.fleetingdns.run`
4. **EdgeHub** forwards request through SSH tunnel to developer's localhost:8080
5. **Response** flows back through tunnel to consumer

## 🔐 Corporate Firewall Compatibility

### Why Port 443?
- **HTTPS Traffic**: Appears as normal web traffic to corporate firewalls
- **Unrestricted**: Port 443 is rarely blocked in corporate environments
- **TLS Wrapped**: SSH traffic is wrapped in TLS for additional stealth
- **Enterprise Friendly**: Meets corporate security policies

### SSH Client Configuration
Create `~/.ssh/config` for easier connections:
```ssh
Host fleetingdns
    HostName edgehub.fleetingdns.com
    Port 443
    User developer
    IdentityFile ~/.ssh/id_rsa
    ServerAliveInterval 30
    ServerAliveCountMax 3
```

Then connect simply:
```bash
ssh -R 0:localhost:8080 fleetingdns
```

## 💡 Use Cases

### 1. **API Development**
```bash
# Expose your local API for frontend team testing
ssh -R 0:localhost:3000 fleetingdns

# Share the URL: https://myservice123456.fleetingdns.run
# Frontend team can now test against your local API
```

### 2. **Webhook Testing**
```bash
# Expose local webhook endpoint for external services
ssh -R 0:localhost:4000 fleetingdns

# Configure webhook URL in external service:
# https://myservice789012.fleetingdns.run/webhook
```

### 3. **Demo & Presentations**
```bash
# Quickly expose your local demo for stakeholders
ssh -R 0:localhost:8080 fleetingdns

# Share demo URL: https://myservice345678.fleetingdns.run
# No deployment needed!
```

### 4. **Mobile App Testing**
```bash
# Expose local backend for mobile app testing
ssh -R 0:localhost:5000 fleetingdns

# Configure mobile app to use: https://myservice456789.fleetingdns.run
# Test on real devices without complex network setup
```

## 🛠️ Advanced Configuration

### Persistent Connections
```bash
# Keep connection alive with autossh
autossh -M 0 -R 0:localhost:8080 fleetingdns \
  -o "ServerAliveInterval 30" \
  -o "ServerAliveCountMax 3"
```

### Multiple Port Forwarding
```bash
# Forward multiple ports in one connection
ssh -R 0:localhost:3000 -R 0:localhost:8080 -R 0:localhost:5432 fleetingdns
```

### Background Tunnels
```bash
# Run tunnel in background
ssh -f -N -R 0:localhost:8080 fleetingdns

# Kill background tunnel
pkill -f "ssh.*fleetingdns"
```

## 📊 Monitoring & Management

### Check Active Tunnels
```bash
# List your active tunnels
ssh fleetingdns "list-tunnels"

# Output:
# myservice123456.fleetingdns.run -> localhost:8080 (active)
# myservice789012.fleetingdns.run -> localhost:3000 (active)
```

### Tunnel Status
```bash
# Check tunnel health
curl https://myservice123456.fleetingdns.run/health

# If your service responds, tunnel is working
```

## 🚨 Security Considerations

### Authentication
- **Public Key Only**: Password authentication is disabled
- **SSH Keys Required**: Generate and configure SSH keys
- **Access Control**: EdgeHub validates authorized keys

### Network Security
- **TLS Encryption**: All traffic encrypted over HTTPS
- **Temporary URLs**: Subdomains are ephemeral and change
- **No Persistent Storage**: No data stored on EdgeHub
- **Firewall Friendly**: Uses standard HTTPS port

### Best Practices
1. **Use SSH Keys**: Never use passwords
2. **Rotate Keys**: Regularly update SSH keys
3. **Monitor Access**: Check tunnel access logs
4. **Limit Exposure**: Only expose necessary services
5. **Use HTTPS**: Ensure your local service uses HTTPS when possible

## 🐛 Troubleshooting

### Connection Issues
```bash
# Test basic connectivity
telnet edgehub.fleetingdns.com 443

# Test SSH connection
ssh -v fleetingdns

# Check SSH key
ssh-add -l
```

### Tunnel Not Working
```bash
# Check local service is running
curl localhost:8080

# Check tunnel status
ssh fleetingdns "tunnel-status myservice123456"

# Restart tunnel
pkill -f "ssh.*fleetingdns"
ssh -R 0:localhost:8080 fleetingdns
```

### Corporate Firewall Issues
```bash
# Try with explicit TLS wrapping
ssh -o ProxyCommand="openssl s_client -connect edgehub.fleetingdns.com:443 -quiet" fleetingdns

# Alternative: Use HTTP CONNECT proxy
ssh -o ProxyCommand="nc -X connect -x proxy.company.com:8080 edgehub.fleetingdns.com 443" fleetingdns
```

## 📚 Examples

### Node.js Express App
```javascript
// app.js
const express = require('express');
const app = express();

app.get('/', (req, res) => {
  res.json({ message: 'Hello from FleetingDNS tunnel!' });
});

app.listen(3000, () => {
  console.log('Server running on localhost:3000');
});
```

```bash
# Terminal 1: Start your app
node app.js

# Terminal 2: Create tunnel
ssh -R 0:localhost:3000 fleetingdns

# Access via: https://myservice123456.fleetingdns.run
```

### Python Flask App
```python
# app.py
from flask import Flask, jsonify

app = Flask(__name__)

@app.route('/')
def hello():
    return jsonify(message='Hello from FleetingDNS tunnel!')

if __name__ == '__main__':
    app.run(host='localhost', port=5000)
```

```bash
# Terminal 1: Start your app
python app.py

# Terminal 2: Create tunnel
ssh -R 0:localhost:5000 fleetingdns

# Access via: https://myservice789012.fleetingdns.run
```

## 🎯 Next Steps

1. **Set up SSH keys** for passwordless authentication
2. **Configure ~/.ssh/config** for easier connections
3. **Test your first tunnel** with a simple local service
4. **Share your public URL** with team members
5. **Explore advanced features** like multiple port forwarding

---

**Need Help?** 
- Check the troubleshooting section above
- Review SSH connection logs with `-v` flag
- Ensure your local service is running and accessible

**Corporate Environment?**
- Use port 443 for firewall compatibility
- Consider TLS wrapping for additional stealth
- Work with IT team if needed for SSH key approval 