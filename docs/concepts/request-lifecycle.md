# Request Lifecycle

Understanding how a request flows through Apify.

## Flow

1.  **Ingress**: Request arrives at a configured Listener (e.g., port 8080).
2.  **Routing**: The router matches the URL path and HTTP method to an Operation defined in an OpenAPI spec.
3.  **Authentication**:
    *   Global auth providers check for credentials (e.g., API Key).
    *   If valid, the request proceeds.
4.  **Validation**:
    *   Path parameters, query parameters, and request body are validated against the OpenAPI schema.
    *   Invalid requests are rejected with 400 Bad Request.
5.  **Processing**:
    *   The `x-apify-action` determines the logic (e.g., `list`, `create`).
    *   SQL is generated based on the action and parameters.
6.  **Data Access**:
    *   The query is executed against the mapped Datasource.
7.  **Response**:
    *   Results are serialized to JSON.
    *   Response headers are set.
    *   Response is sent to the client.
