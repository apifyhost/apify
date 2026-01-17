# Authentication & Authorization

Apify provides built-in support for multiple authentication methods.

## Supported Methods

- **API Key**: Simple header-based authentication.
- **OAuth 2.0 / OIDC**: Integration with identity providers like Keycloak, Auth0, etc.

## API Key Authentication

Configure via `components.securitySchemes` in your OpenAPI spec.

```yaml
components:
  securitySchemes:
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-API-KEY
security:
  - ApiKeyAuth: []
```

## OAuth 2.0 / OIDC

Supports:
- Token introspection
- JWT validation
- Automatic JWKS caching
- Issuer and audience validation

## Access Control

Access control can be configured at multiple levels:
- **Operation Level**: Per API endpoint.
- **Route Level**: Per URL path.
- **Listener Level**: Per network port.
