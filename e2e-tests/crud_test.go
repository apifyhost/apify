package e2e_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Apify CRUD Operations", func() {
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

		// Verify service is ready (should already be started by the workflow)
		Eventually(func() error {
			resp, err := client.Get(baseURL + "/healthz")
			if err != nil {
				return fmt.Errorf("failed to connect: %w", err)
			}
			defer resp.Body.Close()

			if resp.StatusCode != http.StatusOK {
				return fmt.Errorf("health check failed with status %d", resp.StatusCode)
			}
			return nil
		}, "10s", "500ms").Should(Succeed(), "Service should be ready")
	})

	Describe("Health Check", func() {
		It("should return 200 OK for health endpoint", func() {
			resp, err := client.Get(baseURL + "/healthz")
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))
		})
	})

	Describe("Authentication", func() {
		Context("when API key is not provided", func() {
			It("should return 401 Unauthorized", func() {
				req, err := http.NewRequest("GET", baseURL+"/items", nil)
				Expect(err).NotTo(HaveOccurred())

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized))
			})
		})

		Context("when invalid API key is provided", func() {
			It("should return 401 Unauthorized", func() {
				req, err := http.NewRequest("GET", baseURL+"/items", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("X-Api-Key", "invalid-key")

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized))
			})
		})

		Context("when valid API key is provided", func() {
			It("should allow access", func() {
				req, err := http.NewRequest("GET", baseURL+"/items", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("X-Api-Key", apiKey)

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusOK))
			})
		})
	})

	Describe("CRUD Operations", Ordered, func() {
		var itemID int64

		It("should list empty items initially", func() {
			req, err := http.NewRequest("GET", baseURL+"/items", nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var items []map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&items)
			Expect(err).NotTo(HaveOccurred())
		})

		It("should create a new item", func() {
			payload := map[string]interface{}{
				"name":        "Test Item",
				"description": "E2E test item",
				"price":       99.99,
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("POST", baseURL+"/items", bytes.NewBuffer(jsonData))
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)
			req.Header.Set("Content-Type", "application/json")

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var result map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&result)
			Expect(err).NotTo(HaveOccurred())
			Expect(result["affected_rows"]).To(BeNumerically(">=", 1))
		})

		It("should list the created item", func() {
			req, err := http.NewRequest("GET", baseURL+"/items", nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var items []map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&items)
			Expect(err).NotTo(HaveOccurred())
			Expect(items).NotTo(BeEmpty())

			// Store the first item's ID for later tests
			itemID = int64(items[0]["id"].(float64))
			Expect(itemID).To(BeNumerically(">", 0))
			Expect(items[0]["name"]).To(Equal("Test Item"))
		})

		It("should get item by ID", func() {
			req, err := http.NewRequest("GET", fmt.Sprintf("%s/items/%d", baseURL, itemID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var item map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&item)
			Expect(err).NotTo(HaveOccurred())
			Expect(item["id"]).To(BeNumerically("==", itemID))
			Expect(item["name"]).To(Equal("Test Item"))
			Expect(item["price"]).To(BeNumerically("==", 99.99))
		})

		It("should update the item", func() {
			payload := map[string]interface{}{
				"name":  "Updated Item",
				"price": 149.99,
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("PUT", fmt.Sprintf("%s/items/%d", baseURL, itemID), bytes.NewBuffer(jsonData))
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)
			req.Header.Set("Content-Type", "application/json")

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))
		})

		It("should verify the update", func() {
			req, err := http.NewRequest("GET", fmt.Sprintf("%s/items/%d", baseURL, itemID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var item map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&item)
			Expect(err).NotTo(HaveOccurred())
			Expect(item["name"]).To(Equal("Updated Item"))
			Expect(item["price"]).To(BeNumerically("==", 149.99))
		})

		It("should create a second item", func() {
			payload := map[string]interface{}{
				"name":  "Second Item",
				"price": 49.99,
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("POST", baseURL+"/items", bytes.NewBuffer(jsonData))
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)
			req.Header.Set("Content-Type", "application/json")

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))
		})

		It("should list multiple items", func() {
			req, err := http.NewRequest("GET", baseURL+"/items", nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var items []map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&items)
			Expect(err).NotTo(HaveOccurred())
			Expect(len(items)).To(BeNumerically(">=", 2))
		})

		It("should delete the item", func() {
			req, err := http.NewRequest("DELETE", fmt.Sprintf("%s/items/%d", baseURL, itemID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))
		})

		It("should return 404 for deleted item", func() {
			req, err := http.NewRequest("GET", fmt.Sprintf("%s/items/%d", baseURL, itemID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))
		})
	})

	Describe("Large Payload Handling", func() {
		It("should handle large description fields", func() {
			// Create a large description (1000 'x' characters)
			largeDescription := make([]byte, 1000)
			for i := range largeDescription {
				largeDescription[i] = 'x'
			}

			payload := map[string]interface{}{
				"name":        "Large Item",
				"description": string(largeDescription),
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("POST", baseURL+"/items", bytes.NewBuffer(jsonData))
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)
			req.Header.Set("Content-Type", "application/json")

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))
		})
	})

	Describe("Content-Type Validation", func() {
		It("should accept requests with proper Content-Type", func() {
			payload := map[string]interface{}{
				"name": "Content Type Test",
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("POST", baseURL+"/items", bytes.NewBuffer(jsonData))
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)
			req.Header.Set("Content-Type", "application/json")

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))
		})
	})
})

// Helper function to read response body
func readBody(body io.ReadCloser) string {
	data, _ := io.ReadAll(body)
	return string(data)
}
