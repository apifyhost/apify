# Relations and Nested Objects

## Overview

Apify supports creating related records in a single API request using nested objects. This feature is inspired by modern ORM practices and makes it easy to work with related data without multiple API calls.

## Quick Example

```bash
# Create an order with multiple items in ONE request
curl -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{
    "customer_name": "Alice",
    "total": 150.00,
    "items": [
      {"product_name": "Laptop", "quantity": 1, "price": 100.00},
      {"product_name": "Mouse", "quantity": 2, "price": 25.00}
    ]
  }'
```

Behind the scenes, Apify will:
1. Insert the order into `orders` table
2. Insert each item into `order_items` table with the correct `order_id`
3. Return the created order

## Configuration

### Define Relations in OpenAPI Schema

Use the `x-relation` extension to define relationships:

```yaml
components:
  schemas:
    Order:
      type: object
      properties:
        customer_name:
          type: string
        items:
          type: array
          x-relation:                # ← Relation definition
            type: hasMany            # Relation type
            target: OrderItem        # Target schema name
            foreignKey: order_id     # Foreign key in child table
            localKey: id             # Optional: local key (defaults to "id")
          items:
            $ref: '#/components/schemas/OrderItem'

    OrderItem:
      type: object
      properties:
        order_id:
          type: integer
          description: Foreign key to orders
        product_name:
          type: string
        quantity:
          type: integer
        price:
          type: number
```

## Relation Types

### hasMany (One-to-Many)

**Use case:** A parent record has multiple child records

**Example:** Order has many OrderItems

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

**Request:**
```json
{
  "customer_name": "Alice",
  "items": [
    {"product_name": "Item 1", "quantity": 2},
    {"product_name": "Item 2", "quantity": 1}
  ]
}
```

**SQL Generated:**
```sql
INSERT INTO orders (customer_name) VALUES ('Alice');  -- Returns ID 1
INSERT INTO order_items (order_id, product_name, quantity) VALUES (1, 'Item 1', 2);
INSERT INTO order_items (order_id, product_name, quantity) VALUES (1, 'Item 2', 1);
```

### belongsTo (Many-to-One)

**Status:** ✅ Implemented

**Use case:** A child record belongs to a parent record

**Example:** UserProfile belongs to User

```yaml
UserProfile:
  properties:
    user_id:
      type: integer
      description: Foreign key to users table
    user:
      type: object
      x-relation:
        type: belongsTo
        target: User
        foreignKey: user_id
      allOf:
        - $ref: '#/components/schemas/User'
```

**Request (CREATE):**
```json
{
  "user_id": 1,
  "full_name": "Alice Johnson",
  "bio": "Software engineer"
}
```

**Response (GET):**
```json
{
  "id": 1,
  "user_id": 1,
  "full_name": "Alice Johnson",
  "user": {
    "id": 1,
    "username": "alice",
    "email": "alice@example.com"
  }
}
```

### hasOne (One-to-One)

**Status:** ✅ Implemented

**Use case:** A record has exactly one related record

**Example:** User has one Profile

```yaml
User:
  properties:
    profile:
      type: object
      x-relation:
        type: hasOne
        target: UserProfile
        foreignKey: user_id
      allOf:
        - $ref: '#/components/schemas/UserProfile'
```

**Request (CREATE):**
```json
{
  "username": "alice",
  "email": "alice@example.com",
  "profile": {
    "full_name": "Alice Johnson",
    "bio": "Software engineer"
  }
}
```

**Response (GET):**
```json
{
  "id": 1,
  "username": "alice",
  "email": "alice@example.com",
  "profile": {
    "id": 1,
    "user_id": 1,
    "full_name": "Alice Johnson",
    "bio": "Software engineer"
  }
}
```

### belongsToMany (Many-to-Many)

**Status:** Planned (not yet implemented)

**Use case:** Records are related through a junction table

**Example:** Students and Courses (with Enrollments as junction)

```yaml
Student:
  properties:
    courses:
      type: array
      x-relation:
        type: belongsToMany
        target: Course
        through: Enrollment
        foreignKey: student_id
        otherKey: course_id
```

## Field Processing

### Automatic Foreign Key Injection

When creating nested records, Apify automatically:

1. Inserts the parent record
2. Extracts the parent ID
3. Injects it into each child record as the foreign key
4. Inserts all child records

**Example:**

```json
// Request
{
  "customer_name": "Bob",
  "items": [{"product_name": "Widget"}]
}

// What happens internally:
// 1. INSERT INTO orders (customer_name) VALUES ('Bob') → id=5
// 2. For each item: inject order_id=5
// 3. INSERT INTO order_items (order_id, product_name) VALUES (5, 'Widget')
```

