# Zero-Code CRUD Operations

Define your data models in OpenAPI specs with `x-table-schemas`, and Apify automatically generates complete CRUD endpoints (Create, Read, Update, Delete) with database operations. No boilerplate code needed!

## How it works

1. **Define Schema**: Add `x-table-schemas` to your OpenAPI spec.
2. **Auto-Generation**: Apify parses the spec and generates SQL queries.
3. **Execution**: Requests are routed to the generated handlers.

## Example

```yaml
openapi: 3.0.0
info:
  title: Users API
  version: 1.0.0
paths:
  /users:
    get:
      summary: List users
      responses:
        '200':
          description: OK
    post:
      summary: Create user
      responses:
        '201':
          description: Created
components:
  schemas:
    User:
      type: object
      properties:
        id:
          type: integer
        name:
          type: string
x-table-schemas:
  users:
    columns:
      - name: id
        type: integer
        primary_key: true
        auto_increment: true
      - name: name
        type: string
```
