package e2e_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
	"gopkg.in/yaml.v3"
)

func TestE2E(t *testing.T) {
	RegisterFailHandler(Fail)
	RunSpecs(t, "Apify E2E Test Suite")
}

type TestEnv struct {
	ServerCmd  *exec.Cmd
	CPCmd      *exec.Cmd
	TmpDir     string
	BaseURL    string
	APIKey     string
	ConfigFile string
	DBFile     string
}

func StartTestEnv(specFiles map[string]string) *TestEnv {
	var err error
	env := &TestEnv{}

	// Get a free port
	l, err := net.Listen("tcp", "127.0.0.1:0")
	Expect(err).NotTo(HaveOccurred())
	serverPort := fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
	l.Close()
	env.BaseURL = "http://127.0.0.1:" + serverPort
	env.APIKey = "e2e-test-key-001"

	// Create temporary directory
	env.TmpDir, err = os.MkdirTemp("", "apify-e2e-test")
	Expect(err).NotTo(HaveOccurred())

	env.ConfigFile = filepath.Join(env.TmpDir, "config.yaml")
	env.DBFile = filepath.Join(env.TmpDir, "test.sqlite")

	// Create empty DB file
	f, err := os.Create(env.DBFile)
	Expect(err).NotTo(HaveOccurred())
	f.Close()

	// Prepare API names for config
	apisYaml := ""
	if len(specFiles) > 0 {
		apisYaml = "    apis:\n"
		for name := range specFiles {
			apisYaml += fmt.Sprintf("      - %s\n", name)
		}
	}

	// Write Config
	configContent := fmt.Sprintf(`
control-plane:
  listen:
    ip: 127.0.0.1
    port: %s
  database:
    driver: sqlite
    database: //%s

listeners:
  - port: %s
    ip: 127.0.0.1
    protocol: HTTP
%s

auth:
  - name: e2e-api-keys
    type: api-key
    enabled: true
    config:
      source: header
      key_name: X-Api-Key
      consumers:
        - name: default
          keys:
            - %s

datasource:
  default:
    driver: sqlite
    database: //%s
    max_pool_size: 1

log_level: "info"

modules:
  tracing:
    enabled: true
  metrics:
    enabled: false
`, serverPort, env.DBFile, serverPort, apisYaml, env.APIKey, env.DBFile)

	err = os.WriteFile(env.ConfigFile, []byte(configContent), 0644)
	Expect(err).NotTo(HaveOccurred())

	// Start Control Plane
	wd, _ := os.Getwd()
	projectRoot := filepath.Dir(wd)

	env.CPCmd = exec.Command("cargo", "run", "--", "--config", env.ConfigFile, "--control-plane")
	env.CPCmd.Dir = projectRoot
	env.CPCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+env.DBFile)
	env.CPCmd.Stdout = GinkgoWriter
	env.CPCmd.Stderr = GinkgoWriter

	err = env.CPCmd.Start()
	Expect(err).NotTo(HaveOccurred())

	client := &http.Client{Timeout: 5 * time.Second}

	// Wait for CP to be ready
	Eventually(func() error {
		resp, err := client.Get(env.BaseURL + "/_meta/apis")
		if err != nil {
			return err
		}
		defer resp.Body.Close()
		if resp.StatusCode != 200 {
			return fmt.Errorf("status code %d", resp.StatusCode)
		}
		return nil
	}, 300*time.Second, 1*time.Second).Should(Succeed())

	// Import Specs
	for name, path := range specFiles {
		// Read YAML
		yamlContent, err := os.ReadFile(filepath.Join(projectRoot, path))
		Expect(err).NotTo(HaveOccurred())

		// Parse YAML to interface{}
		var fullConfig map[string]interface{}
		err = yaml.Unmarshal(yamlContent, &fullConfig)
		Expect(err).NotTo(HaveOccurred())

		// Extract "openapi" -> "spec"
		openapi, ok := fullConfig["openapi"].(map[string]interface{})
		Expect(ok).To(BeTrue(), "Missing 'openapi' key in spec file")
		specObj, ok := openapi["spec"]
		Expect(ok).To(BeTrue(), "Missing 'spec' key in openapi object")

		// Construct payload
		payload := map[string]interface{}{
			"name":    name,
			"version": "1.0.0",
			"spec":    specObj,
		}
		payloadBytes, err := json.Marshal(payload)
		Expect(err).NotTo(HaveOccurred())

		resp, err := client.Post(env.BaseURL+"/_meta/apis", "application/json", bytes.NewBuffer(payloadBytes))
		Expect(err).NotTo(HaveOccurred())
		Expect(resp.StatusCode).To(Equal(201))
		resp.Body.Close()
	}

	// Stop Control Plane
	if env.CPCmd.Process != nil {
		env.CPCmd.Process.Kill()
		env.CPCmd.Wait()
	}

	// Start Data Plane
	env.ServerCmd = exec.Command("cargo", "run", "--", "--config", env.ConfigFile)
	env.ServerCmd.Dir = projectRoot
	env.ServerCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+env.DBFile)
	env.ServerCmd.Stdout = GinkgoWriter
	env.ServerCmd.Stderr = GinkgoWriter

	err = env.ServerCmd.Start()
	Expect(err).NotTo(HaveOccurred())

	// Wait for server to be ready
	Eventually(func() error {
		resp, err := client.Get(env.BaseURL + "/healthz")
		if err != nil {
			return err
		}
		defer resp.Body.Close()
		if resp.StatusCode != 200 {
			return fmt.Errorf("status %d", resp.StatusCode)
		}
		return nil
	}, "120s", "1s").Should(Succeed(), "Server failed to start")

	return env
}

func (e *TestEnv) Stop() {
	if e.ServerCmd != nil && e.ServerCmd.Process != nil {
		e.ServerCmd.Process.Kill()
		e.ServerCmd.Wait()
	}
	if e.CPCmd != nil && e.CPCmd.Process != nil {
		e.CPCmd.Process.Kill()
		e.CPCmd.Wait()
	}
	if e.TmpDir != "" {
		os.RemoveAll(e.TmpDir)
	}
}
