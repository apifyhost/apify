# Relations (Nested Objects) Example

This example demonstrates Apify's support for creating related records in a single request using nested objects.

## Features

- ✅ **HasMany Relations** - Create parent records with nested children
- ✅ **HasOne Relations** - Create parent with single nested child
- ✅ **BelongsTo Relations** - Automatic parent loading for child records
- ✅ **Automatic Foreign Key Injection** - System automatically sets foreign keys
- ✅ **Auto-load Nested Data** - GET requests automatically include related records
- ✅ **Audit Trail** - Works with authentication to track creators

## Quick Start

```bash
# From repository root
./quickstart.sh relations

# Or manually with docker-compose
cd examples/relations
docker-compose up -d
```

## API Endpoint

- **API**: http://localhost:3000

## Example: Create Order with Items

### Single Request with Nested Items

```bash
curl -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{
    "customer_name": "Alice Johnson",
    "total": 150.00,
    "status": "pending",
    "items": [
      {
        "product_name": "Laptop",
        "quantity": 1,
        "price": 100.00
      },
      {
        "product_name": "Mouse",
        "quantity": 2,
        "price": 25.00
      }
    ]
  }'
```

**What happens internally:**
1. Creates the order record in `orders` table
2. Extracts the nested `items` array
3. For each item, automatically injects `order_id` (foreign key)
4. Creates each item in `order_items` table
5. Returns the created order

**Response:**
```json
{
  "id": 1,
  "customer_name": "Alice Johnson",
  "total": 150.00,
  "status": "pending",
  "createdAt": "2025-11-23T10:30:00Z",
  "updatedAt": null
}
```

## Schema Definition

Define relations in your OpenAPI schema using `x-relation`:

```yaml
components:
  schemas:
    Order:
      properties:
        items:
          type: array
          x-relation:
            type: hasMany          # Relation type
            target: OrderItem      # Related schema
            foreignKey: order_id   # Foreign key column
          items:
            $ref: '#/components/schemas/OrderItem'
```

## Supported Relation Types

### 1. hasMany (One-to-Many)

**Example:** An order has many items

```yaml
Order:
  properties:
    items:
      type: array
      x-relation:
        type: hasMany
        target: OrderItem
        foreignKey: order_id
```

**Usage:**
```json
{
  "customer_name": "Bob",
  "items": [
    {"product_name": "Item 1", "quantity": 2},
    {"product_name": "Item 2", "quantity": 1}
  ]
}
```

### 2. belongsTo (Many-to-One)

**Status:** ✅ Implemented

**Example:** UserProfile belongs to User

```yaml
UserProfile:
  properties:
    user:
      type: object
      x-relation:
        type: belongsTo
        target: User
        foreignKey: user_id
```

**Usage:**
```bash
# GET /userprofiles/1 automatically includes the parent user
{
  "id": 1,
  "user_id": 1,
  "full_name": "Alice",
  "user": {
    "id": 1,
    "username": "alice",
    "email": "alice@example.com"
  }
}
```

### 3. hasOne (One-to-One)

**Status:** ✅ Implemented

**Example:** User has one profile

```yaml
User:
  properties:
    profile:
      type: object
      x-relation:
        type: hasOne
        target: UserProfile
        foreignKey: user_id
```

**Usage:**
```bash
# Create user with nested profile
curl -X POST /users -d '{
  "username": "alice",
  "email": "alice@example.com",
  "profile": {
    "full_name": "Alice Johnson",
    "bio": "Software engineer"
  }
}'

# GET /users/1 automatically includes the profile
{
  "id": 1,
  "username": "alice",
  "profile": {
    "id": 1,
    "full_name": "Alice Johnson"
  }
}
```

### 4. belongsToMany (Many-to-Many)

**Example:** Students and courses

*(Coming soon - requires junction table support)*

## How It Works

### 1. Schema Recognition

The schema generator scans your OpenAPI spec for `x-relation` extensions and extracts:
- Relation type (hasMany, belongsTo, etc.)
- Target table name
- Foreign key column
- Local key column (optional, defaults to "id")

### 2. Request Processing

When creating a record:
1. Extract nested relation data from request body
2. Insert parent record first
3. Get inserted parent ID
4. For each nested item:
   - Inject foreign key value
   - Inject audit fields if authenticated
   - Insert into target table

### 3. Database Schema

Tables are created with foreign key constraints:

```sql
CREATE TABLE orders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  customer_name TEXT NOT NULL,
  total REAL,
  status TEXT DEFAULT 'pending',
  createdAt DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE order_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  order_id INTEGER NOT NULL,     -- Foreign key
  product_name TEXT NOT NULL,
  quantity INTEGER NOT NULL,
  price REAL NOT NULL,
  FOREIGN KEY (order_id) REFERENCES orders(id)
);
```

## Testing

Run the comprehensive test scripts:

```bash
# Test hasMany relations (orders with items)
./test_relations.sh

# Test hasOne and belongsTo relations (users with profiles)
./test_relations_advanced.sh
```

Or manually test:

```bash
# Create an order with items
ORDER_ID=$(curl -s -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{
    "customer_name": "Test User",
    "total": 99.99,
    "items": [
      {"product_name": "Widget", "quantity": 3, "price": 33.33}
    ]
  }' | jq -r .id)

# Get order (now includes nested items automatically!)
curl http://localhost:3000/orders/$ORDER_ID | jq '.'

# List all orders (each includes its items)
curl http://localhost:3000/orders | jq '.'
```
curl http://localhost:3000/order-items
```

## Limitations (Current Version)

1. **Update operations** - Nested updates not yet supported (must update children separately)
2. **Delete cascading** - Manual deletion of child records required
3. **Transaction rollback** - Currently no transaction support (may leave partial data on failure)

## Future Enhancements

- [x] ✅ Fetch nested data on GET requests
- [x] ✅ belongsTo relation support
- [x] ✅ hasOne relation support
- [ ] Update nested relations
- [ ] Cascade delete support
- [ ] Full transaction support with rollback
- [ ] belongsToMany (many-to-many) relations
- [ ] Eager loading configuration

## Stop and Clean

```bash
docker-compose down -v
```

## See Also

- [docs/RELATIONS.md](../../docs/RELATIONS.md) - Complete relations documentation
- [examples/oauth](../oauth/) - Authentication and audit trail
- [examples/full](../full/) - Complete E2E example
