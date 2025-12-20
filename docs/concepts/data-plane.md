# Data Plane

The Data Plane is the high-performance engine responsible for handling actual API traffic. It executes the logic defined by the Control Plane configuration.

## Responsibilities

1.  **Request Handling**: Accepts incoming HTTP requests on configured ports.
2.  **Routing**: Matches requests to specific API endpoints defined in OpenAPI specs.
3.  **Validation**: Validates request parameters and bodies against the OpenAPI schema.
4.  **Execution**:
    *   **SQL Generation**: Converts API requests into optimized SQL queries.
    *   **Database Interaction**: Executes queries against the configured data sources.
5.  **Response Formatting**: Formats database results into JSON responses matching the schema.

## Performance

The Data Plane is built in Rust for maximum performance and low latency. It uses asynchronous I/O and connection pooling to handle high concurrency.
