package e2e_test

import (
	"bytes"
	"fmt"
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
		serverPort  = "3006"
		baseURL     = "http://localhost:" + serverPort
		configFile  string
		apiFile     string
		dbFile      string
		client      *http.Client
		tmpDir      string
	)

	BeforeEach(func() {
		var err error
		// Create temporary directory
		tmpDir, err = os.MkdirTemp("", "apify-validation-test")
		Expect(err).NotTo(HaveOccurred())

		configFile = filepath.Join(tmpDir, "config.yaml")
		apiFile = filepath.Join(tmpDir, "api.yaml")
		dbFile = filepath.Join(tmpDir, "test.sqlite")

		// Write API definition
		apiContent := `
openapi:
  validation:
    validate_request_body: true
  spec:
    openapi: "3.0.0"
    info:
      title: "Validation Test API"
      version: "1.0.0"
    paths:
      /users:
        post:
          summary: Create user
          x-table-name: users
          parameters:
            - name: x-request-id
              in: header
              required: true
              schema:
                type: string
                minLength: 5
            - name: dry_run
              in: query
              schema:
                type: boolean
            - name: limit
              in: query
              schema:
                type: integer
                minimum: 1
          requestBody:
            required: true
            content:
              application/json:
                schema:
                  type: object
                  required:
                    - name
                    - email
                  properties:
                    name:
                      type: string
                      minLength: 3
                    email:
                      type: string
                      format: email
                    age:
                      type: integer
                      minimum: 0
          responses:
            '200':
              description: Created
    components:
      schemas: {}
    x-table-schemas:
      - table_name: "users"
        columns:
          - name: "id"
            column_type: "INTEGER"
            primary_key: true
            auto_increment: true
            nullable: false
            unique: false
          - name: "name"
            column_type: "TEXT"
            nullable: false
            primary_key: false
            unique: false
            auto_increment: false
          - name: "email"
            column_type: "TEXT"
            nullable: false
            primary_key: false
            unique: false
            auto_increment: false
          - name: "age"
            column_type: "INTEGER"
            nullable: true
            primary_key: false
            unique: false
            auto_increment: false
        indexes: []
`
		err = os.WriteFile(apiFile, []byte(apiContent), 0644)
		Expect(err).NotTo(HaveOccurred())

		// Write Config
		configContent := fmt.Sprintf(`
listeners:
  - port: %s
    ip: 127.0.0.1
    protocol: HTTP
    apis:
      - path: %s
        datasource: sqlite1

consumers:
  - name: default
    keys:
      - test-key

datasource:
  sqlite1:
    driver: sqlite
    database: %s
    max_pool_size: 1

observability:
  log_level: "info"
  metrics_enabled: false
`, serverPort, apiFile, dbFile)
		err = os.WriteFile(configFile, []byte(configContent), 0644)
		Expect(err).NotTo(HaveOccurred())

		// Create empty DB file
		f, err := os.Create(dbFile)
		Expect(err).NotTo(HaveOccurred())
		f.Close()

		// Start Server
		wd, _ := os.Getwd()
		projectRoot := filepath.Dir(wd)
		
		serverCmd = exec.Command("cargo", "run", "--", "--config", configFile)
		serverCmd.Dir = projectRoot
		
		err = serverCmd.Start()
		Expect(err).NotTo(HaveOccurred())

		client = &http.Client{Timeout: 5 * time.Second}

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
		}, "30s", "1s").Should(Succeed(), "Server failed to start")
	})

	AfterEach(func() {
		if serverCmd != nil && serverCmd.Process != nil {
			serverCmd.Process.Kill()
			serverCmd.Wait()
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
