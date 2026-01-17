package e2e_test

import (
	"bytes"
	"fmt"
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

var _ = Describe("Multi-Listener API Support", func() {
	var (
		cpCmd      *exec.Cmd
		dpCmd      *exec.Cmd
		tmpDir     string
		cpBaseURL  string
		dp1BaseURL string
		dp2BaseURL string
		client     *http.Client
	)

	BeforeEach(func() {
		var err error
		wd, _ := os.Getwd()
		projectRoot := filepath.Dir(wd)

		// Allocate ports
		getFreePort := func() int {
			l, err := net.Listen("tcp", "127.0.0.1:0")
			Expect(err).NotTo(HaveOccurred())
			defer l.Close()
			return l.Addr().(*net.TCPAddr).Port
		}

		cpPort := getFreePort()
		dp1Port := getFreePort()
		dp2Port := getFreePort()
		metricsPort := getFreePort()

		cpBaseURL = fmt.Sprintf("http://127.0.0.1:%d", cpPort)
		dp1BaseURL = fmt.Sprintf("http://127.0.0.1:%d", dp1Port)
		dp2BaseURL = fmt.Sprintf("http://127.0.0.1:%d", dp2Port)

		// Create temp dir
		tmpDir, err = os.MkdirTemp("", "apify-multi-listener-test")
		Expect(err).NotTo(HaveOccurred())

		configFile := filepath.Join(tmpDir, "config.yaml")
		dbFile := filepath.Join(tmpDir, "test.sqlite")

		// Create empty DB
		f, err := os.Create(dbFile)
		Expect(err).NotTo(HaveOccurred())
		f.Close()

		// Config content
		configContent := fmt.Sprintf(`
control-plane:
  listen:
    ip: 127.0.0.1
    port: %d
  database:
    driver: sqlite
    database: //%s

log_level: "info"

modules:
  metrics:
    enabled: true
    port: %d
`, cpPort, dbFile, metricsPort)

		err = os.WriteFile(configFile, []byte(configContent), 0644)
		Expect(err).NotTo(HaveOccurred())

		// Start CP
		binPath := filepath.Join(projectRoot, "target", "debug", "apify")
		if _, err := os.Stat(binPath); err == nil {
			cpCmd = exec.Command(binPath, "--control-plane", "--config", configFile)
		} else {
			cpCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--control-plane", "--config", configFile)
		}
		cpCmd.Dir = projectRoot
		cpCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+dbFile)
		var cpStdout, cpStderr bytes.Buffer
		cpCmd.Stdout = &cpStdout
		cpCmd.Stderr = &cpStderr
		err = cpCmd.Start()
		Expect(err).NotTo(HaveOccurred())

		client = &http.Client{Timeout: 5 * time.Second}

		// Wait for CP
		Eventually(func() error {
			resp, err := client.Get(cpBaseURL + "/apify/admin/apis")
			if err != nil {
				return err
			}
			defer resp.Body.Close()
			if resp.StatusCode != 200 {
				return fmt.Errorf("status %d", resp.StatusCode)
			}
			return nil
		}, 30*time.Second, 1*time.Second).Should(Succeed(), func() string {
			return fmt.Sprintf("CP failed to start. Stdout: %s, Stderr: %s", cpStdout.String(), cpStderr.String())
		})

		// Prepare Import Config
		apiPath := filepath.Join(projectRoot, "examples/basic/config/openapi/items.yaml")
		apiKey := "test-api-key"

		importConfig := map[string]interface{}{
			"auth": []map[string]interface{}{
				{
					"name":    "default-api-keys",
					"type":    "api-key",
					"enabled": true,
					"config": map[string]interface{}{
						"source":   "header",
						"key_name": "X-API-KEY",
						"consumers": []map[string]interface{}{
							{
								"name": "default",
								"keys": []string{apiKey},
							},
						},
					},
				},
			},
			"datasource": map[string]interface{}{
				"default": map[string]interface{}{
					"driver":        "sqlite",
					"database":      "//" + dbFile,
					"max_pool_size": 1,
				},
			},
			"listeners": []map[string]interface{}{
				{
					"name":     "listener-1",
					"port":     dp1Port,
					"ip":       "127.0.0.1",
					"protocol": "HTTP",
				},
				{
					"name":     "listener-2",
					"port":     dp2Port,
					"ip":       "127.0.0.1",
					"protocol": "HTTP",
				},
			},
			"apis": []map[string]interface{}{
				{
					"path":       apiPath,
					"datasource": "default",
					"listeners":  []string{"listener-1", "listener-2"},
				},
			},
		}

		importYaml, err := yaml.Marshal(importConfig)
		Expect(err).NotTo(HaveOccurred())

		resp, err := client.Post(cpBaseURL+"/apify/admin/import", "application/x-yaml", bytes.NewBuffer(importYaml))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(200))

		// Start DP
		if _, err := os.Stat(binPath); err == nil {
			dpCmd = exec.Command(binPath, "--data-plane", "--config", configFile)
		} else {
			dpCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--data-plane", "--config", configFile)
		}
		dpCmd.Dir = projectRoot
		dpCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+dbFile, "APIFY_CONFIG_POLL_INTERVAL=1")
		var dpStdout, dpStderr bytes.Buffer
		dpCmd.Stdout = &dpStdout
		dpCmd.Stderr = &dpStderr
		err = dpCmd.Start()
		Expect(err).NotTo(HaveOccurred())

		// Wait for DP (check metrics port as it's always up, but we really want to check if listeners are up)
		// Since listeners are dynamic, we should check the actual API endpoints
		Eventually(func() error {
			// Check listener 1
			req1, _ := http.NewRequest("GET", dp1BaseURL+"/items", nil)
			req1.Header.Set("X-API-KEY", apiKey)
			resp1, err := client.Do(req1)
			if err != nil {
				return err
			}
			resp1.Body.Close()
			if resp1.StatusCode != 200 {
				return fmt.Errorf("listener 1 status %d", resp1.StatusCode)
			}

			// Check listener 2
			req2, _ := http.NewRequest("GET", dp2BaseURL+"/items", nil)
			req2.Header.Set("X-API-KEY", apiKey)
			resp2, err := client.Do(req2)
			if err != nil {
				return err
			}
			resp2.Body.Close()
			if resp2.StatusCode != 200 {
				return fmt.Errorf("listener 2 status %d", resp2.StatusCode)
			}

			return nil
		}, 30*time.Second, 1*time.Second).Should(Succeed(), func() string {
			return fmt.Sprintf("DP failed to start. Stdout: %s, Stderr: %s", dpStdout.String(), dpStderr.String())
		})
	})

	AfterEach(func() {
		if dpCmd != nil && dpCmd.Process != nil {
			dpCmd.Process.Kill()
			dpCmd.Wait()
		}
		if cpCmd != nil && cpCmd.Process != nil {
			cpCmd.Process.Kill()
			cpCmd.Wait()
		}
		if tmpDir != "" {
			os.RemoveAll(tmpDir)
		}
	})

	It("should expose the same API on multiple listeners", func() {
		apiKey := "test-api-key"

		// Verify Listener 1
		req1, _ := http.NewRequest("GET", dp1BaseURL+"/items", nil)
		req1.Header.Set("X-API-KEY", apiKey)
		resp1, err := client.Do(req1)
		Expect(err).NotTo(HaveOccurred())
		Expect(resp1.StatusCode).To(Equal(200))

		// Verify Listener 2
		req2, _ := http.NewRequest("GET", dp2BaseURL+"/items", nil)
		req2.Header.Set("X-API-KEY", apiKey)
		resp2, err := client.Do(req2)
		Expect(err).NotTo(HaveOccurred())
		Expect(resp2.StatusCode).To(Equal(200))
	})
})
