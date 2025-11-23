# OAuth Example

Demonstrates OAuth/OIDC authentication with Keycloak.

## Features

- ✅ OAuth 2.0 / OIDC authentication
- ✅ Keycloak integration
- ✅ Token introspection
- ✅ JWT validation
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
  -d "client_secret=your-client-secret" \
  -d "username=testuser" \
  -d "password=testpass" | jq -r .access_token)

# Use token to access API
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/secure-items
```

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
