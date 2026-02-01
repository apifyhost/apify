package e2e_test

import (
	"bytes"
	"encoding/json"
	"net/http"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Control Plane API Operations", func() {
	var (
		env    *TestEnv
		client *http.Client
	)

	BeforeEach(func() {
		// Start with empty spec to get a clean environment (although StartTestEnv creates default resources)
		env = StartTestEnv(map[string]string{})
		client = &http.Client{
			Timeout: 10 * time.Second,
		}
	})

	AfterEach(func() {
		if env != nil {
			env.Stop()
		}
	})

	It("should create and retrieve a datasource", func() {
		datasourceConfig := map[string]interface{}{
			"name": "test-db",
			"config": map[string]interface{}{
				"driver":   "sqlite",
				"database": "/tmp/test.db",
			},
		}
		body, _ := json.Marshal(datasourceConfig)
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/datasources", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))

		resp, err = client.Get(env.CPBaseURL + "/apify/admin/datasources")
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusOK))

		var datasources []map[string]interface{}
		err = json.NewDecoder(resp.Body).Decode(&datasources)
		Expect(err).NotTo(HaveOccurred())

		found := false
		for _, ds := range datasources {
			if ds["name"] == "test-db" {
				found = true
				break
			}
		}
		Expect(found).To(BeTrue(), "Created datasource not found")
	})

	It("should create and retrieve an auth config", func() {
		authConfig := map[string]interface{}{
			"type":    "api-key",
			"name":    "test-auth",
			"enabled": true,
			"config": map[string]interface{}{
				"source":    "header",
				"key_name":  "X-Test-Key",
				"consumers": []interface{}{},
			},
		}
		body, _ := json.Marshal(authConfig)
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/auth", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))

		resp, err = client.Get(env.CPBaseURL + "/apify/admin/auth")
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusOK))

		var auths []map[string]interface{}
		err = json.NewDecoder(resp.Body).Decode(&auths)
		Expect(err).NotTo(HaveOccurred())

		found := false
		for _, a := range auths {
			// The response structure might be different (record vs config), let's check
			// The GET returns records which contain "config" as a string or object depending on implementation
			// In auth.rs: GET returns records. AuthConfigRecord has config as String?
			// Let's check models.rs if possible, but based on auth.rs:
			// let auth_record: AuthConfigRecord = serde_json::from_value(record)?;
			// let auth_config: Authenticator = serde_json::from_str(&auth_record.config)?;
			// Wait, handle_auth_request GET returns `records` directly.
			// records are from `_meta_auth_configs`.
			// In auth.rs POST: data.insert("config".to_string(), Value::String(config_str));
			// So the GET response will have "config" as a JSON string, not object.

			// We need to parse the config string if it is a string
			if configStr, ok := a["config"].(string); ok {
				var configObj map[string]interface{}
				json.Unmarshal([]byte(configStr), &configObj)
				if configObj["name"] == "test-auth" {
					found = true
					break
				}
			}
		}
		Expect(found).To(BeTrue(), "Created auth config not found")
	})

	It("should create and retrieve a listener", func() {
		listenerConfig := map[string]interface{}{
			"name":     "test-listener",
			"port":     9090,
			"ip":       "0.0.0.0",
			"protocol": "HTTP",
		}
		body, _ := json.Marshal(listenerConfig)
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))

		resp, err = client.Get(env.CPBaseURL + "/apify/admin/listeners")
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusOK))

		var listeners []map[string]interface{}
		err = json.NewDecoder(resp.Body).Decode(&listeners)
		Expect(err).NotTo(HaveOccurred())

		found := false
		for _, l := range listeners {
			if name, ok := l["name"].(string); ok && name == "test-listener" {
				found = true
				break
			}
		}
		Expect(found).To(BeTrue(), "Created listener not found")
	})

	It("should create and retrieve an API", func() {
		// Let's use a simple spec object
		spec := map[string]interface{}{
			"openapi": "3.0.0",
			"info": map[string]interface{}{
				"title":   "Test API",
				"version": "1.0.0",
			},
			"paths": map[string]interface{}{},
		}

		apiConfig := map[string]interface{}{
			"name":            "test-api",
			"version":         "1.0.0",
			"spec":            spec,
			"datasource_name": "default",
		}

		body, _ := json.Marshal(apiConfig)
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/apis", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))

		resp, err = client.Get(env.CPBaseURL + "/apify/admin/apis")
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusOK))

		var apis []map[string]interface{}
		err = json.NewDecoder(resp.Body).Decode(&apis)
		Expect(err).NotTo(HaveOccurred())

		found := false
		for _, a := range apis {
			if a["name"] == "test-api" {
				found = true
				break
			}
		}
		Expect(found).To(BeTrue(), "Created API not found")
	})

	It("should reject duplicate datasource names", func() {
		datasourceConfig := map[string]interface{}{
			"name": "duplicate-db",
			"config": map[string]interface{}{
				"driver":   "sqlite",
				"database": "/tmp/dup.db",
			},
		}
		body, _ := json.Marshal(datasourceConfig)

		// First creation should succeed
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/datasources", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))
		resp.Body.Close()

		// Second creation with same name should fail
		body, _ = json.Marshal(datasourceConfig)
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/datasources", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusConflict))
		resp.Body.Close()
	})

	It("should reject duplicate listener IP and port", func() {
		listenerConfig := map[string]interface{}{
			"name":     "dup-listener",
			"port":     9999,
			"ip":       "127.0.0.1",
			"protocol": "http",
		}
		body, _ := json.Marshal(listenerConfig)

		// First creation should succeed
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))
		resp.Body.Close()

		// Second creation with same IP and port should fail
		body, _ = json.Marshal(listenerConfig)
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusConflict))
		resp.Body.Close()

		// But different IP with same port should succeed
		listenerConfig["ip"] = "127.0.0.2"
		listenerConfig["name"] = "dup-listener-2"
		body, _ = json.Marshal(listenerConfig)
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))
		resp.Body.Close()
	})

	It("should reject 0.0.0.0 listener when specific IP exists on same port", func() {
		// Create listener on specific IP first
		listenerConfig := map[string]interface{}{
			"name":     "specific-ip-listener",
			"port":     9998,
			"ip":       "127.0.0.1",
			"protocol": "http",
		}
		body, _ := json.Marshal(listenerConfig)
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))
		resp.Body.Close()

		// Try to create 0.0.0.0 listener on same port should fail
		listenerConfig["ip"] = "0.0.0.0"
		listenerConfig["name"] = "all-interfaces-listener"
		body, _ = json.Marshal(listenerConfig)
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/listeners", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusConflict))
		resp.Body.Close()
	})

	It("should reject duplicate auth config names", func() {
		authConfig := map[string]interface{}{
			"type":    "api-key",
			"name":    "dup-auth",
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

		// First creation should succeed
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/auth", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))
		resp.Body.Close()

		// Second creation with same name should fail
		body, _ = json.Marshal(authConfig)
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/auth", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusConflict))
		resp.Body.Close()
	})

	It("should allow updating API with same name and version", func() {
		apiConfig := map[string]interface{}{
			"name":            "update-api",
			"version":         "1.0.0",
			"datasource_name": "default",
			"spec": map[string]interface{}{
				"openapi": "3.0.0",
				"info": map[string]interface{}{
					"title":   "Update API",
					"version": "1.0.0",
				},
				"paths": map[string]interface{}{},
			},
		}
		body, _ := json.Marshal(apiConfig)

		// First creation should succeed
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/apis", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))
		resp.Body.Close()

		// Second creation with same name and version should succeed (update)
		body, _ = json.Marshal(apiConfig)
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/apis", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))
		resp.Body.Close()

		// Same name with different version should also succeed
		apiConfig["version"] = "2.0.0"
		body, _ = json.Marshal(apiConfig)
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/apis", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))
		resp.Body.Close()
	})
})
