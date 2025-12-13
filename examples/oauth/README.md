# OAuth & Audit Trail Example

Demonstrates OAuth/OIDC authentication with Keycloak and automatic audit trail tracking.

## Features

- ✅ OAuth 2.0 / OIDC authentication
- ✅ Keycloak integration
- ✅ Token introspection
- ✅ JWT validation
- ✅ **Automatic audit trail** - tracks who created/updated records
- ✅ PostgreSQL database

## Quick Start

```bash
# From repository root
./quickstart.sh oauth

# Wait for Keycloak to be ready (takes ~60 seconds)
```

## Access Points

- **API**: http://localhost:3000
- **Keycloak Admin**: http://localhost:8080 (admin/admin)
- **PostgreSQL**: localhost:5432

## Setup Keycloak

1. Access Keycloak admin console at http://localhost:8080
2. Login with username `admin`, password `admin`
3. Navigate to the `apify` realm
4. Import the realm configuration from `config/keycloak/realm-export.json`
5. Create a test user or use existing OAuth client

## Get Access Token

```bash
# Get token using password grant
TOKEN=$(curl -X POST http://localhost:8080/realms/apify/protocol/openid-connect/token \
  -d "grant_type=password" \
  -d "client_id=apify-client" \
  -d "client_secret=apify-test-secret" \
  -d "username=testuser" \
  -d "password=testpass" | jq -r .access_token)

# Use token to access API
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/secure-items
```

## Audit Trail Usage

When authenticated users create or update items, audit fields are automatically populated:

```bash
# Create an item (audit fields auto-populated)
curl -X POST http://localhost:3000/secure-items \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Test Item",
    "description": "Testing audit trail",
    "price": 99.99
  }'

# Response includes audit fields:
# {
#   "id": 1,
#   "name": "Test Item",
#   "createdBy": "testuser",   ← Automatically set from OAuth token
#   "updatedBy": "testuser",   ← Automatically set
#   "createdAt": "2025-11-23T10:30:00Z",
#   "updatedAt": null
# }

# Update the item
curl -X PUT http://localhost:3000/secure-items/1 \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "Updated Item", "price": 149.99}'

# Note: createdBy and createdAt are preserved, updatedBy and updatedAt are updated
```

**Security Note:** Even if you try to override audit fields in the request body, the system will ignore your values and use the authenticated user's identity.

## Configuration

- **OAuth Provider**: Configured in `config/config.yaml` under `oauth_providers`
- **Keycloak Realm**: `config/keycloak/realm-export.json`
- **API Spec**: `config/openapi/items_oauth.yaml`

## Important Notes

- The first startup takes longer as Keycloak initializes
- Client secret must match between Apify config and Keycloak
- Token issuer URL must be `http://localhost:8080/realms/apify`

## Stop and Clean

```bash
./quickstart.sh oauth stop    # Stop services
./quickstart.sh oauth clean   # Stop and remove data
```

## Configuration

- **Main Config**: `config/config.yaml`
- **Resources**: `config/resource.yaml` (listeners, auth, datasources)
- **OpenAPI Spec**: `config/openapi/items_oauth.yaml`