### Audit Fields

If authentication is enabled, audit fields are injected into BOTH parent and child records:

```json
{
  "customer_name": "Carol",
  "items": [{"product_name": "Gadget"}]
}

// Result (with OAuth user "alice"):
// orders table:
// - createdBy: "alice"
// - updatedBy: "alice"
//
// order_items table:
// - createdBy: "alice"  ← Also injected!
// - updatedBy: "alice"
```

### UPDATE Operations (Nested Relations)

**Status:** ✅ Supported for hasMany and hasOne

When updating a record with nested relations, Apify will:

1. Update the parent record
2. Delete all existing child records
3. Insert the new child records from the request

**Example (hasMany):**

```bash
# Update order and replace all items
PUT /orders/1
{
  "customer_name": "Alice Updated",
  "total": 250.00,
  "items": [
    {"product_name": "New Laptop", "quantity": 1, "price": 250.00}
  ]
}

# Result:
# - Order #1 updated
# - Old items deleted (Laptop, Mouse)
# - New item created (New Laptop)
```

**Example (hasOne):**

```bash
# Update user and replace profile
PUT /users/1
{
  "username": "alice",
  "email": "alice.new@example.com",
  "profile": {
    "full_name": "Alice M. Johnson",
    "bio": "Senior Software Engineer"
  }
}

# Result:
# - User #1 updated
# - Old profile deleted
# - New profile created
```

**Note:** Currently this is a "replace all" operation. Partial updates of nested items (update specific children by ID) are not yet supported.

### DELETE Operations (Cascade Delete)

**Status:** ✅ Supported for hasMany and hasOne

When deleting a parent record, Apify automatically deletes all related child records for hasMany and hasOne relations.

**Example (hasMany):**

```bash
DELETE /orders/1

# What happens:
# 1. Delete all items WHERE order_id = 1
# 2. Delete order WHERE id = 1
```

**Example (hasOne):**

```bash
DELETE /users/1

# What happens:
# 1. Delete profile WHERE user_id = 1
# 2. Delete user WHERE id = 1
```

**Note:** belongsTo relations do NOT trigger cascade delete (children are not deleted when parent is deleted).

## Response Format

The response now automatically includes nested data for all relation types:

**hasMany example:**
```json
{
  "id": 1,
  "customer_name": "Alice",
  "items": [
    {"id": 1, "product_name": "Laptop", "order_id": 1},
    {"id": 2, "product_name": "Mouse", "order_id": 1}
  ]
}
```

**hasOne example:**
```json
{
  "id": 1,
  "username": "alice",
  "profile": {
    "id": 1,
    "full_name": "Alice Johnson",
    "user_id": 1
  }
}
```

**belongsTo example:**
```json
{
  "id": 1,
  "user_id": 1,
  "full_name": "Alice Johnson",
  "user": {
    "id": 1,
    "username": "alice",
    "email": "alice@example.com"
  }
}
```

## Examples

### E-commerce Order System

```yaml
Order:
  properties:
    customer_name: {type: string}
    total: {type: number}
    items:
      type: array
      x-relation:
        type: hasMany
        target: OrderItem
        foreignKey: order_id

OrderItem:
  properties:
    order_id: {type: integer}
    product_name: {type: string}
    quantity: {type: integer}
    price: {type: number}
```

```bash
curl -X POST /orders -d '{
  "customer_name": "Alice",
  "total": 299.99,
  "items": [
    {"product_name": "Laptop", "quantity": 1, "price": 299.99}
  ]
}'
```

### Blog Post with Comments

```yaml
Post:
  properties:
    title: {type: string}
    content: {type: string}
    comments:
      type: array
      x-relation:
        type: hasMany
        target: Comment
        foreignKey: post_id

Comment:
  properties:
    post_id: {type: integer}
    author: {type: string}
    text: {type: string}
```

```bash
curl -X POST /posts -d '{
  "title": "Hello World",
  "content": "My first post",
  "comments": [
    {"author": "Bob", "text": "Great post!"},
    {"author": "Carol", "text": "Thanks for sharing"}
  ]
}'
```

### Nested Addresses

```yaml
Customer:
  properties:
    name: {type: string}
    email: {type: string}
    addresses:
      type: array
      x-relation:
        type: hasMany
        target: Address
        foreignKey: customer_id

Address:
  properties:
    customer_id: {type: integer}
    street: {type: string}
    city: {type: string}
    country: {type: string}
```

