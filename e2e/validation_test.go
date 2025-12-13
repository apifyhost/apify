package e2e_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("OpenAPI Validation", func() {
	var (
		serverCmd   *exec.Cmd
		cpCmd       *exec.Cmd
		serverPort  string
		baseURL     string
		configFile  string
		dbFile      string
		client      *http.Client
		tmpDir      string
	)

	BeforeEach(func() {
		var err error
		// Get a free port
		l, err := net.Listen("tcp", "127.0.0.1:0")
		Expect(err).NotTo(HaveOccurred())
		serverPort = fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
		l.Close()
		baseURL = "http://127.0.0.1:" + serverPort

		// Create temporary directory
		tmpDir, err = os.MkdirTemp("", "apify-validation-test")
		Expect(err).NotTo(HaveOccurred())

		configFile = filepath.Join(tmpDir, "config.yaml")
		dbFile = filepath.Join(tmpDir, "test.sqlite")

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

		// Write Config
		configContent := fmt.Sprintf(`
control-plane:
  listen:
    ip: 127.0.0.1
    port: %s
  database:
    driver: sqlite
    database: //%s

listeners:
  - port: %s
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - validation-api

consumers:
  - name: default
    keys:
      - test-key

datasource:
  sqlite1:
    driver: sqlite
    database: //%s
    max_pool_size: 1

log_level: "info"

modules:
  tracing:
    enabled: true
  metrics:
    enabled: false
`, serverPort, dbFile, serverPort, dbFile)
		err = os.WriteFile(configFile, []byte(configContent), 0644)
		Expect(err).NotTo(HaveOccurred())

		// Create empty DB file
		f, err := os.Create(dbFile)
		Expect(err).NotTo(HaveOccurred())
		f.Close()

		// Start Server (Control Plane)
		wd, _ := os.Getwd()
		projectRoot := filepath.Dir(wd)
		
		cpCmd = exec.Command("cargo", "run", "--bin", "apify-cp", "--", "--config", configFile)
		cpCmd.Dir = projectRoot
		cpCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+dbFile)
		cpCmd.Stdout = GinkgoWriter
		cpCmd.Stderr = GinkgoWriter
		
		err = cpCmd.Start()
		Expect(err).NotTo(HaveOccurred())

		client = &http.Client{Timeout: 5 * time.Second}

		// Wait for CP to be ready
		Eventually(func() error {
			resp, err := client.Get(baseURL + "/_meta/apis")
			if err != nil {
				return err
			}
			defer resp.Body.Close()
			if resp.StatusCode != 200 {
				return fmt.Errorf("status code %d", resp.StatusCode)
			}
			return nil
		}, 300*time.Second, 1*time.Second).Should(Succeed())

		// Post API Spec
		// Wrap the spec in the expected payload format
		var specObj map[string]interface{}
		err = json.Unmarshal([]byte(apiSpecJSON), &specObj)
		Expect(err).NotTo(HaveOccurred())

		payload := map[string]interface{}{
			"name":    "validation-api",
			"version": "1.0.0",
			"spec":    specObj,
		}
		payloadBytes, err := json.Marshal(payload)
		Expect(err).NotTo(HaveOccurred())

		resp, err := client.Post(baseURL+"/_meta/apis", "application/json", bytes.NewBuffer(payloadBytes))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(201))
		resp.Body.Close()

		// Stop Control Plane
		if cpCmd.Process != nil {
			cpCmd.Process.Kill()
			cpCmd.Wait()
		}

		// Start Server (Data Plane)
		serverCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--config", configFile)
		serverCmd.Dir = projectRoot
		serverCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+dbFile)
		serverCmd.Stdout = GinkgoWriter
		serverCmd.Stderr = GinkgoWriter
		
		err = serverCmd.Start()
		Expect(err).NotTo(HaveOccurred())

		// Wait for server to be ready
		Eventually(func() error {
			resp, err := client.Get(baseURL + "/healthz")
			if err != nil {
				return err
			}
			defer resp.Body.Close()
			if resp.StatusCode != 200 {
				return fmt.Errorf("status %d", resp.StatusCode)
			}
			return nil
		}, "120s", "1s").Should(Succeed(), "Server failed to start")
	})

	AfterEach(func() {
		if serverCmd != nil && serverCmd.Process != nil {
			serverCmd.Process.Kill()
			serverCmd.Wait()
		}
		if cpCmd != nil && cpCmd.Process != nil {
			cpCmd.Process.Kill()
			cpCmd.Wait()
		}
		if tmpDir != "" {
			os.RemoveAll(tmpDir)
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
