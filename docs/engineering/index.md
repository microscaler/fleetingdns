 # Engineering Index
 
 This document provides an overview of the system architecture, key components, and workflows involved in the engineering platform. It includes sequence diagrams and detailed explanations to help engineers understand the interactions between services, APIs, and infrastructure elements.


## Document Index

- [E1‑Core DNS Service (Design v0.1)](E1-Core_DNS_Service_(Design_v0.1).md)
- [E2‑Tunnel Server & CLI (Design v0.2)](E2-Tunnel_Server_&_CLI_(Design_v0.2).md)
- [E3‑Edge Proxy (Design v0.1)](E3-Edge_Proxy_(Design_v0.1).md)
- [E4‑Basic Auth Redirect and Auth Modes (Design v0.2)](E4-Basic_Auth_Redirect_and_Auth_Modes_(Design_v0.2).md)
- [E5‑SDK Integration (Design v0.2)](E5-SDK_Integration_(Design_v0.2).md)
- [E6‑CI Integration GitHub Action (Design v0.1)](E6-CI_Integration_GitHub_Action_(Design_v0.1).md)
- [E7‑Rate Limiting Design (v0.1)](E7-Rate_Limiting_Design(v0.1).md)
- [E8‑Security & Hardening (Design v0.1)](E8-Security_&_Hardening_(Design_v0.1).md)
- [E9‑Api Key Strategy Design (V0.1)](E9-Api_Key_Strategy_Design_(V0.1).md)

---

###  End to end backend sequence diagram for the Ephemeral DNS Forwarder (FDF) system

```mermaid
sequenceDiagram
autonumber
participant TR as Test‑runner (your code)
participant SDK as SDK client lib
participant API as APIService
participant ETCD as etcd KV‑store
participant DNS as CoreDNS
participant EDGE as TLSRedirector / Proxy
participant TP as TunnelProxy (edge)
participant TC as TunnelClient (on dev box/CI)
participant APP as LocalService (127.0.0.1:8080)

    %% 1Allocate
    TR->>SDK: allocate(target="http://127.0.0.1:8080", ttl=300 s)
    SDK->>+API: POST /v1/endpoint
    API->>+ETCD: PUT /dns/<uuid> {A→Edge‑IP, meta}
    ETCD-->>-API: 200 OK
    API-->>SDK: 201 {id, fqdn}
    SDK-->>TR: fqdn = <uuid>.ep.testdns.dev

    %% 2Start reverse tunnel
    TR->>TC: start zrok|ssh‑R|socat reverse tunnel
    TC->>+TP: websocket/auth handshake
    TP-->>-TC: tunnel ready

    %% 3External caller hits the host
    Note over TR,DNS: Meanwhile in another process/host…
    External-->DNS: query <uuid>.ep.testdns.dev
    DNS->>ETCD: lookup /dns/<uuid>
    DNS-->>External: Arecord (Edge‑IP)

    External->>EDGE: TLS+HTTP GET https://<fqdn>/
    EDGE->>ETCD: GET /dns/<uuid> (meta)
    EDGE-->>External: 302 Location: https://tp.ep.testdns.dev/<uuid>

    External->>TP: TLS+HTTP GET /<uuid>
    TP->>TC: multiplexed tunnel traffic
    TC->>APP: HTTP GET /
    APP-->>TC: 200 response body
    TC-->>TP: stream response
    TP-->>External: 200 response

    %% 4Cleanup
    TR-->SDK: deallocate(id)
    SDK->>+API: DELETE /v1/endpoint/<id>
    API->>+ETCD: DEL /dns/<uuid>
    ETCD-->>-API: 200
    API-->>SDK: 204 NoContent
    SDK-->>TR: done
    alt GC loop
        API->>ETCD: purge expired keys
    end
```

