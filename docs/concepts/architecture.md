# Architecture Overview

Apify is built on a modular, high-performance architecture designed for scalability and flexibility.

## Core Components

### Control Plane (CP)
- Manages configuration, APIs, and datasources
- Provides REST API for dynamic updates
- Stores metadata in a dedicated database
- Pushes updates to Data Plane without restarts

### Data Plane (DP)
- Handles actual API traffic
- Executes CRUD operations
- Manages database connections
- Enforces authentication and authorization

## Request Lifecycle

1. **HeaderParse**: Extract and validate HTTP headers
2. **BodyParse**: Parse and validate request body
3. **Route**: Match request to API operation
4. **Access**: Authentication and authorization
5. **Data**: Execute CRUD operations
6. **Response**: Format and return response
7. **Log**: Request and response logging

## Performance

- **Multi-threaded**: Uses `SO_REUSEPORT` for efficient connection handling
- **Async I/O**: Built on Tokio runtime
- **Zero-Copy**: Optimized request routing
