package e2e_test

import (
	"net/http"
	"os"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("OpenAPI Security Scheme", func() {
	var (
		baseURL string
		apiKey  string
		client  *http.Client
	)

	BeforeEach(func() {
		baseURL = os.Getenv("BASE_URL")
		if baseURL == "" {
			baseURL = "http://localhost:3000"
		}

		apiKey = os.Getenv("API_KEY")
		if apiKey == "" {
			apiKey = "e2e-test-key-001"
		}

		client = &http.Client{
			Timeout: 10 * time.Second,
		}
	})

	Describe("Security Scheme - ApiKeyAuth", func() {
		Context("when operations use standard OpenAPI security", func() {
			It("should enforce authentication via securitySchemes", func() {
				// Test GET /items - should require auth
				req, err := http.NewRequest("GET", baseURL+"/items", nil)
				Expect(err).NotTo(HaveOccurred())

				// Without API key - should fail
				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()
				Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized), 
					"GET /items without X-Api-Key should return 401")

				// With valid API key - should succeed
				req.Header.Set("X-Api-Key", apiKey)
				resp, err = client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()
				Expect(resp.StatusCode).To(Equal(http.StatusOK),
					"GET /items with valid X-Api-Key should return 200")
			})

			It("should apply global security when operation has no local security", func() {
				// If global security is defined in the spec, all operations
				// should inherit it unless explicitly overridden
				req, err := http.NewRequest("POST", baseURL+"/items", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("Content-Type", "application/json")

				// Without API key
				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()
				Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized),
					"POST /items should inherit security from global config")
			})

			It("should validate API key format from securitySchemes", func() {
				// Test with invalid key format
				req, err := http.NewRequest("GET", baseURL+"/items", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("X-Api-Key", "wrong-key")

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()
				Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized),
					"Invalid API key should be rejected")
			})
		})

		Context("backward compatibility with x-modules", func() {
			It("should still support legacy x-modules if present", func() {
				// This test ensures backward compatibility
				// In the future, specs might still have x-modules during migration
				// Both mechanisms (security + x-modules) should work
				req, err := http.NewRequest("GET", baseURL+"/items", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("X-Api-Key", apiKey)

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()
				Expect(resp.StatusCode).To(Equal(http.StatusOK),
					"Authentication should work regardless of using security or x-modules")
			})
		})

		Context("when endpoint requires no authentication", func() {
			It("should allow public access to healthz", func() {
				// Healthz should always be accessible without auth
				req, err := http.NewRequest("GET", baseURL+"/healthz", nil)
				Expect(err).NotTo(HaveOccurred())

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()
				Expect(resp.StatusCode).To(Equal(http.StatusOK),
					"/healthz should be accessible without authentication")
			})
		})

		Context("security requirements at different levels", func() {
			It("should apply operation-level security over global security", func() {
				// Operation-level security should override global defaults
				// This test documents the precedence: operation > global
				req, err := http.NewRequest("PUT", baseURL+"/items/1", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("Content-Type", "application/json")

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()
				Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized),
					"PUT /items/{id} should require authentication per operation-level security")
			})
		})
	})

	Describe("Security Scheme Configuration", func() {
		It("should reject requests with missing required header", func() {
			// securityScheme defines "in: header, name: X-Api-Key"
			req, err := http.NewRequest("DELETE", baseURL+"/items/1", nil)
			Expect(err).NotTo(HaveOccurred())

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized),
				"Missing X-Api-Key header should be rejected")
		})

		It("should accept requests with proper header name", func() {
			// Verify the exact header name defined in securityScheme
			req, err := http.NewRequest("GET", baseURL+"/items", nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey) // Must match "name: X-Api-Key"

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK),
				"Request with correct X-Api-Key header should succeed")
		})
	})
})
