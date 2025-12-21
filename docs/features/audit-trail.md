# Audit Trail

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
