package e2e_test

import (
	"bytes"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	"gopkg.in/yaml.v3"
)

var _ = Describe("API Listener Bindings", func() {
	var (
		env        *TestEnv
		client     *http.Client
		portPublic string
		portAdmin  string
		portUnused string
	)

	startMultiListenerEnv := func() {
		var err error
		env = &TestEnv{}

		wd, _ := os.Getwd()
		projectRoot := filepath.Dir(wd)

		// Allocate 3 ports
		l1, _ := net.Listen("tcp", "127.0.0.1:0")
		portPublic = fmt.Sprintf("%d", l1.Addr().(*net.TCPAddr).Port)
		l1.Close()

		l2, _ := net.Listen("tcp", "127.0.0.1:0")
		portAdmin = fmt.Sprintf("%d", l2.Addr().(*net.TCPAddr).Port)
		l2.Close()

		l3, _ := net.Listen("tcp", "127.0.0.1:0")
		portUnused = fmt.Sprintf("%d", l3.Addr().(*net.TCPAddr).Port)
		l3.Close()

		// CP Port
		lCP, _ := net.Listen("tcp", "127.0.0.1:0")
		cpPort := fmt.Sprintf("%d", lCP.Addr().(*net.TCPAddr).Port)
		lCP.Close()
		env.CPBaseURL = "http://127.0.0.1:" + cpPort

		// Metrics Port
		lM, _ := net.Listen("tcp", "127.0.0.1:0")
		env.MetricsPort = fmt.Sprintf("%d", lM.Addr().(*net.TCPAddr).Port)
		lM.Close()

		env.TmpDir, err = os.MkdirTemp("", "apify-e2e-listeners")
		Expect(err).NotTo(HaveOccurred())

		env.ConfigFile = filepath.Join(env.TmpDir, "config.yaml")
		env.DBFile = filepath.Join(env.TmpDir, "test.sqlite")

		// Create empty DB
		f, err := os.Create(env.DBFile)
		Expect(err).NotTo(HaveOccurred())
		f.Close()

		// Create API Specs
		createSpec := func(name, tableName string) string {
			content := fmt.Sprintf(`openapi: "3.0.0"
info:
  title: "%s"
  version: "1.0.0"
x-table-schemas:
  - table_name: "%s"
    columns:
      - { name: "id", column_type: "INTEGER", nullable: false, primary_key: true, unique: false, auto_increment: true, default_value: null }
    indexes: []
paths:
  /%s:
    get:
      x-table-name: "%s"
      responses:
        "200":
          description: "ok"
`, name, tableName, name, tableName)
			path := filepath.Join(env.TmpDir, name+".yaml")
			err := os.WriteFile(path, []byte(content), 0644)
			Expect(err).NotTo(HaveOccurred())
			return path
		}

		pathPublic := createSpec("public", "public_items")
		pathAdmin := createSpec("admin", "admin_items")
		pathShared := createSpec("shared", "shared_items")
		pathOrphan := createSpec("orphan", "orphan_items")

		// Config for CP
		configContent := fmt.Sprintf(`
control-plane:
  listen:
    ip: 127.0.0.1
    port: %s
  database:
    driver: sqlite
    database: //%s

log_level: "info"

modules:
  tracing:
    enabled: true
  metrics:
    enabled: true
    port: %s
`, cpPort, env.DBFile, env.MetricsPort)

		err = os.WriteFile(env.ConfigFile, []byte(configContent), 0644)
		Expect(err).NotTo(HaveOccurred())

		// Start CP
		binPath := filepath.Join(projectRoot, "target", "debug", "apify")
		if _, err := os.Stat(binPath); err == nil {
			env.CPCmd = exec.Command(binPath, "--control-plane", "--config", env.ConfigFile)
		} else {
			env.CPCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--control-plane", "--config", env.ConfigFile)
		}
		env.CPCmd.Dir = projectRoot
		env.CPCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+env.DBFile)

		// Capture CP output for debugging
		var cpStdout, cpStderr bytes.Buffer
		env.CPCmd.Stdout = io.MultiWriter(&cpStdout, GinkgoWriter)
		env.CPCmd.Stderr = io.MultiWriter(&cpStderr, GinkgoWriter)

		err = env.CPCmd.Start()
		Expect(err).NotTo(HaveOccurred())

		client = &http.Client{Timeout: 5 * time.Second}

		// Wait for CP
		Eventually(func() error {
			resp, err := client.Get(env.CPBaseURL + "/_meta/apis")
			if err != nil {
				return err
			}
			defer resp.Body.Close()
			if resp.StatusCode != 200 {
				return fmt.Errorf("status code %d", resp.StatusCode)
			}
			return nil
		}, 60*time.Second, 1*time.Second).Should(Succeed(), func() string {
			return fmt.Sprintf("Control Plane failed to start.\nStdout: %s\nStderr: %s", cpStdout.String(), cpStderr.String())
		})

		// Import Config
		importConfig := map[string]interface{}{
			"datasource": map[string]interface{}{
				"default": map[string]interface{}{
					"driver":        "sqlite",
					"database":      "//" + env.DBFile,
					"max_pool_size": 1,
				},
			},
			"listeners": []map[string]interface{}{
				{"name": "public", "port": mustParseInt(portPublic), "ip": "127.0.0.1", "protocol": "HTTP"},
				{"name": "admin", "port": mustParseInt(portAdmin), "ip": "127.0.0.1", "protocol": "HTTP"},
				{"name": "unused", "port": mustParseInt(portUnused), "ip": "127.0.0.1", "protocol": "HTTP"},
			},
			"apis": []map[string]interface{}{
				{"path": pathPublic, "listeners": []string{"public"}},
				{"path": pathAdmin, "listeners": []string{"admin"}},
				{"path": pathShared, "listeners": []string{"public", "admin"}},
				{"path": pathOrphan, "listeners": []string{"non_existent"}},
			},
		}

		importYaml, err := yaml.Marshal(importConfig)
		Expect(err).NotTo(HaveOccurred())

		resp, err := client.Post(env.CPBaseURL+"/_meta/import", "application/x-yaml", bytes.NewBuffer(importYaml))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200))
		resp.Body.Close()

		// Start DP
		if _, err := os.Stat(binPath); err == nil {
			env.ServerCmd = exec.Command(binPath, "--data-plane", "--config", env.ConfigFile)
		} else {
			env.ServerCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--data-plane", "--config", env.ConfigFile)
		}
		env.ServerCmd.Dir = projectRoot
		env.ServerCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+env.DBFile, "APIFY_CONFIG_POLL_INTERVAL=1")

		var dpStdout, dpStderr bytes.Buffer
		env.ServerCmd.Stdout = io.MultiWriter(&dpStdout, GinkgoWriter)
		env.ServerCmd.Stderr = io.MultiWriter(&dpStderr, GinkgoWriter)

		err = env.ServerCmd.Start()
		Expect(err).NotTo(HaveOccurred())

		// Wait for DP (check public listener)
		Eventually(func() error {
			resp, err := client.Get(fmt.Sprintf("http://127.0.0.1:%s/healthz", portPublic))
			if err != nil {
				return err
			}
			defer resp.Body.Close()
			if resp.StatusCode != 200 {
				return fmt.Errorf("status %d", resp.StatusCode)
			}
			return nil
		}, "60s", "1s").Should(Succeed(), func() string {
			return fmt.Sprintf("Data Plane failed to start.\nStdout: %s\nStderr: %s", dpStdout.String(), dpStderr.String())
		})
	}

	stopEnv := func() {
		if env != nil {
			env.Stop()
		}
	}

	BeforeEach(startMultiListenerEnv)
	AfterEach(stopEnv)

	It("should restrict API access based on listener bindings", func() {
		basePublic := fmt.Sprintf("http://127.0.0.1:%s", portPublic)
		baseAdmin := fmt.Sprintf("http://127.0.0.1:%s", portAdmin)
		baseUnused := fmt.Sprintf("http://127.0.0.1:%s", portUnused)

		// 1. Public Listener
		// Should have /public
		resp, err := client.Get(basePublic + "/public")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200), "Public listener should serve /public")
		resp.Body.Close()

		// Should have /shared
		resp, err = client.Get(basePublic + "/shared")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200), "Public listener should serve /shared")
		resp.Body.Close()

		// Should NOT have /admin
		resp, err = client.Get(basePublic + "/admin")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(404), "Public listener should NOT serve /admin")
		resp.Body.Close()

		// 2. Admin Listener
		// Should have /admin
		resp, err = client.Get(baseAdmin + "/admin")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200), "Admin listener should serve /admin")
		resp.Body.Close()

		// Should have /shared
		resp, err = client.Get(baseAdmin + "/shared")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200), "Admin listener should serve /shared")
		resp.Body.Close()

		// Should NOT have /public
		resp, err = client.Get(baseAdmin + "/public")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(404), "Admin listener should NOT serve /public")
		resp.Body.Close()

		// 3. Unused Listener
		// Should be up (healthz) but have no APIs
		resp, err = client.Get(baseUnused + "/healthz")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200))
		resp.Body.Close()

		resp, err = client.Get(baseUnused + "/public")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(404))
		resp.Body.Close()

		resp, err = client.Get(baseUnused + "/admin")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(404))
		resp.Body.Close()

		// 4. Orphan API
		// Should not be reachable anywhere
		resp, err = client.Get(basePublic + "/orphan")
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(404))
		resp.Body.Close()
	})
})

func mustParseInt(s string) int {
	var i int
	fmt.Sscanf(s, "%d", &i)
	return i
}
