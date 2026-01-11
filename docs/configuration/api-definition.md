# API Definition (OpenAPI)

Apify uses standard OpenAPI 3.0 files to define APIs. These files can be in YAML or JSON format.
Apify extends the OpenAPI specification with custom extensions to define the underlying database schema and link API operations to database actions.

## Table Schema Definition (`x-table-schemas`)

You can define the database tables that back your API using the `x-table-schemas` extension at the root of your OpenAPI document. 
This configuration allows Apify to automatically create or migrate tables in the configured Datasource.

**Note:** All property names in these extensions use `camelCase`.

```yaml
openapi: 3.0.0
info:
  title: Users API
  version: 1.0.0
x-table-schemas:
  - tableName: "users"
    columns:
      - name: "id"
        columnType: "INTEGER"
        primaryKey: true
        autoIncrement: true
        nullable: false
      - name: "email"
        columnType: "TEXT"
        unique: true
        nullable: false
      - name: "name"
        columnType: "TEXT"
        nullable: true
    indexes: []
```

### Table Schema Properties

| Property | Type | Description |
|----------|------|-------------|
| `tableName` | String | The name of the table in the database. |
| `columns` | Array | List of column definitions. |
| `indexes` | Array | (Optional) List of indexes. |
| `relations` | Array | (Optional) List of relations (e.g. nested objects). |

### Column Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `name` | String | Required | Column name. |
| `columnType` | String | Required | Database type (e.g., `INTEGER`, `TEXT`, `VARCHAR(255)`, `BOOLEAN`). The specific types depend on the underlying driver (SQLite/Postgres). |
| `nullable` | Boolean | `false` | Whether the column allows NULL values. |
| `primaryKey` | Boolean | `false` | Whether this is a primary key. |
| `unique` | Boolean | `false` | Whether values must be unique. |
| `autoIncrement` | Boolean | `false` | Whether the DB automatically increments this value. |
| `defaultValue` | String | `null` | Default value for the column. |

## Linking Operations to Tables

Apify binds standard HTTP methods to CRUD operations.

* `GET /collection` -> List
* `POST /collection` -> Create
* `GET /collection/{id}` -> Get One
* `PUT /collection/{id}` -> Update
* `DELETE /collection/{id}` -> Delete

Apify automatically tries to infer the target table for a CRUD operation based on the path. You can explicitly define the target table using `x-table-name` at the operation level if automatic inference fails or if you want to map a path to a different table.

```yaml
paths:
  /users:
    get:
      operationId: listUsers
      x-table-name: "users"
      responses:
        '200':
          description: List of users
```

## Enabling Modules (`x-modules`)

You can enable specific Apify modules for an endpoint using the `x-modules` extension. This is commonly used for authentication or applying specific middleware.

```yaml
paths:
  /secure-data:
    get:
      x-modules:
        access: ["key_auth"]
      responses:
        '200':
          description: Secure data
```

| Key | Value | Description |
|-----|-------|-------------|
| `access` | Array of Strings | List of auth/access modules to enable (e.g., `key_auth`). |
