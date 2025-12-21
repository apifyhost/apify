# Relations and Nested Objects

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

### hasOne (One-to-One)

**Use case:** A parent record has a single related record

**Example:** User has one Profile

```yaml
User:
  properties:
    profile:
      type: object
      x-relation:
        type: hasOne
        target: Profile
        foreignKey: user_id
```

### belongsTo (Many-to-One)

**Use case:** A child record references a parent record

**Example:** OrderItem belongs to Product

```yaml
OrderItem:
  properties:
    product:
      type: object
      x-relation:
        type: belongsTo
        target: Product
        foreignKey: product_id
```
