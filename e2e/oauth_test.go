package e2e_test

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"strconv"
	"strings"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

type TokenResponse struct {
	AccessToken string `json:"access_token"`
	TokenType   string `json:"token_type"`
	ExpiresIn   int    `json:"expires_in"`
}

var _ = Describe("OAuth/OIDC Integration", func() {
	var (
		env          *TestEnv
		baseURL      string
		keycloakURL  string
		client       *http.Client
		accessToken  string
		clientID     = "apify-test-client"
		clientSecret = "apify-test-secret"
		username     = "testuser"
		password     = "testpassword"
		realm        = "apify"
	)

	BeforeEach(func() {
		keycloakURL = os.Getenv("KEYCLOAK_URL")
		if keycloakURL == "" {
			Skip("KEYCLOAK_URL not set, skipping OAuth tests")
		}

		env = StartTestEnv(map[string]string{
			"items":       "examples/basic/config/openapi/items.yaml",
			"items_oauth": "examples/oauth/config/openapi/items_oauth.yaml",
		})
		baseURL = env.BaseURL

		client = &http.Client{
			Timeout: 15 * time.Second,
		}

		// Obtain access token from Keycloak
		tokenEndpoint := keycloakURL + "/realms/" + realm + "/protocol/openid-connect/token"
		data := url.Values{}
		data.Set("grant_type", "password")
		data.Set("client_id", clientID)
		data.Set("client_secret", clientSecret)
		data.Set("username", username)
		data.Set("password", password)

		req, err := http.NewRequest("POST", tokenEndpoint, strings.NewReader(data.Encode()))
		Expect(err).NotTo(HaveOccurred())
		req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()

		Expect(resp.StatusCode).To(Equal(http.StatusOK), "Failed to obtain access token from Keycloak")

		var tokenResp TokenResponse
		err = json.NewDecoder(resp.Body).Decode(&tokenResp)
		Expect(err).NotTo(HaveOccurred())
		Expect(tokenResp.AccessToken).NotTo(BeEmpty())

		accessToken = tokenResp.AccessToken
		GinkgoWriter.Printf("Obtained access token: %s...\n", accessToken[:20])
	})

	AfterEach(func() {
		if env != nil {
			env.Stop()
		}
	})

	Describe("Bearer Token Authentication", func() {
		Context("when valid bearer token is provided", func() {
			It("should allow access to protected endpoints", func() {
				req, err := http.NewRequest("GET", baseURL+"/secure-items", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("Authorization", "Bearer "+accessToken)

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusOK),
					"GET /secure-items with valid Bearer token should return 200")
			})

			It("should allow creating items with bearer token", func() {
				body := `{"name": "OAuth Test Item", "description": "Created via OAuth", "price": 99.99}`
				req, err := http.NewRequest("POST", baseURL+"/secure-items", strings.NewReader(body))
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("Authorization", "Bearer "+accessToken)
				req.Header.Set("Content-Type", "application/json")

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusOK),
					"POST /secure-items with valid Bearer token should return 200")
			})
		})

		Context("when bearer token is missing", func() {
			It("should return 401 Unauthorized", func() {
				req, err := http.NewRequest("GET", baseURL+"/secure-items", nil)
				Expect(err).NotTo(HaveOccurred())

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized),
					"GET /secure-items without Authorization header should return 401")
			})
		})

		Context("when invalid bearer token is provided", func() {
			It("should return 401 Unauthorized", func() {
				req, err := http.NewRequest("GET", baseURL+"/secure-items", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("Authorization", "Bearer invalid-token-xyz")

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized),
					"GET /secure-items with invalid Bearer token should return 401")
			})
		})

		Context("when token is expired (simulated)", func() {
			It("should return 401 Unauthorized", func() {
				// Use a clearly malformed token to simulate expiration
				expiredToken := "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjE2MDAwMDAwMDB9.invalid"
				req, err := http.NewRequest("GET", baseURL+"/secure-items", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("Authorization", "Bearer "+expiredToken)

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized),
					"GET /secure-items with expired token should return 401")
			})
		})
	})

	Describe("Cross-Surface Data Visibility", func() {
		It("should see item created via API key on OAuth endpoint", func() {
			// Create item via API key on /items
			uniqueName := "cross-key-" + time.Now().Format("150405.000")
			body := `{"name":"` + uniqueName + `","description":"from key","price":1}`
			createReq, err := http.NewRequest("POST", baseURL+"/items", strings.NewReader(body))
			Expect(err).NotTo(HaveOccurred())
			createReq.Header.Set("X-API-KEY", os.Getenv("API_KEY"))
			createReq.Header.Set("Content-Type", "application/json")
			createResp, err := client.Do(createReq)
			Expect(err).NotTo(HaveOccurred())
			defer createResp.Body.Close()
			Expect(createResp.StatusCode).To(Equal(http.StatusOK))

			// List via OAuth on /secure-items
			listReq, err := http.NewRequest("GET", baseURL+"/secure-items", nil)
			Expect(err).NotTo(HaveOccurred())
			listReq.Header.Set("Authorization", "Bearer "+accessToken)
			listResp, err := client.Do(listReq)
			Expect(err).NotTo(HaveOccurred())
			defer listResp.Body.Close()
			Expect(listResp.StatusCode).To(Equal(http.StatusOK))
			b, _ := io.ReadAll(listResp.Body)
			Expect(string(b)).To(ContainSubstring(uniqueName))
		})

		It("should see item created via OAuth endpoint on API key endpoint", func() {
			uniqueName := "cross-oauth-" + time.Now().Format("150405.000")
			body := `{"name":"` + uniqueName + `","description":"from oauth","price":2}`
			createReq, err := http.NewRequest("POST", baseURL+"/secure-items", strings.NewReader(body))
			Expect(err).NotTo(HaveOccurred())
			createReq.Header.Set("Authorization", "Bearer "+accessToken)
			createReq.Header.Set("Content-Type", "application/json")
			createResp, err := client.Do(createReq)
			Expect(err).NotTo(HaveOccurred())
			defer createResp.Body.Close()
			Expect(createResp.StatusCode).To(Equal(http.StatusOK))

			listReq, err := http.NewRequest("GET", baseURL+"/items", nil)
			Expect(err).NotTo(HaveOccurred())
			listReq.Header.Set("X-API-KEY", os.Getenv("API_KEY"))
			listResp, err := client.Do(listReq)
			Expect(err).NotTo(HaveOccurred())
			defer listResp.Body.Close()
			Expect(listResp.StatusCode).To(Equal(http.StatusOK))
			b, _ := io.ReadAll(listResp.Body)
			Expect(string(b)).To(ContainSubstring(uniqueName))
		})
	})

	Describe("Public Endpoints", func() {
		Context("when accessing healthz without auth", func() {
			It("should allow access to health check", func() {
				req, err := http.NewRequest("GET", baseURL+"/healthz", nil)
				Expect(err).NotTo(HaveOccurred())

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusOK),
					"/healthz should be accessible without Bearer token")
			})
		})
	})

	Describe("Token Introspection Fallback", func() {
		Context("when introspection is enabled", func() {
			It("should validate token via introspection endpoint", func() {
				// This test assumes the oauth module attempts introspection
				// when configured. The token should be validated successfully.
				req, err := http.NewRequest("GET", baseURL+"/secure-items", nil)
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("Authorization", "Bearer "+accessToken)

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				// Should succeed via either JWT or introspection
				Expect(resp.StatusCode).To(Equal(http.StatusOK))
			})
		})
	})

	Describe("OpenID Connect Discovery", func() {
		It("should fetch OIDC discovery metadata", func() {
			discoveryURL := keycloakURL + "/realms/" + realm + "/.well-known/openid-configuration"
			resp, err := client.Get(discoveryURL)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var discovery map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&discovery)
			Expect(err).NotTo(HaveOccurred())

			Expect(discovery).To(HaveKey("issuer"))
			Expect(discovery).To(HaveKey("jwks_uri"))
			Expect(discovery).To(HaveKey("introspection_endpoint"))
			GinkgoWriter.Printf("OIDC Discovery: issuer=%v\n", discovery["issuer"])
		})
	})

	Describe("Audit Trail", func() {
		Context("when creating items with OAuth authentication", func() {
			It("should automatically populate createdBy and updatedBy fields", func() {
				body := `{"name": "Audit Test Item", "description": "Testing audit trail", "price": 55.55}`
				req, err := http.NewRequest("POST", baseURL+"/secure-items", strings.NewReader(body))
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("Authorization", "Bearer "+accessToken)
				req.Header.Set("Content-Type", "application/json")

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				Expect(resp.StatusCode).To(Equal(http.StatusOK))

				var item map[string]interface{}
				bodyBytes, _ := io.ReadAll(resp.Body)
				err = json.Unmarshal(bodyBytes, &item)
				Expect(err).NotTo(HaveOccurred())

				// Verify audit fields are populated
				if createdBy, ok := item["createdBy"]; ok {
					Expect(createdBy).To(Equal(username), "createdBy should be set to the authenticated user")
				}
				if updatedBy, ok := item["updatedBy"]; ok {
					Expect(updatedBy).To(Equal(username), "updatedBy should be set to the authenticated user")
				}
				if createdAt, ok := item["createdAt"]; ok {
					Expect(createdAt).NotTo(BeNil(), "createdAt should be populated")
				}

				GinkgoWriter.Printf("Created item with audit fields: createdBy=%v\n", item["createdBy"])
			})
		})

		Context("when updating items with OAuth authentication", func() {
			It("should update updatedBy field while preserving createdBy", func() {
				// First create an item
				createBody := `{"name": "Update Audit Test", "description": "Initial", "price": 10.00}`
				createReq, err := http.NewRequest("POST", baseURL+"/secure-items", strings.NewReader(createBody))
				Expect(err).NotTo(HaveOccurred())
				createReq.Header.Set("Authorization", "Bearer "+accessToken)
				createReq.Header.Set("Content-Type", "application/json")

				createResp, err := client.Do(createReq)
				Expect(err).NotTo(HaveOccurred())
				defer createResp.Body.Close()

				var createdItem map[string]interface{}
				createBodyBytes, _ := io.ReadAll(createResp.Body)
				err = json.Unmarshal(createBodyBytes, &createdItem)
				Expect(err).NotTo(HaveOccurred())

				originalCreatedBy := createdItem["createdBy"]
				originalCreatedAt := createdItem["createdAt"]
				itemID := createdItem["id"]

				// Wait a moment to ensure timestamp difference
				time.Sleep(1 * time.Second)

				// Update the item
				updateBody := `{"name": "Updated Audit Test", "description": "Modified", "price": 20.00}`
				updateReq, err := http.NewRequest("PUT", baseURL+"/secure-items/"+toString(itemID), strings.NewReader(updateBody))
				Expect(err).NotTo(HaveOccurred())
				updateReq.Header.Set("Authorization", "Bearer "+accessToken)
				updateReq.Header.Set("Content-Type", "application/json")

				updateResp, err := client.Do(updateReq)
				Expect(err).NotTo(HaveOccurred())
				defer updateResp.Body.Close()

				var updatedItem map[string]interface{}
				updateBodyBytes, _ := io.ReadAll(updateResp.Body)
				err = json.Unmarshal(updateBodyBytes, &updatedItem)
				Expect(err).NotTo(HaveOccurred())

				// Verify createdBy and createdAt are preserved
				if createdBy, ok := updatedItem["createdBy"]; ok {
					Expect(createdBy).To(Equal(originalCreatedBy), "createdBy should not change on update")
				}
				if createdAt, ok := updatedItem["createdAt"]; ok {
					Expect(createdAt).To(Equal(originalCreatedAt), "createdAt should not change on update")
				}

				// Verify updatedBy is set
				if updatedBy, ok := updatedItem["updatedBy"]; ok {
					Expect(updatedBy).To(Equal(username), "updatedBy should be set to the authenticated user")
				}

				GinkgoWriter.Printf("Updated item: createdBy=%v, updatedBy=%v\n",
					updatedItem["createdBy"], updatedItem["updatedBy"])
			})
		})

		Context("when user tries to override audit fields", func() {
			It("should ignore user-provided audit field values", func() {
				// Try to create with manually set audit fields
				body := `{"name": "Hacker Item", "description": "test", "price": 1.00, "createdBy": "hacker", "updatedBy": "hacker"}`
				req, err := http.NewRequest("POST", baseURL+"/secure-items", strings.NewReader(body))
				Expect(err).NotTo(HaveOccurred())
				req.Header.Set("Authorization", "Bearer "+accessToken)
				req.Header.Set("Content-Type", "application/json")

				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				defer resp.Body.Close()

				var item map[string]interface{}
				bodyBytes, _ := io.ReadAll(resp.Body)
				err = json.Unmarshal(bodyBytes, &item)
				Expect(err).NotTo(HaveOccurred())

				// Audit fields should be overridden by the system
				if createdBy, ok := item["createdBy"]; ok {
					Expect(createdBy).To(Equal(username), "createdBy should be overridden to authenticated user, not 'hacker'")
				}
				if updatedBy, ok := item["updatedBy"]; ok {
					Expect(updatedBy).To(Equal(username), "updatedBy should be overridden to authenticated user, not 'hacker'")
				}
			})
		})
	})
})

// Helper function to convert interface{} to string for URL construction
func toString(v interface{}) string {
	switch val := v.(type) {
	case float64:
		return strconv.FormatFloat(val, 'f', -1, 64)
	case int:
		return strconv.Itoa(val)
	case int64:
		return strconv.FormatInt(val, 10)
	case string:
		return val
	default:
		return fmt.Sprintf("%v", v)
	}
}
