package e2e_test

import (
	"bytes"
	"encoding/json"
	"net/http"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Datasources CRUD Operations", func() {
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
			baseURL := env.CPBaseURL + "/apify/admin/datasources"

			// CREATE
			dsConfig := map[string]interface{}{
				"name": "test-ds",
				"config": map[string]interface{}{
					"driver":   "sqlite",
					"database": "//test.db",
				},
			}
			body, _ := json.Marshal(dsConfig)
			resp, err := client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusCreated))

			var createResult map[string]interface{}
			Expect(decodeJSON(resp, &createResult)).To(Succeed())
			dsID := createResult["id"].(string)
			Expect(dsID).NotTo(BeEmpty())

			// LIST
			resp, err = client.Get(baseURL)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var dsList []map[string]interface{}
			Expect(decodeJSON(resp, &dsList)).To(Succeed())
			Expect(len(dsList)).To(BeNumerically(">", 0))

			// GET by ID
			resp, err = client.Get(baseURL + "/" + dsID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var ds map[string]interface{}
			Expect(decodeJSON(resp, &ds)).To(Succeed())
			Expect(ds["id"]).To(Equal(dsID))
			Expect(ds["name"]).To(Equal("test-ds"))

			// UPDATE
			updatedConfig := map[string]interface{}{
				"name": "test-ds-updated",
				"config": map[string]interface{}{
					"driver":   "sqlite",
					"database": "//test-updated.db",
				},
			}
			resp, err = putJSON(client, baseURL+"/"+dsID, updatedConfig)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			// Verify update
			resp, err = client.Get(baseURL + "/" + dsID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			var updatedDS map[string]interface{}
			Expect(decodeJSON(resp, &updatedDS)).To(Succeed())
			Expect(updatedDS["name"]).To(Equal("test-ds-updated"))

			// DELETE
			resp, err = deleteRequest(client, baseURL+"/"+dsID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNoContent))

			// Verify deletion
			resp, err = client.Get(baseURL + "/" + dsID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))
		})

		It("should return 404 for non-existent datasource operations", func() {
			baseURL := env.CPBaseURL + "/apify/admin/datasources"
			fakeID := "non-existent-id"

			// GET non-existent
			resp, err := client.Get(baseURL + "/" + fakeID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))

			// UPDATE non-existent
			dsConfig := map[string]interface{}{
				"name": "test",
				"config": map[string]interface{}{
					"driver":   "sqlite",
					"database": "//test.db",
				},
			}
			resp, err = putJSON(client, baseURL+"/"+fakeID, dsConfig)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))

			// DELETE non-existent
			resp, err = deleteRequest(client, baseURL+"/"+fakeID)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))
		})

		It("should reject duplicate datasource names", func() {
			baseURL := env.CPBaseURL + "/apify/admin/datasources"

			dsConfig := map[string]interface{}{
				"name": "duplicate-ds",
				"config": map[string]interface{}{
					"driver":   "sqlite",
					"database": "//dup.db",
				},
			}

			// First creation
			body, _ := json.Marshal(dsConfig)
			resp, err := client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusCreated))

			// Second creation with same name
			body, _ = json.Marshal(dsConfig)
			resp, err = client.Post(baseURL, "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusConflict))
		})
	})
})
