# ServicePlan Enhancements PRD

## Overview
This PRD outlines the implementation of comprehensive ServicePlan management functionality for FleetingDNS, enabling flexible service tier management, user assignment, and quota enforcement.

## Current Status: ✅ **PHASE 1 COMPLETED**

### ✅ **Phase 1: Complete Admin CRUD endpoints** - **COMPLETED**
- **Admin ServicePlan CRUD API endpoints** ✅
  - `POST /admin/service-plans` - Create new ServicePlan
  - `GET /admin/service-plans` - List all ServicePlans  
  - `GET /admin/service-plans/:id` - Get specific ServicePlan
  - `PUT /admin/service-plans/:id` - Update ServicePlan
  - `DELETE /admin/service-plans/:id` - Delete ServicePlan (with validation)
- **User ServicePlan Assignment** ✅
  - `POST /admin/users/:user_id/service-plan` - Assign ServicePlan to user
- **Comprehensive validation** ✅
  - Unique name validation
  - Active assignment protection (prevents deletion of plans in use)
  - Proper error handling with custom error types
- **Database integration** ✅
  - Full SeaORM integration with PostgreSQL
  - Proper entity relationships and constraints
  - Transaction safety and data integrity
- **Authentication & Authorization** ✅
  - JWT token validation for all admin endpoints
  - Admin role validation (placeholder for future implementation)
- **Comprehensive testing** ✅
  - Unit tests for all CRUD operations
  - E2E integration tests with PostgreSQL container
  - Error handling and edge case coverage

### 🔄 **Phase 2: User-facing ServicePlan endpoints** - **NEXT**
- User ServicePlan retrieval endpoints
- Current plan status and quota information
- Plan upgrade/downgrade requests
- Usage statistics and limits

### 📋 **Phase 3: Rate limiting integration** - **PENDING**
- ServicePlan-based rate limiting
- Dynamic quota enforcement
- Usage tracking and analytics

### 📋 **Phase 4: Billing and lifecycle management** - **PENDING**
- Plan expiration handling
- Automatic plan transitions
- Billing event integration

## Implementation Details

### Database Schema
- **ServicePlan table**: Complete with all required fields
- **UserServicePlan table**: Assignment tracking with lifecycle management
- **Constraints**: Proper foreign keys and unique constraints
- **Migrations**: Fully tested and working

### API Endpoints Implemented

#### ServicePlan Management
```http
POST /admin/service-plans
{
  "name": "Pro",
  "api_rate_limit": 1000,
  "tunnel_creation_limit": 10,
  "dns_provisioning_limit": 5,
  "max_concurrent_tunnels": 3,
  "features_json": "{\"custom_domains\": true}"
}

GET /admin/service-plans
GET /admin/service-plans/:id
PUT /admin/service-plans/:id
DELETE /admin/service-plans/:id
```

#### User Assignment
```http
POST /admin/users/:user_id/service-plan
{
  "service_plan_id": "plan-uuid",
  "end_date": "2024-12-31T23:59:59Z" // optional
}
```

### Error Handling
- **ValidationError**: For business rule violations
- **NotFound**: For missing resources
- **DatabaseError**: For database operation failures
- **AuthenticationFailed**: For invalid tokens

### Testing Coverage
- ✅ **Unit tests**: All CRUD operations
- ✅ **Integration tests**: Full E2E with PostgreSQL
- ✅ **Error handling**: Comprehensive error scenarios
- ✅ **Validation**: Unique constraints and business rules
- ✅ **Authentication**: JWT token validation

## Next Steps

### Phase 2: User-facing endpoints
1. **User ServicePlan retrieval**
   - `GET /v1/users/me/service-plan` - Get current plan
   - `GET /v1/users/me/usage` - Get usage statistics
2. **Plan management**
   - `POST /v1/users/me/plan-upgrade-request` - Request plan change
   - `GET /v1/service-plans` - List available plans (public)

### Phase 3: Rate limiting integration
1. **ServicePlan-based rate limiting**
   - Dynamic rate limit configuration
   - Per-plan quota enforcement
2. **Usage tracking**
   - Real-time usage monitoring
   - Quota exhaustion handling

### Phase 4: Billing and lifecycle
1. **Plan lifecycle management**
   - Automatic plan expiration
   - Grace period handling
2. **Billing integration**
   - Stripe integration
   - Payment event handling

## Technical Architecture

### Database Layer
- **SeaORM**: Type-safe database operations
- **PostgreSQL**: Primary database with proper constraints
- **Migrations**: Version-controlled schema changes

### API Layer
- **Axum**: High-performance web framework
- **JWT Authentication**: Secure token-based auth
- **Validation**: Comprehensive input validation
- **Error Handling**: Structured error responses

### Testing Strategy
- **Unit Tests**: Individual component testing
- **Integration Tests**: Full API endpoint testing
- **E2E Tests**: Complete workflow testing with containers

## Success Metrics
- ✅ **Phase 1**: All admin CRUD operations working
- ✅ **Database**: Full schema with constraints
- ✅ **API**: Complete RESTful endpoints
- ✅ **Testing**: Comprehensive test coverage
- ✅ **Documentation**: Clear API documentation

## Dependencies
- **PostgreSQL**: Database backend
- **SeaORM**: Database ORM
- **Axum**: Web framework
- **JWT**: Authentication
- **Testcontainers**: Integration testing 