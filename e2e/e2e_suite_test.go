package e2e_test

import (
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
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
	MetricsPort string
}

func StartTestEnv(specFiles map[string]string) *TestEnv {
	var err error
	env := &TestEnv{}

	wd, _ := os.Getwd()
	projectRoot := filepath.Dir(wd)

	// Get a free port
	l, err := net.Listen("tcp", "127.0.0.1:0")
	Expect(err).NotTo(HaveOccurred())
	serverPort := fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
	l.Close()
	env.BaseURL = "http://127.0.0.1:" + serverPort
	env.APIKey = "e2e-test-key-001"

	// Get a free port for metrics
	l, err = net.Listen("tcp", "127.0.0.1:0")
	Expect(err).NotTo(HaveOccurred())
	env.MetricsPort = fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
	l.Close()

	// Create temporary directory
	env.TmpDir, err = os.MkdirTemp("", "apify-e2e-test")
	Expect(err).NotTo(HaveOccurred())

	env.ConfigFile = filepath.Join(env.TmpDir, "config.yaml")
	env.DBFile = filepath.Join(env.TmpDir, "test.sqlite")

	// Create empty DB file
	f, err := os.Create(env.DBFile)
	Expect(err).NotTo(HaveOccurred())
	f.Close()

	// Generate config.yaml
	apisYaml := ""
	if len(specFiles) > 0 {
		apisYaml = "\n    apis:"
		for name, path := range specFiles {
			// Read the actual content from the source path
			// Assuming path is relative to project root
			fullPath := filepath.Join(projectRoot, path)
			content, err := os.ReadFile(fullPath)
			Expect(err).NotTo(HaveOccurred())

			// Write spec to file
			specPath := filepath.Join(env.TmpDir, name+".yaml")
			err = os.WriteFile(specPath, content, 0644)
			Expect(err).NotTo(HaveOccurred())

			apisYaml += fmt.Sprintf(`
      - path: %s
`, specPath)
		}
	}

	keycloakURL := os.Getenv("KEYCLOAK_URL")
	oidcConfig := ""
	if keycloakURL != "" {
		oidcConfig = fmt.Sprintf(`
  - name: keycloak
    type: oidc
    enabled: true
    config:
      issuer: %s/realms/apify
      client_id: apify-test-client
      client_secret: apify-test-secret
`, keycloakURL)
	}

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
%s

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
    enabled: true
    port: %s
`, serverPort, env.DBFile, serverPort, apisYaml, env.APIKey, oidcConfig, env.DBFile, env.MetricsPort)

	err = os.WriteFile(env.ConfigFile, []byte(configContent), 0644)
	Expect(err).NotTo(HaveOccurred())

	// Start Control Plane
	env.CPCmd = exec.Command("cargo", "run", "--bin", "apify-cp", "--", "--config", env.ConfigFile)
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

	// Stop Control Plane
	if env.CPCmd.Process != nil {
		env.CPCmd.Process.Kill()
		env.CPCmd.Wait()
	}

	// Start Data Plane
	env.ServerCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--config", env.ConfigFile)
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

func indent(s string, n int) string {
	lines := strings.Split(s, "\n")
	pad := strings.Repeat(" ", n)
	for i, line := range lines {
		if line != "" {
			lines[i] = pad + line
		}
	}
	return strings.Join(lines, "\n")
}

