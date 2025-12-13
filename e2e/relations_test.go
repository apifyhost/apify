package e2e_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Apify Relations", func() {
	var (
		env     *TestEnv
		baseURL string
		apiKey  string
		client  *http.Client
	)

	startEnv := func() {
		env = StartTestEnv(map[string]string{
			"orders": "examples/relations/config/openapi/orders.yaml",
			"users":  "examples/relations/config/openapi/users.yaml",
		})
		baseURL = env.BaseURL
		apiKey = env.APIKey
		client = &http.Client{
			Timeout: 10 * time.Second,
		}
	}

	stopEnv := func() {
		if env != nil {
			env.Stop()
		}
	}

	Describe("hasMany Relations (Orders with Items)", Ordered, func() {
		BeforeAll(startEnv)
		AfterAll(stopEnv)

		var orderID int64

		It("should create an order with nested items", func() {
			payload := map[string]interface{}{
				"customer_name": "John Doe",
				"total":         299.97,
				"status":        "pending",
				"items": []map[string]interface{}{
					{
						"product_name": "Product A",
						"quantity":     2,
						"price":        49.99,
					},
					{
						"product_name": "Product B",
						"quantity":     1,
						"price":        199.99,
					},
				},
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("POST", baseURL+"/orders", bytes.NewBuffer(jsonData))
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
			Expect(result["id"]).NotTo(BeNil())
			
			id, ok := result["id"].(float64)
			Expect(ok).To(BeTrue())
			orderID = int64(id)
		})

		It("should GET order with auto-loaded items", func() {
			req, err := http.NewRequest("GET", fmt.Sprintf("%s/orders/%d", baseURL, orderID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var order map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&order)
			Expect(err).NotTo(HaveOccurred())

			Expect(order["id"]).To(BeNumerically("==", orderID))
			Expect(order["customer_name"]).To(Equal("John Doe"))

			// Verify nested items are auto-loaded
			items, ok := order["items"].([]interface{})
			Expect(ok).To(BeTrue(), "items should be an array")
			Expect(items).To(HaveLen(2), "should have 2 items")

			item1 := items[0].(map[string]interface{})
			Expect(item1["product_name"]).To(Equal("Product A"))
			Expect(item1["quantity"]).To(BeNumerically("==", 2))
			Expect(item1["order_id"]).To(BeNumerically("==", orderID))

			item2 := items[1].(map[string]interface{})
			Expect(item2["product_name"]).To(Equal("Product B"))
			Expect(item2["quantity"]).To(BeNumerically("==", 1))
		})

		It("should LIST orders with auto-loaded items", func() {
			req, err := http.NewRequest("GET", baseURL+"/orders", nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var orders []map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&orders)
			Expect(err).NotTo(HaveOccurred())
			Expect(orders).NotTo(BeEmpty())

			// Find our order
			var testOrder map[string]interface{}
			for _, order := range orders {
				if order["customer_name"] == "John Doe" {
					testOrder = order
					break
				}
			}
			Expect(testOrder).NotTo(BeNil())

			// Verify nested items are auto-loaded in list
			items, ok := testOrder["items"].([]interface{})
			Expect(ok).To(BeTrue())
			Expect(items).To(HaveLen(2))
		})

		It("should UPDATE order with replaced items", func() {
			// Update order with completely new items
			payload := map[string]interface{}{
				"customer_name": "John Doe",
				"total":         399.99,
				"status":        "confirmed",
				"items": []map[string]interface{}{
					{
						"product_name": "Product C",
						"quantity":     3,
						"price":        99.99,
					},
					{
						"product_name": "Product D",
						"quantity":     1,
						"price":        100.02,
					},
				},
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("PUT", fmt.Sprintf("%s/orders/%d", baseURL, orderID), bytes.NewBuffer(jsonData))
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)
			req.Header.Set("Content-Type", "application/json")

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))
		})

		It("should verify items were replaced", func() {
			req, err := http.NewRequest("GET", fmt.Sprintf("%s/orders/%d", baseURL, orderID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var order map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&order)
			Expect(err).NotTo(HaveOccurred())

			Expect(order["status"]).To(Equal("confirmed"))

			items, ok := order["items"].([]interface{})
			Expect(ok).To(BeTrue())
			Expect(items).To(HaveLen(2), "should have 2 new items")

			item1 := items[0].(map[string]interface{})
			Expect(item1["product_name"]).To(Equal("Product C"))
			Expect(item1["quantity"]).To(BeNumerically("==", 3))
		})

		It("should CASCADE DELETE items when order is deleted", func() {
			// Delete the order
			req, err := http.NewRequest("DELETE", fmt.Sprintf("%s/orders/%d", baseURL, orderID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			// Verify order is gone
			req, err = http.NewRequest("GET", fmt.Sprintf("%s/orders/%d", baseURL, orderID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err = client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))

			// Verify items are also deleted (list should not contain items with this order_id)
			req, err = http.NewRequest("GET", baseURL+"/order_items", nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err = client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			var items []map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&items)
			Expect(err).NotTo(HaveOccurred())

			// Check that none of the items have our orderID
			for _, item := range items {
				if oid, ok := item["order_id"].(float64); ok {
					Expect(int64(oid)).NotTo(Equal(orderID), "Items should be cascade deleted")
				}
			}
		})
	})

	Describe("hasOne and belongsTo Relations (Users with Profiles)", Ordered, func() {
		BeforeAll(startEnv)
		AfterAll(stopEnv)

		var userID int64
		var profileID int64

		It("should create a user with nested profile", func() {
			payload := map[string]interface{}{
				"username": "testuser123",
				"email":    "test@example.com",
				"profile": map[string]interface{}{
					"full_name":  "Test User",
					"bio":        "Software engineer",
					"avatar_url": "https://example.com/avatar.jpg",
				},
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("POST", baseURL+"/users", bytes.NewBuffer(jsonData))
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
			Expect(result["id"]).NotTo(BeNil())

			id, ok := result["id"].(float64)
			Expect(ok).To(BeTrue())
			userID = int64(id)
		})

		It("should GET user with auto-loaded profile (hasOne)", func() {
			req, err := http.NewRequest("GET", fmt.Sprintf("%s/users/%d", baseURL, userID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var user map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&user)
			Expect(err).NotTo(HaveOccurred())

			Expect(user["id"]).To(BeNumerically("==", userID))
			Expect(user["username"]).To(Equal("testuser123"))

			// Verify nested profile is auto-loaded
			profile, ok := user["profile"].(map[string]interface{})
			Expect(ok).To(BeTrue(), "profile should be an object")
			Expect(profile["bio"]).To(Equal("Software engineer"))
			Expect(profile["user_id"]).To(BeNumerically("==", userID))

			// Save profile ID for belongsTo test
			pid, ok := profile["id"].(float64)
			Expect(ok).To(BeTrue())
			profileID = int64(pid)
		})

		It("should GET profile with auto-loaded user (belongsTo)", func() {
			req, err := http.NewRequest("GET", fmt.Sprintf("%s/user_profiles/%d", baseURL, profileID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var profile map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&profile)
			Expect(err).NotTo(HaveOccurred())

			Expect(profile["id"]).To(BeNumerically("==", profileID))
			Expect(profile["bio"]).To(Equal("Software engineer"))

			// Verify parent user is auto-loaded (belongsTo)
			user, ok := profile["user"].(map[string]interface{})
			Expect(ok).To(BeTrue(), "user should be an object")
			Expect(user["id"]).To(BeNumerically("==", userID))
			Expect(user["username"]).To(Equal("testuser123"))
			Expect(user["email"]).To(Equal("test@example.com"))
		})

		It("should LIST users with auto-loaded profiles", func() {
			req, err := http.NewRequest("GET", baseURL+"/users", nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var users []map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&users)
			Expect(err).NotTo(HaveOccurred())
			Expect(users).NotTo(BeEmpty())

			// Find our user
			var testUser map[string]interface{}
			for _, user := range users {
				if user["username"] == "testuser123" {
					testUser = user
					break
				}
			}
			Expect(testUser).NotTo(BeNil())

			// Verify nested profile is auto-loaded in list
			profile, ok := testUser["profile"].(map[string]interface{})
			Expect(ok).To(BeTrue())
			Expect(profile["bio"]).To(Equal("Software engineer"))
		})

		It("should UPDATE user with replaced profile", func() {
			payload := map[string]interface{}{
				"username": "testuser123",
				"email":    "newemail@example.com",
				"profile": map[string]interface{}{
					"full_name":  "Updated User",
					"bio":        "Senior software engineer",
					"avatar_url": "https://example.com/new-avatar.jpg",
				},
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("PUT", fmt.Sprintf("%s/users/%d", baseURL, userID), bytes.NewBuffer(jsonData))
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)
			req.Header.Set("Content-Type", "application/json")

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))
		})

		It("should verify profile was replaced", func() {
			req, err := http.NewRequest("GET", fmt.Sprintf("%s/users/%d", baseURL, userID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var user map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&user)
			Expect(err).NotTo(HaveOccurred())

			Expect(user["email"]).To(Equal("newemail@example.com"))
			
			profile, ok := user["profile"].(map[string]interface{})
			Expect(ok).To(BeTrue())
			Expect(profile["full_name"]).To(Equal("Updated User"))
			Expect(profile["bio"]).To(Equal("Senior software engineer"))

			// Old profile should be deleted, new one created
			newProfileID, ok := profile["id"].(float64)
			Expect(ok).To(BeTrue())
			Expect(int64(newProfileID)).NotTo(Equal(profileID), "Should be a new profile")
		})

		It("should CASCADE DELETE profile when user is deleted", func() {
			req, err := http.NewRequest("DELETE", fmt.Sprintf("%s/users/%d", baseURL, userID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			// Verify user is gone
			req, err = http.NewRequest("GET", fmt.Sprintf("%s/users/%d", baseURL, userID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err = client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusNotFound))

			// Verify profile is also deleted
			req, err = http.NewRequest("GET", baseURL+"/user_profiles", nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err = client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			var profiles []map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&profiles)
			Expect(err).NotTo(HaveOccurred())

			// Check that none of the profiles have our userID
			for _, profile := range profiles {
				if uid, ok := profile["user_id"].(float64); ok {
					Expect(int64(uid)).NotTo(Equal(userID), "Profile should be cascade deleted")
				}
			}
		})
	})

	Describe("Audit Fields in Relations", func() {
		BeforeEach(startEnv)
		AfterEach(stopEnv)

		It("should propagate audit fields to nested records", func() {
			payload := map[string]interface{}{
				"customer_name": "Audit Test Customer",
				"total":         100.00,
				"status":        "pending",
				"items": []map[string]interface{}{
					{
						"product_name": "Audit Product",
						"quantity":     1,
						"price":        100.00,
					},
				},
			}

			jsonData, err := json.Marshal(payload)
			Expect(err).NotTo(HaveOccurred())

			req, err := http.NewRequest("POST", baseURL+"/orders", bytes.NewBuffer(jsonData))
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

			orderID := int64(result["id"].(float64))

			// Get the order with items
			req, err = http.NewRequest("GET", fmt.Sprintf("%s/orders/%d", baseURL, orderID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)

			resp, err = client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			var order map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&order)
			Expect(err).NotTo(HaveOccurred())

			// Verify audit fields exist on parent
			Expect(order["createdAt"]).NotTo(BeNil())
			Expect(order["updatedAt"]).NotTo(BeNil())

			// Verify audit fields exist on nested items
			items := order["items"].([]interface{})
			item := items[0].(map[string]interface{})
			Expect(item["createdAt"]).NotTo(BeNil())
			Expect(item["updatedAt"]).NotTo(BeNil())

			// Clean up
			req, err = http.NewRequest("DELETE", fmt.Sprintf("%s/orders/%d", baseURL, orderID), nil)
			Expect(err).NotTo(HaveOccurred())
			req.Header.Set("X-Api-Key", apiKey)
			client.Do(req)
		})
	})
})
