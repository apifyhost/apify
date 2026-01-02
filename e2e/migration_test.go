package e2e_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Schema Migration", func() {
	var (
		env      *TestEnv
		baseURL  string
		client   *http.Client
		specFile string
	)

	// Helper to submit OpenAPI spec via API
	submitSpec := func(name, content string) {
		payload := map[string]string{
			"name":    name,
			"version": "1.0.0",
			"spec":    content,
		}
		body, _ := json.Marshal(payload)
		req, _ := http.NewRequest("POST", env.CPBaseURL+"/_meta/apis", bytes.NewBuffer(body))
		req.Header.Set("Content-Type", "application/json")

		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		if resp.StatusCode != 200 && resp.StatusCode != 201 {
			buf := new(bytes.Buffer)
			buf.ReadFrom(resp.Body)
			fmt.Printf("API Error: %s\n", buf.String())
		}
		Expect(resp.StatusCode).To(Or(Equal(200), Equal(201)))
		resp.Body.Close()
	}

	BeforeEach(func() {
		// Use a logical name for the API
		specFile = "api:products-api"

		// Initial Spec V1
		v1 := `
openapi: 3.0.0
info:
  title: Products API
  version: 1.0.0
paths:
  /products:
    post:
      summary: Create product
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/Product'
      responses:
        '200':
          description: Created
    get:
      summary: List products
      responses:
        '200':
          description: List
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Product'
components:
  schemas:
    Product:
      type: object
      properties:
        id:
          type: integer
          readOnly: true
        name:
          type: string
      x-table-schema:
        tableName: products
        columns:
          - name: id
            columnType: integer
            primaryKey: true
            autoIncrement: true
          - name: name
            columnType: text
            nullable: false
`
		// Start env with listener pointing to "products-api"
		// Note: StartTestEnv will try to import this. It will fail to read file "products-api",
		// but the listener will be configured to look for API named "products-api".
		env = StartTestEnv(map[string]string{
			"products": specFile,
		})
		baseURL = env.BaseURL
		client = &http.Client{
			Timeout: 10 * time.Second,
		}

		// Now submit the spec via API
		submitSpec("products-api", v1)

		// Wait for poller to pick it up
		time.Sleep(2 * time.Second)
	})

	AfterEach(func() {
		if env != nil {
			env.Stop()
		}
	})

	It("should migrate schema when adding a column", func() {
		// 1. Create a product (V1)
		product := map[string]interface{}{
			"name": "Laptop",
		}
		body, _ := json.Marshal(product)
		req, _ := http.NewRequest("POST", baseURL+"/products", bytes.NewBuffer(body))
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("X-Api-Key", env.APIKey)

		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200))
		resp.Body.Close()

		// 2. Update Spec to V2 (Add 'price' column)
		v2 := `
openapi: 3.0.0
info:
  title: Products API
  version: 1.0.0
paths:
  /products:
    post:
      summary: Create product
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/Product'
      responses:
        '200':
          description: Created
    get:
      summary: List products
      responses:
        '200':
          description: List
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Product'
components:
  schemas:
    Product:
      type: object
      properties:
        id:
          type: integer
          readOnly: true
        name:
          type: string
        price:
          type: number
      x-table-schema:
        tableName: products
        columns:
          - name: id
            columnType: integer
            primaryKey: true
            autoIncrement: true
          - name: name
            columnType: text
            nullable: false
          - name: price
            columnType: real
            nullable: true
`
		// 3. Submit updated spec via API
		submitSpec("products-api", v2)

		// Wait for reload (poll interval is 1s, give it a bit more)
		time.Sleep(3 * time.Second)

		// 4. Create a product with new field (V2)
		product2 := map[string]interface{}{
			"name":  "Mouse",
			"price": 29.99,
		}
		body, _ = json.Marshal(product2)
		req, _ = http.NewRequest("POST", baseURL+"/products", bytes.NewBuffer(body))
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("X-Api-Key", env.APIKey)

		resp, err = client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200))
		resp.Body.Close()

		// 5. Verify both products
		req, _ = http.NewRequest("GET", baseURL+"/products", nil)
		req.Header.Set("X-Api-Key", env.APIKey)
		resp, err = client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200))

		var products []map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&products)
		resp.Body.Close()

		Expect(len(products)).To(Equal(2))

		// Check first product (should have null price or missing)
		p1 := products[0]
		Expect(p1["name"]).To(Equal("Laptop"))
		// Price might be nil or 0 depending on implementation, but it shouldn't crash

		// Check second product
		p2 := products[1]
		Expect(p2["name"]).To(Equal("Mouse"))
		Expect(p2["price"]).To(Equal(29.99))
	})
})
