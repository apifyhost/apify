package e2e_test

import (
	"bytes"
	"encoding/json"
	"net/http"
	"os"
	"path/filepath"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("APIs CRUD Operations", func() {
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
			baseURL := env.CPBaseURL + "/apify/admin/apis"

			// CREATE
			apiSpec := map[string]interface{}{
				"openapi": "3.0.0",
				"info": map[string]interface{}{
					"title":   "Test API",
					"version": "1.0.0",
				},
				"paths": map[string]interface{}{
					"/test": map[string]interface{}{
						"get": map[string]interface{}{
							"responses": map[string]interface{}{
								"200": map[string]interface{}{
									"description": "Success",
								},
							},
						},
					},
				},
			}

			apiConfig := map[string]interface{}{
				"name":    "test-api",
				"version": "1.0.0",
				"spec":    apiSpec,
			}
			body, _ := json.Marshal(apiConfig)
			resp, err := client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusCreated))

			var createResult map[string]interface{}
			Expect(decodeJSON(resp, &createResult)).To(Succeed())
			apiID := createResult["id"].(string)
			Expect(apiID).NotTo(BeEmpty())

			// LIST
			resp, err = client.Get(baseURL)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var apiList []map[string]interface{}
			Expect(decodeJSON(resp, &apiList)).To(Succeed())
			Expect(len(apiList)).To(BeNumerically(">", 0))

			// GET by ID
			resp, err = client.Get(baseURL + "/" + apiID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var api map[string]interface{}
			Expect(decodeJSON(resp, &api)).To(Succeed())
			Expect(api["id"]).To(Equal(apiID))
			Expect(api["name"]).To(Equal("test-api"))

			// UPDATE
			updatedSpec := map[string]interface{}{
				"openapi": "3.0.0",
				"info": map[string]interface{}{
					"title":   "Updated Test API",
					"version": "1.0.0",
				},
				"paths": map[string]interface{}{
					"/updated": map[string]interface{}{
						"get": map[string]interface{}{
							"responses": map[string]interface{}{
								"200": map[string]interface{}{
									"description": "Success",
								},
							},
						},
					},
				},
			}

			updatedConfig := map[string]interface{}{
				"name":    "test-api-updated",
				"version": "1.0.0",
				"spec":    updatedSpec,
			}
			resp, err = putJSON(client, baseURL+"/"+apiID, updatedConfig)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			// Verify update
			resp, err = client.Get(baseURL + "/" + apiID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			var updatedAPI map[string]interface{}
			Expect(decodeJSON(resp, &updatedAPI)).To(Succeed())
			Expect(updatedAPI["name"]).To(Equal("test-api-updated"))

			// DELETE
			resp, err = deleteRequest(client, baseURL+"/"+apiID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNoContent))

			// Verify deletion
			resp, err = client.Get(baseURL + "/" + apiID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))
		})

		It("should return 404 for non-existent API operations", func() {
			baseURL := env.CPBaseURL + "/apify/admin/apis"
			fakeID := "non-existent-id"

			// GET non-existent
			resp, err := client.Get(baseURL + "/" + fakeID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))

			// UPDATE non-existent
			apiConfig := map[string]interface{}{
				"name":    "test",
				"version": "1.0.0",
				"spec": map[string]interface{}{
					"openapi": "3.0.0",
					"info": map[string]interface{}{
						"title":   "Test",
						"version": "1.0.0",
					},
					"paths": map[string]interface{}{},
				},
			}
			resp, err = putJSON(client, baseURL+"/"+fakeID, apiConfig)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))

			// DELETE non-existent
			resp, err = deleteRequest(client, baseURL+"/"+fakeID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))
		})

		It("should support API creation from file path", func() {
			baseURL := env.CPBaseURL + "/apify/admin/apis"

			// Create a spec file
			specContent := `openapi: "3.0.0"
info:
  title: "File-based API"
  version: "1.0.0"
paths:
  /from-file:
    get:
      responses:
        "200":
          description: "Success"
`
			specFile := filepath.Join(env.TmpDir, "test-spec.yaml")
			err := os.WriteFile(specFile, []byte(specContent), 0644)
			Expect(err).NotTo(HaveOccurred())

			// CREATE API from file
			apiConfig := map[string]interface{}{
				"name":    "file-based-api",
				"version": "1.0.0",
				"path":    specFile,
			}
			body, _ := json.Marshal(apiConfig)
			resp, err := client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusCreated))

			var createResult map[string]interface{}
			Expect(decodeJSON(resp, &createResult)).To(Succeed())
			apiID := createResult["id"].(string)

			// Verify API was created
			resp, err = client.Get(baseURL + "/" + apiID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))
		})

		It("should support listeners and datasource configuration", func() {
			baseURL := env.CPBaseURL + "/apify/admin/apis"

			// CREATE API with datasource and listeners
			apiSpec := map[string]interface{}{
				"openapi": "3.0.0",
				"info": map[string]interface{}{
					"title":   "API with Config",
					"version": "1.0.0",
				},
				"paths": map[string]interface{}{},
			}

			apiConfig := map[string]interface{}{
				"name":            "configured-api",
				"version":         "1.0.0",
				"spec":            apiSpec,
				"datasource_name": "default",
				"listeners":       []string{"main"},
			}
			body, _ := json.Marshal(apiConfig)
			resp, err := client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusCreated))

			var createResult map[string]interface{}
			Expect(decodeJSON(resp, &createResult)).To(Succeed())
			apiID := createResult["id"].(string)

			// Verify configuration
			resp, err = client.Get(baseURL + "/" + apiID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			var api map[string]interface{}
			Expect(decodeJSON(resp, &api)).To(Succeed())
			Expect(api["datasource_name"]).To(Equal("default"))
			Expect(api["listeners"]).NotTo(BeNil())
		})

		It("should handle missing required fields", func() {
			baseURL := env.CPBaseURL + "/apify/admin/apis"

			// Missing name
			apiConfig := map[string]interface{}{
				"version": "1.0.0",
				"spec": map[string]interface{}{
					"openapi": "3.0.0",
					"info":    map[string]interface{}{},
					"paths":   map[string]interface{}{},
				},
			}
			body, _ := json.Marshal(apiConfig)
			resp, err := client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Or(Equal(http.StatusBadRequest), Equal(http.StatusInternalServerError)))

			// Missing version
			apiConfig = map[string]interface{}{
				"name": "test",
				"spec": map[string]interface{}{
					"openapi": "3.0.0",
					"info":    map[string]interface{}{},
					"paths":   map[string]interface{}{},
				},
			}
			body, _ = json.Marshal(apiConfig)
			resp, err = client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Or(Equal(http.StatusBadRequest), Equal(http.StatusInternalServerError)))

			// Missing spec and path
			apiConfig = map[string]interface{}{
				"name":    "test",
				"version": "1.0.0",
			}
			body, _ = json.Marshal(apiConfig)
			resp, err = client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Or(Equal(http.StatusBadRequest), Equal(http.StatusInternalServerError)))
		})
	})
})