```bash
curl -X POST /customers -d '{
  "name": "Alice Johnson",
  "email": "alice@example.com",
  "addresses": [
    {"street": "123 Main St", "city": "NYC", "country": "USA"},
    {"street": "456 Oak Ave", "city": "Boston", "country": "USA"}
  ]
}'
```

## Implementation Details

### Schema Generator

The `SchemaGenerator` extracts relation definitions from the OpenAPI spec:

```rust
pub struct RelationDefinition {
    pub field_name: String,        // "items"
    pub relation_type: RelationType, // HasMany
    pub target_table: String,      // "order_items"
    pub foreign_key: String,       // "order_id"
    pub local_key: Option<String>, // "id"
}
```

### CRUD Handler

The `handle_create` method processes nested relations:

1. Extract nested arrays from request body
2. Insert parent record
3. Get inserted parent ID
4. For each nested item:
   - Inject foreign key
   - Inject audit fields
   - Insert into target table

## Limitations (Current Version)

### 1. Transaction Support

Currently, there is no database-level transaction support. If nested record creation fails midway, partial data may be left in the database.

**Impact:** Low - errors during creation are rare, and failed records can be cleaned up manually.

**Future:** Full transaction support planned for Phase 4.

### 2. Complex Relation Updates

Nested updates replace ALL children:

```bash
# This REPLACES all items (deletes old ones, creates new ones)
curl -X PUT /orders/1 -d '{
  "customer_name": "Alice Updated",
  "items": [{"product_name": "New Item"}]
}'
```

Fine-grained updates (update specific children by ID) are not yet supported.

**Workaround:** Fetch current children, modify as needed, and send complete array in update.

### 3. Many-to-Many Relations

belongsToMany relations are not yet implemented.

**Workaround:** Manually manage junction tables and relations.

**Future:** Full belongsToMany support planned for Phase 4.

## Roadmap

### Phase 1 ✅ (Completed)

- [x] hasMany relation type
- [x] Automatic foreign key injection
- [x] Audit field injection for nested records
- [x] OpenAPI schema definition

### Phase 2 ✅ (Completed)

- [x] Fetch nested data on GET (automatic loading)
- [x] belongsTo relation type
- [x] hasOne relation type
- [x] Nested data in LIST operations

### Phase 3 🚧 (Partially Completed)

- [x] Update nested relations (hasMany & hasOne)
- [x] Cascade delete for nested relations
- [ ] Proper transaction support with rollback (deferred - requires architecture changes)

### Phase 4 (Future)

- [ ] belongsToMany (many-to-many) relations
- [ ] Eager loading configuration
- [ ] Lazy loading support
- [ ] Nested pagination
- [ ] Circular reference handling
- [ ] Soft delete support

## Best Practices

### 1. Use for Simple Relations

Nested objects work best for simple one-to-many relations with small datasets.

**Good:**
```json
{
  "order_name": "Order #123",
  "items": [
    {"product": "A", "qty": 1},
    {"product": "B", "qty": 2}
  ]
}
```

**Avoid:** Very large nested arrays (use pagination instead)

### 2. Define Foreign Keys in Schema

Always explicitly define foreign key columns:

```yaml
OrderItem:
  properties:
    order_id:
      type: integer
      description: Foreign key to orders table
```

### 3. Use Audit Fields

Enable authentication to track who created nested records:

```yaml
OrderItem:
  properties:
    createdBy: {type: string, readOnly: true}
    updatedBy: {type: string, readOnly: true}
```

### 4. Validate Before Creating

Validate nested data on the client side to avoid partial failures.

## Troubleshooting

### Foreign Key Not Injected

**Problem:** Nested items created without parent ID

**Solution:** Check `x-relation` definition:
```yaml
x-relation:
  type: hasMany
  target: OrderItem
  foreignKey: order_id  # ← Must match column name
```

### Nested Array Ignored

**Problem:** Items not created

**Solution:** Ensure field name matches `x-relation` field_name:
```yaml
properties:
  items:  # ← Must match request JSON key
    x-relation:
      field_name: items  # ← Not needed, inferred from property name
```

### Audit Fields Missing on Children

**Problem:** Parent has `createdBy` but children don't

**Solution:** Define audit fields in child schema:
```yaml
OrderItem:
  properties:
    createdBy: {type: string, readOnly: true}
```

## See Also

- [examples/relations](../examples/relations/) - Working example
- [AUDIT_TRAIL.md](AUDIT_TRAIL.md) - Audit field documentation
- [OpenAPI Specification](https://swagger.io/specification/) - OpenAPI reference
