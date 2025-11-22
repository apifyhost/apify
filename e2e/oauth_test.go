package e2e_test

import (
	"encoding/json"
	"io"
	"net/http"
	"net/url"
	"os"
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
		baseURL       string
		keycloakURL   string
		client        *http.Client
		accessToken   string
		clientID      = "apify-test-client"
		clientSecret  = "apify-test-secret"
		username      = "testuser"
		password      = "testpassword"
		realm         = "apify"
	)

	BeforeEach(func() {
		baseURL = os.Getenv("BASE_URL")
		if baseURL == "" {
			baseURL = "http://localhost:3000"
		}

		keycloakURL = os.Getenv("KEYCLOAK_URL")
		if keycloakURL == "" {
			keycloakURL = "http://localhost:8080"
		}

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
			createReq.Header.Set("X-Api-Key", os.Getenv("API_KEY"))
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
			listReq.Header.Set("X-Api-Key", os.Getenv("API_KEY"))
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
})
