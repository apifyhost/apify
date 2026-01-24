package e2e_test

import (
	"bytes"
	"encoding/json"
	"net/http"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Listeners CRUD Operations", func() {
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

	Describe("POST /apify/admin/listeners", func() {
		It("should create a new listener", func() {
			listenerConfig := map[string]interface{}{
				"name":     "test-listener-create",
				"port":     8080,
				"ip":       "0.0.0.0",
				"protocol": "HTTP",
			}
			body, _ := json.Marshal(listenerConfig)
			resp, err := client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusCreated))

			var result map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&result)
			Expect(err).NotTo(HaveOccurred())
			Expect(result["id"]).NotTo(BeEmpty())
		})
	})

	Describe("GET /apify/admin/listeners", func() {
		It("should list all listeners", func() {
			// Create a listener first
			listenerConfig := map[string]interface{}{
				"name":     "test-listener-list",
				"port":     8081,
				"ip":       "0.0.0.0",
				"protocol": "HTTP",
			}
			body, _ := json.Marshal(listenerConfig)
			resp, err := client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())
			resp.Body.Close()

			// List listeners
			resp, err = client.Get(env.CPBaseURL + "/apify/admin/listeners")
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var results []map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&results)
			Expect(err).NotTo(HaveOccurred())
			Expect(len(results)).To(BeNumerically(">", 0))
		})
	})

	Describe("GET /apify/admin/listeners/{id}", func() {
		It("should get a listener by ID", func() {
			// Create a listener first
			listenerConfig := map[string]interface{}{
				"name":     "test-listener-get",
				"port":     8082,
				"ip":       "0.0.0.0",
				"protocol": "HTTP",
			}
			body, _ := json.Marshal(listenerConfig)
			resp, err := client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())

			var created map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&created)
			resp.Body.Close()
			Expect(err).NotTo(HaveOccurred())
			id := created["id"].(string)

			// Get listener by ID
			resp, err = client.Get(env.CPBaseURL + "/apify/admin/listeners/" + id)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var result map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&result)
			Expect(err).NotTo(HaveOccurred())
			Expect(result["id"]).To(Equal(id))
		})

		It("should return 404 for non-existent listener", func() {
			resp, err := client.Get(env.CPBaseURL + "/apify/admin/listeners/non-existent-id")
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))
		})
	})

	Describe("PUT /apify/admin/listeners/{id}", func() {
		It("should update a listener", func() {
			// Create a listener first
			listenerConfig := map[string]interface{}{
				"name":     "test-listener-update",
				"port":     8083,
				"ip":       "0.0.0.0",
				"protocol": "HTTP",
			}
			body, _ := json.Marshal(listenerConfig)
			resp, err := client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())

			var created map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&created)
			resp.Body.Close()
			Expect(err).NotTo(HaveOccurred())
			id := created["id"].(string)

			// Update listener
			updatedConfig := map[string]interface{}{
				"name":     "test-listener-updated",
				"port":     8084,
				"ip":       "0.0.0.0",
				"protocol": "HTTP",
			}
			updateBody, _ := json.Marshal(updatedConfig)
			resp, err = putJSON(client, env.CPBaseURL+"/apify/admin/listeners/"+id, updateBody)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			// Verify update
			resp, err = client.Get(env.CPBaseURL + "/apify/admin/listeners/" + id)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			var updated map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&updated)
			Expect(err).NotTo(HaveOccurred())
			Expect(updated["name"]).To(Equal("test-listener-updated"))
		})
	})

	Describe("DELETE /apify/admin/listeners/{id}", func() {
		It("should delete a listener", func() {
			// Create a listener first
			listenerConfig := map[string]interface{}{
				"name":     "test-listener-delete",
				"port":     8085,
				"ip":       "0.0.0.0",
				"protocol": "HTTP",
			}
			body, _ := json.Marshal(listenerConfig)
			resp, err := client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
			Expect(err).NotTo(HaveOccurred())

			var created map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&created)
			resp.Body.Close()
			Expect(err).NotTo(HaveOccurred())
			id := created["id"].(string)

			// Delete listener
			resp, err = deleteRequest(client, env.CPBaseURL+"/apify/admin/listeners/"+id)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			// Verify deletion
			resp, err = client.Get(env.CPBaseURL + "/apify/admin/listeners/" + id)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()
			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))
		})
	})
})
