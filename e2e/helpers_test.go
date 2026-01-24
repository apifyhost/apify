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
	"time"

	. "github.com/onsi/gomega"
)

// decodeJSON decodes a JSON response
func decodeJSON(resp *http.Response, target interface{}) error {
	defer resp.Body.Close()
	return json.NewDecoder(resp.Body).Decode(target)
}

// putJSON makes a PUT request with JSON body
func putJSON(client *http.Client, url string, body interface{}) (*http.Response, error) {
	jsonBytes, err := json.Marshal(body)
	if err != nil {
		return nil, err
	}

	req, err := http.NewRequest(http.MethodPut, url, bytes.NewBuffer(jsonBytes))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")

	return client.Do(req)
}

// deleteRequest makes a DELETE request
func deleteRequest(client *http.Client, url string) (*http.Response, error) {
	req, err := http.NewRequest(http.MethodDelete, url, nil)
	if err != nil {
		return nil, err
	}

	return client.Do(req)
}

// SetupControlPlaneEnv creates a minimal test environment with only Control Plane
func SetupControlPlaneEnv() (*TestEnv, *http.Client, error) {
	env := &TestEnv{}

	wd, _ := os.Getwd()
	projectRoot := filepath.Dir(wd)

	// Create temporary directory
	tmpDir, err := os.MkdirTemp("", "apify-e2e-crud")
	if err != nil {
		return nil, nil, err
	}
	env.TmpDir = tmpDir
	env.ConfigFile = filepath.Join(tmpDir, "config.yaml")
	env.DBFile = filepath.Join(tmpDir, "test.sqlite")

	// Create empty DB file
	f, err := os.Create(env.DBFile)
	if err != nil {
		return nil, nil, err
	}
	f.Close()

	// Get a free port for CP
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, nil, err
	}
	cpPort := fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
	l.Close()

	// Get a free port for metrics
	l, err = net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, nil, err
	}
	env.MetricsPort = fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
	l.Close()

	env.CPPort = cpPort
	env.CPBaseURL = "http://127.0.0.1:" + cpPort

	// Create minimal config
	configContent := fmt.Sprintf(`control-plane:
  listen:
    ip: 127.0.0.1
    port: %s
  database:
    driver: sqlite
    database: //%s
modules:
  metrics:
    enabled: true
    port: %s
log_level: "info"
`, cpPort, env.DBFile, env.MetricsPort)

	err = os.WriteFile(env.ConfigFile, []byte(configContent), 0644)
	if err != nil {
		return nil, nil, err
	}

	// Start Control Plane
	binPath := filepath.Join(projectRoot, "target", "debug", "apify")
	if _, err := os.Stat(binPath); err == nil {
		env.CPCmd = exec.Command(binPath, "--control-plane", "--config", env.ConfigFile)
	} else {
		env.CPCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--control-plane", "--config", env.ConfigFile)
	}

	env.CPCmd.Dir = projectRoot
	env.CPCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+env.DBFile)

	err = env.CPCmd.Start()
	if err != nil {
		return nil, nil, err
	}

	// Wait for CP to be ready
	client := &http.Client{Timeout: 10 * time.Second}
	Eventually(func() error {
		resp, err := client.Get(env.CPBaseURL + "/apify/admin/apis")
		if err != nil {
			return err
		}
		defer resp.Body.Close()
		if resp.StatusCode != 200 {
			return fmt.Errorf("status code %d", resp.StatusCode)
		}
		return nil
	}, 60*time.Second, 1*time.Second).Should(Succeed())
	return env, client, nil
}
