package e2e_test

import (
	"bytes"
	"net/http"
	"os"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("OpenAPI Validation", func() {
	var (
		env     *TestEnv
		baseURL string
		client  *http.Client
	)

	BeforeEach(func() {
		// Write API definition
		apiSpecJSON := `
{
  "openapi": "3.0.0",
  "info": {
    "title": "Validation Test API",
    "version": "1.0.0"
  },
  "paths": {
    "/users": {
      "post": {
        "summary": "Create user",
        "x-table-name": "users",
        "parameters": [
          {
            "name": "x-request-id",
            "in": "header",
            "required": true,
            "schema": {
              "type": "string",
              "minLength": 5
            }
          },
          {
            "name": "dry_run",
            "in": "query",
            "schema": {
              "type": "boolean"
            }
          },
          {
            "name": "limit",
            "in": "query",
            "schema": {
              "type": "integer",
              "minimum": 1
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "name",
                  "email"
                ],
                "properties": {
                  "name": {
                    "type": "string",
                    "minLength": 3
                  },
                  "email": {
                    "type": "string",
                    "format": "email"
                  },
                  "age": {
                    "type": "integer",
                    "minimum": 0
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Created"
          }
        }
      }
    }
  },
  "components": {
    "schemas": {
    }
  },
  "x-table-schemas": [
    {
      "table_name": "users",
      "columns": [
        {
          "name": "id",
          "column_type": "INTEGER",
          "primary_key": true,
          "auto_increment": true,
          "nullable": false,
          "unique": false
        },
        {
          "name": "name",
          "column_type": "TEXT",
          "nullable": false,
          "primary_key": false,
          "unique": false,
          "auto_increment": false
        },
        {
          "name": "email",
          "column_type": "TEXT",
          "nullable": false,
          "primary_key": false,
          "unique": false,
          "auto_increment": false
        },
        {
          "name": "age",
          "column_type": "INTEGER",
          "nullable": true,
          "primary_key": false,
          "unique": false,
          "auto_increment": false
        }
      ],
      "indexes": [

      ]
    }
  ]
}
`

		// Create a temporary file for the spec
		tmpFile, err := os.CreateTemp("", "validation-api-*.json")
		Expect(err).NotTo(HaveOccurred())
		defer tmpFile.Close()

		_, err = tmpFile.WriteString(apiSpecJSON)
		Expect(err).NotTo(HaveOccurred())

		env = StartTestEnv(map[string]string{
			"validation-api": tmpFile.Name(),
		})
		baseURL = env.BaseURL
		client = &http.Client{Timeout: 5 * time.Second}
	})

	AfterEach(func() {
		if env != nil {
			env.Stop()
		}
	})

	It("should accept valid requests with headers and query params", func() {
		body := []byte(`{"name": "Alice", "email": "alice@example.com", "age": 30}`)
		req, _ := http.NewRequest("POST", baseURL+"/users?dry_run=true&limit=10", bytes.NewBuffer(body))
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("x-request-id", "12345")

		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusOK))
	})

	It("should reject requests missing required header", func() {
		body := []byte(`{"name": "Alice", "email": "alice@example.com"}`)
		req, _ := http.NewRequest("POST", baseURL+"/users", bytes.NewBuffer(body))
		req.Header.Set("Content-Type", "application/json")
		// Missing x-request-id

		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusBadRequest))
	})

	It("should reject requests with invalid header format", func() {
		body := []byte(`{"name": "Alice", "email": "alice@example.com"}`)
		req, _ := http.NewRequest("POST", baseURL+"/users", bytes.NewBuffer(body))
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("x-request-id", "123") // Too short (min 5)

		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusBadRequest))
	})

	It("should reject requests with invalid query param type", func() {
		body := []byte(`{"name": "Alice", "email": "alice@example.com"}`)
		req, _ := http.NewRequest("POST", baseURL+"/users?limit=notanumber", bytes.NewBuffer(body))
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("x-request-id", "12345")

		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusBadRequest))
	})

	It("should reject requests with invalid query param constraint", func() {
		body := []byte(`{"name": "Alice", "email": "alice@example.com"}`)
		req, _ := http.NewRequest("POST", baseURL+"/users?limit=0", bytes.NewBuffer(body)) // min 1
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("x-request-id", "12345")

		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusBadRequest))
	})
})
