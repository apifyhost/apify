# Main Configuration

The main configuration file (`config.yaml`) controls the global settings of the Apify instance.

## Structure

```yaml
# Global datasource configuration
datasource:
  sqlite1:
    driver: sqlite
    database: ./apify.sqlite
    max_pool_size: 5

# Global consumer (API key) configuration
auth:
  - name: default-api-keys
    type: api-key
    enabled: true
    config:
      source: header
      key_name: X-Api-Key
      consumers:
        - name: default
          keys:
            - dev-key-123

# HTTP listeners
listeners:
  - name: public
    port: 3000
    ip: 0.0.0.0
    protocol: HTTP

# Global API definitions (mapping APIs to Listeners)
apis:
  - path: openapi/users.yaml
    listeners: 
      - public
    datasource: sqlite1
```

## Sections

### Datasource
Defines database connections available to APIs.

### Auth
Configures global authentication providers.

### Listeners
Configures HTTP servers. Note that each listener must have a unique `name` to be referenced by APIs.

### Apis
Defines which OpenAPI specifications to load and which listeners they should be attached to.
*   `path`: Path to the OpenAPI file.
*   `listeners`: List of listener names that will serve this API.
*   `datasource`: The default datasource to use for operations in this API.

## Modules

You can enable additional, system-wide modules in the main configuration.

```yaml
modules:
  openapi_docs:
    enabled: true
    port: 4001
  metrics:
    enabled: true
    port: 9090
```

### OpenAPI Documentation Server (`openapi_docs`)

The `openapi_docs` module serves the aggregated OpenAPI specification and a Swagger UI to visualize it.

* `enabled`: (Boolean) Enable/Disable the docs server.
* `port`: (Integer) The port where the Docs Server will listen.

Once enabled, you can access:
* **Swagger UI:** `http://localhost:<port>/docs` (e.g., `http://localhost:4001/docs`)
* **Combined OpenAPI Spec:** `http://localhost:<port>/openapi.json`

