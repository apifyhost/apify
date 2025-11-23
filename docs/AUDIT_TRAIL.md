# Audit Trail Feature

## Overview

The audit trail feature automatically tracks who created and last modified database records when users are authenticated via OAuth/OIDC.

## How It Works

When you mark fields as `readOnly` in your OpenAPI schema, Apify automatically:
1. Creates the corresponding database columns
2. Injects user identity on CREATE operations (for `createdBy` and `updatedBy`)
3. Injects user identity on UPDATE operations (for `updatedBy` only)
4. Prevents users from overriding these values

## Supported Audit Fields

- **createdBy** (TEXT): User ID who created the record
- **updatedBy** (TEXT): User ID who last updated the record
- **createdAt** (DATETIME): Timestamp when created (auto-populated by database)
- **updatedAt** (DATETIME): Timestamp when last updated

## Configuration

### Method 1: Using OpenAPI Schema (Recommended)

Define your schema in `components.schemas` with `readOnly` fields:

```yaml
openapi:
  spec:
    openapi: 3.0.0
    components:
      schemas:
        Task:
          type: object
          required:
            - title
          properties:
            id:
              type: integer
              readOnly: true
            title:
              type: string
            createdBy:
              type: string
              readOnly: true
              description: User who created the task
            updatedBy:
              type: string
              readOnly: true
              description: User who last updated the task
            createdAt:
              type: string
              format: date-time
              readOnly: true
            updatedAt:
              type: string
              format: date-time
              readOnly: true
```

### Method 2: Using x-table-schemas

Directly define table columns with `auto_field: true`:

```yaml
openapi:
  spec:
    x-table-schemas:
      - table_name: "items"
        columns:
          - name: "createdBy"
            column_type: "TEXT"
            nullable: true
            auto_field: true
          - name: "updatedBy"
            column_type: "TEXT"
            nullable: true
            auto_field: true
          - name: "createdAt"
            column_type: "DATETIME"
            nullable: true
            default_value: "CURRENT_TIMESTAMP"
            auto_field: true
          - name: "updatedAt"
            column_type: "DATETIME"
            nullable: true
            auto_field: true
```

## OAuth Configuration

Audit trail requires OAuth/OIDC authentication to extract user identity:

```yaml
listeners:
  - port: 8080
    ip: 0.0.0.0
    protocol: HTTP
    apis:
      - path: ./openapi/tasks.yaml
        datasource: postgres1

oauth_providers:
  - name: keycloak
    issuer: http://localhost:8080/realms/myrealm
    audience: account
```

## User Identity Extraction

The system extracts user identity from:
1. JWT `sub` claim (preferred)
2. JWT `username` claim (fallback)
3. Token introspection `sub` field
4. Token introspection `username` field

## Example

### Creating a Record

```bash
# User authenticates
TOKEN=$(curl -X POST http://localhost:8080/realms/myrealm/protocol/openid-connect/token \
  -d "grant_type=password" \
  -d "client_id=myclient" \
  -d "username=alice" \
  -d "password=secret" | jq -r .access_token)

# Create a task
curl -X POST http://localhost:3000/tasks \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "My Task", "description": "Do something"}'

# Response includes audit fields:
{
  "id": 1,
  "title": "My Task",
  "description": "Do something",
  "createdBy": "alice",
  "updatedBy": "alice",
  "createdAt": "2025-11-23T10:30:00Z",
  "updatedAt": null
}
```

### Updating a Record

```bash
# Update the task
curl -X PUT http://localhost:3000/tasks/1 \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title": "Updated Task"}'

# Response shows updatedBy changed:
{
  "id": 1,
  "title": "Updated Task",
  "description": "Do something",
  "createdBy": "alice",        # Preserved
  "updatedBy": "alice",         # Updated
  "createdAt": "2025-11-23T10:30:00Z",  # Preserved
  "updatedAt": "2025-11-23T11:45:00Z"   # Updated
}
```

## Security

- Audit fields are **automatically populated** and cannot be overridden by users
- Even if a user sends `"createdBy": "hacker"` in the request body, the system will replace it with the authenticated user's identity
- Without authentication, audit fields will be empty or null

## Testing

See `e2e/oauth_test.go` for comprehensive test cases covering:
- Automatic population of audit fields
- Preservation of `createdBy` on updates
- Protection against user override attempts

## Examples

- **Full Example**: `examples/full/` - Complete setup with OAuth and audit fields for E2E testing
- **OAuth Example**: `examples/oauth/` - OAuth authentication with audit trail demonstration
