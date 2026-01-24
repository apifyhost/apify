package e2e_test

import (
	"bytes"
	"encoding/json"
	"net/http"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Auth Configs CRUD Operations", func() {
	var (
		env    *TestEnv
		client *http.Client
	)

	BeforeEach(func() {
		var err error
		env, client, err = SetupControlPlaneEnv()
		Expect(err).NotTo(HaveOccurred())
	})

	AfterEach(func() {
		if env != nil {
			env.Stop()
		}
	})

	Describe("Complete CRUD Lifecycle", func() {
		It("should support create, list, get, update, and delete operations", func() {
			baseURL := env.CPBaseURL + "/apify/admin/auth"

			// CREATE
			authConfig := map[string]interface{}{
				"type":    "api-key",
				"name":    "test-auth",
				"enabled": true,
				"config": map[string]interface{}{
					"key_name": "X-API-KEY",
					"consumers": []map[string]interface{}{
						{
							"name": "test-consumer",
							"keys": []string{"test-key-123"},
						},
					},
				},
			}
			body, _ := json.Marshal(authConfig)
			resp, err := client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusCreated))

			var createResult map[string]interface{}
			Expect(decodeJSON(resp, &createResult)).To(Succeed())
			authID := createResult["id"].(string)
			Expect(authID).NotTo(BeEmpty())

			// LIST
			resp, err = client.Get(baseURL)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var authList []map[string]interface{}
			Expect(decodeJSON(resp, &authList)).To(Succeed())
			Expect(len(authList)).To(BeNumerically(">", 0))

			// GET by ID
			resp, err = client.Get(baseURL + "/" + authID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var auth map[string]interface{}
			Expect(decodeJSON(resp, &auth)).To(Succeed())
			Expect(auth["id"]).To(Equal(authID))

			// UPDATE
			updatedConfig := map[string]interface{}{
				"type":    "api-key",
				"name":    "test-auth-updated",
				"enabled": false,
				"config": map[string]interface{}{
					"key_name": "X-API-KEY",
					"consumers": []map[string]interface{}{
						{
							"name": "updated-consumer",
							"keys": []string{"updated-key-456"},
						},
					},
				},
			}
			resp, err = putJSON(client, baseURL+"/"+authID, updatedConfig)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			// Verify update
			resp, err = client.Get(baseURL + "/" + authID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			var updatedAuth map[string]interface{}
			Expect(decodeJSON(resp, &updatedAuth)).To(Succeed())

			// Parse the config string to verify the update
			configStr, ok := updatedAuth["config"].(string)
			Expect(ok).To(BeTrue())
			var configObj map[string]interface{}
			err = json.Unmarshal([]byte(configStr), &configObj)
			Expect(err).NotTo(HaveOccurred())
			Expect(configObj["name"]).To(Equal("test-auth-updated"))
			Expect(configObj["enabled"]).To(BeFalse())

			// DELETE
			resp, err = deleteRequest(client, baseURL+"/"+authID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNoContent))

			// Verify deletion
			resp, err = client.Get(baseURL + "/" + authID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))
		})

		It("should return 404 for non-existent auth config operations", func() {
			baseURL := env.CPBaseURL + "/apify/admin/auth"
			fakeID := "non-existent-id"

			// GET non-existent
			resp, err := client.Get(baseURL + "/" + fakeID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))

			// UPDATE non-existent
			authConfig := map[string]interface{}{
				"type":    "api-key",
				"name":    "test",
				"enabled": true,
				"config": map[string]interface{}{
					"key_name":  "X-API-KEY",
					"consumers": []interface{}{},
				},
			}
			resp, err = putJSON(client, baseURL+"/"+fakeID, authConfig)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))

			// DELETE non-existent
			resp, err = deleteRequest(client, baseURL+"/"+fakeID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))
		})

		It("should reject duplicate auth config names", func() {
			baseURL := env.CPBaseURL + "/apify/admin/auth"

			authConfig := map[string]interface{}{
				"type":    "api-key",
				"name":    "duplicate-auth",
				"enabled": true,
				"config": map[string]interface{}{
					"key_name": "X-API-KEY",
					"consumers": []map[string]interface{}{
						{
							"name": "consumer",
							"keys": []string{"key-123"},
						},
					},
				},
			}

			// First creation
			body, _ := json.Marshal(authConfig)
			resp, err := client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusCreated))

			// Second creation with same name
			body, _ = json.Marshal(authConfig)
			resp, err = client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusConflict))
		})

		It("should handle OIDC auth config type", func() {
			baseURL := env.CPBaseURL + "/apify/admin/auth"

			// CREATE OIDC config
			authConfig := map[string]interface{}{
				"type":    "oidc",
				"name":    "oidc-auth",
				"enabled": true,
				"config": map[string]interface{}{
					"issuer_url":   "https://example.com/auth",
					"client_id":    "test-client",
					"redirect_uri": "http://localhost:3000/callback",
				},
			}
			body, _ := json.Marshal(authConfig)
			resp, err := client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusCreated))

			var createResult map[string]interface{}
			Expect(decodeJSON(resp, &createResult)).To(Succeed())
			authID := createResult["id"].(string)

			// GET and verify
			resp, err = client.Get(baseURL + "/" + authID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var auth map[string]interface{}
			Expect(decodeJSON(resp, &auth)).To(Succeed())
			Expect(auth["id"]).To(Equal(authID))
		})
	})
})
