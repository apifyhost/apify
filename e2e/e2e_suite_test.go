package e2e_test

import (
	"bytes"
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
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

var _ = BeforeSuite(func() {
	// Build the binary once before running any tests to avoid timeouts due to compilation
	wd, _ := os.Getwd()
	projectRoot := filepath.Dir(wd)

	fmt.Println("Building apify binary...")
	cmd := exec.Command("cargo", "build", "--bin", "apify")
	cmd.Dir = projectRoot
	cmd.Stdout = GinkgoWriter
	cmd.Stderr = GinkgoWriter
	err := cmd.Run()
	Expect(err).NotTo(HaveOccurred(), "Failed to build apify binary")
	fmt.Println("Binary built successfully")
})

type TestEnv struct {
	ServerCmd   *exec.Cmd
	CPCmd       *exec.Cmd
	TmpDir      string
	BaseURL     string
	CPBaseURL   string
	APIKey      string
	ConfigFile  string
	DBFile      string
	MetricsPort string
}

func StartTestEnv(specFiles map[string]string) *TestEnv {
	var err error
	env := &TestEnv{}

	wd, _ := os.Getwd()
	projectRoot := filepath.Dir(wd)

	// Get a free port for DP
	l, err := net.Listen("tcp", "127.0.0.1:0")
	Expect(err).NotTo(HaveOccurred())
	dpPort := fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
	l.Close()
	env.BaseURL = "http://127.0.0.1:" + dpPort
	env.APIKey = "e2e-test-key-001"

	// Get a free port for CP
	l, err = net.Listen("tcp", "127.0.0.1:0")
	Expect(err).NotTo(HaveOccurred())
	cpPort := fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
	l.Close()
	env.CPBaseURL = "http://127.0.0.1:" + cpPort

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

	keycloakURL := os.Getenv("KEYCLOAK_URL")

	// 1. Create minimal config for startup (CP only)
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

	// Start Control Plane
	// Use pre-built binary if available to speed up tests and avoid timeouts
	binPath := filepath.Join(projectRoot, "target", "debug", "apify")
	if _, err := os.Stat(binPath); err == nil {
		env.CPCmd = exec.Command(binPath, "--control-plane", "--config", env.ConfigFile)
	} else {
		env.CPCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--control-plane", "--config", env.ConfigFile)
	}
	env.CPCmd.Dir = projectRoot
	env.CPCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+env.DBFile)

	var cpStdout, cpStderr bytes.Buffer
	env.CPCmd.Stdout = &cpStdout
	env.CPCmd.Stderr = &cpStderr

	err = env.CPCmd.Start()
	Expect(err).NotTo(HaveOccurred())

	client := &http.Client{Timeout: 5 * time.Second}

	// Wait for CP to be ready
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

	// 2. Prepare Import Config
	var authConfigs []map[string]interface{}
	authConfigs = append(authConfigs, map[string]interface{}{
		"name":    "e2e-api-keys",
		"type":    "api-key",
		"enabled": true,
		"config": map[string]interface{}{
			"source":   "header",
			"key_name": "X-Api-Key",
			"consumers": []map[string]interface{}{
				{
					"name": "default",
					"keys": []string{env.APIKey},
				},
			},
		},
	})

	if keycloakURL != "" {
		authConfigs = append(authConfigs, map[string]interface{}{
			"name":    "keycloak",
			"type":    "oidc",
			"enabled": true,
			"config": map[string]interface{}{
				"issuer":        fmt.Sprintf("%s/realms/apify", keycloakURL),
				"client_id":     "apify-test-client",
				"client_secret": "apify-test-secret",
			},
		})
	}

	var apiPaths []string
	for _, path := range specFiles {
		var fullPath string
		// If path starts with "api:", treat it as a logical name, not a file path
		if strings.HasPrefix(path, "api:") {
			fullPath = strings.TrimPrefix(path, "api:")
		} else if filepath.IsAbs(path) {
			fullPath = path
		} else {
			fullPath = filepath.Join(projectRoot, path)
		}
		apiPaths = append(apiPaths, fullPath)
	}

	dpPortInt, _ := strconv.Atoi(dpPort)
	importConfig := map[string]interface{}{
		"auth": authConfigs,
		"datasource": map[string]interface{}{
			"default": map[string]interface{}{
				"driver":        "sqlite",
				"database":      "//" + env.DBFile,
				"max_pool_size": 1,
			},
		},
		"listeners": []map[string]interface{}{
			{
				"port":     dpPortInt,
				"ip":       "127.0.0.1",
				"protocol": "HTTP",
				"apis":     apiPaths,
			},
		},
	}

	importYaml, err := yaml.Marshal(importConfig)
	Expect(err).NotTo(HaveOccurred())

	resp, err := client.Post(env.CPBaseURL+"/_meta/import", "application/x-yaml", bytes.NewBuffer(importYaml))
	Expect(err).NotTo(HaveOccurred())
	Expect(resp.StatusCode).To(Equal(200))
	resp.Body.Close()

	// Start Data Plane
	if _, err := os.Stat(binPath); err == nil {
		env.ServerCmd = exec.Command(binPath, "--data-plane", "--config", env.ConfigFile)
	} else {
		env.ServerCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--data-plane", "--config", env.ConfigFile)
	}
	env.ServerCmd.Dir = projectRoot
	env.ServerCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+env.DBFile, "APIFY_CONFIG_POLL_INTERVAL=1")

	var dpStdout, dpStderr bytes.Buffer
	env.ServerCmd.Stdout = &dpStdout
	env.ServerCmd.Stderr = &dpStderr

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
	}, "60s", "1s").Should(Succeed(), func() string {
		return fmt.Sprintf("Data Plane failed to start.\nStdout: %s\nStderr: %s", dpStdout.String(), dpStderr.String())
	})

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
