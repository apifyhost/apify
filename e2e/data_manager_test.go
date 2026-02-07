package e2e_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"os/exec"
	"path/filepath"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Data Manager API", func() {
	var (
		env        *TestEnv
		client     *http.Client
		userDBPath string
	)

	BeforeEach(func() {
		// 1. Start Environment
		env = StartTestEnv(map[string]string{})
		client = &http.Client{
			Timeout: 10 * time.Second,
		}

		// 2. Create a user sqlite database with a table
		userDBPath = filepath.Join(env.TmpDir, "user.db")
		cmd := exec.Command("sqlite3", userDBPath, "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, email TEXT, age INTEGER);")
		err := cmd.Run()
		Expect(err).NotTo(HaveOccurred(), "Failed to create user sqlite db")

		// 3. Register the datasource
		datasourceConfig := map[string]interface{}{
			"name": "user-ds",
			"config": map[string]interface{}{
				"driver":   "sqlite",
				"database": userDBPath,
			},
		}
		body, _ := json.Marshal(datasourceConfig)
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/datasources", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))
	})

	AfterEach(func() {
		if env != nil {
			env.Stop()
		}
	})

	It("should list tables in the datasource", func() {
		resp, err := client.Get(env.CPBaseURL + "/apify/admin/data/user-ds/tables")
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusOK))

		var tables []string
		err = json.NewDecoder(resp.Body).Decode(&tables)
		Expect(err).NotTo(HaveOccurred())
		Expect(tables).To(ContainElement("users"))
	})

	It("should get table schema", func() {
		resp, err := client.Get(env.CPBaseURL + "/apify/admin/data/user-ds/schema/users")
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusOK))

		var schema map[string]interface{}
		err = json.NewDecoder(resp.Body).Decode(&schema)
		Expect(err).NotTo(HaveOccurred())
		// TableSchema uses camelCase JSON serialization
		Expect(schema["tableName"]).To(Equal("users"))
		
		columns := schema["columns"].([]interface{})
		Expect(len(columns)).To(Equal(4)) // id, name, email, age
	})

	It("should perform CRUD operations", func() {
		// 1. Create (Insert)
		user1 := map[string]interface{}{
			"name":  "Alice",
			"email": "alice@example.com",
			"age":   30,
		}
		body, _ := json.Marshal(user1)
		resp, err := client.Post(env.CPBaseURL+"/apify/admin/data/user-ds/users", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusCreated))

		// 2. Query
		queryPayload := map[string]interface{}{
			"where": map[string]interface{}{
				"name": "Alice",
			},
		}
		body, _ = json.Marshal(queryPayload)
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/data/user-ds/users/query", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusOK))

		var users []map[string]interface{}
		err = json.NewDecoder(resp.Body).Decode(&users)
		Expect(err).NotTo(HaveOccurred())
		Expect(len(users)).To(Equal(1))
		Expect(users[0]["name"]).To(Equal("Alice"))
		
		// Get ID for update/delete
		// ID might be float64 due to JSON decoding
		idVal := users[0]["id"]
		var id int
		switch v := idVal.(type) {
		case float64:
			id = int(v)
		case int:
			id = v
		default:
			Fail("Unknown ID type")
		}

		// 3. Update
		updateData := map[string]interface{}{
			"age": 31,
		}
		resp, err = putJSON(client, fmt.Sprintf("%s/apify/admin/data/user-ds/users/%d", env.CPBaseURL, id), updateData)
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusOK))

		// Verify Update
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/data/user-ds/users/query", "application/json", bytes.NewBuffer(body)) // Re-query alice
		Expect(err).NotTo(HaveOccurred())
		var updatedUsers []map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&updatedUsers)
		Expect(updatedUsers[0]["age"]).To(Equal(float64(31)))

		// 4. Delete
		resp, err = deleteRequest(client, fmt.Sprintf("%s/apify/admin/data/user-ds/users/%d", env.CPBaseURL, id))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(http.StatusNoContent))

		// Verify Delete
		resp, err = client.Post(env.CPBaseURL+"/apify/admin/data/user-ds/users/query", "application/json", bytes.NewBuffer(body))
		Expect(err).NotTo(HaveOccurred())
		var finalUsers []map[string]interface{}
		json.NewDecoder(resp.Body).Decode(&finalUsers)
		Expect(len(finalUsers)).To(Equal(0))
	})
})
