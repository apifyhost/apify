# API Definition (OpenAPI)

Apify uses standard OpenAPI 3.0 files to define APIs. You add special extensions to map endpoints to database actions.

## Extensions

### `x-apify-action`
Defines the CRUD operation for an endpoint.

*   `list`: Get a list of resources.
*   `get`: Get a single resource by ID.
*   `create`: Create a new resource.
*   `update`: Update an existing resource.
*   `delete`: Delete a resource.

### `x-apify-resource`
Specifies the database table name.

```yaml
paths:
  /users:
    get:
      x-apify-action: list
      x-apify-resource: users
      ...
```

### `x-apify-relations`
Defines relationships between resources (e.g., one-to-many).

```yaml
x-apify-relations:
  - name: posts
    type: hasMany
    resource: posts
    foreignKey: user_id
```
