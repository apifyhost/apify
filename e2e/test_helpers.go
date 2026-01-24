package e2e
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

// SetupControlPlaneEnv creates a minimal test environment with only Control Plane
func SetupControlPlaneEnv() (*TestEnv, *http.Client, error) {
	env := &TestEnv{}

	wd, _ := os.Getwd()
	projectRoot := filepath.Dir(wd)

	// Get a free port for CP
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {


















































































































}	return client.Do(req)	}		return nil, err	if err != nil {	req, err := http.NewRequest(http.MethodDelete, url, nil)func deleteRequest(client *http.Client, url string) (*http.Response, error) {// Helper function to make DELETE request}	return client.Do(req)	req.Header.Set("Content-Type", "application/json")	}		return nil, err	if err != nil {	req, err := http.NewRequest(http.MethodPut, url, bytes.NewBuffer(jsonBytes))	}		return nil, err	if err != nil {	jsonBytes, err := json.Marshal(body)func putJSON(client *http.Client, url string, body interface{}) (*http.Response, error) {// Helper function to make PUT request}	return json.NewDecoder(resp.Body).Decode(target)	defer resp.Body.Close()func decodeJSON(resp *http.Response, target interface{}) error {// Helper function to decode JSON response}	return env, client, nil	}, 60*time.Second, 1*time.Second).Should(Succeed())		return nil		}			return fmt.Errorf("status code %d", resp.StatusCode)		if resp.StatusCode != 200 {		defer resp.Body.Close()		}			return err		if err != nil {		resp, err := client.Get(env.CPBaseURL + "/apify/admin/apis")	Eventually(func() error {	// Wait for CP to be ready	client := &http.Client{Timeout: 10 * time.Second}	}		return nil, nil, err	if err != nil {	err = env.CPCmd.Start()	env.CPCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+env.DBFile)	env.CPCmd.Dir = projectRoot	}		env.CPCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--control-plane", "--config", env.ConfigFile)	} else {		env.CPCmd = exec.Command(binPath, "--control-plane", "--config", env.ConfigFile)	if _, err := os.Stat(binPath); err == nil {	binPath := filepath.Join(projectRoot, "target", "debug", "apify")	// Start Control Plane	}		return nil, nil, err	if err != nil {	err = os.WriteFile(env.ConfigFile, []byte(configContent), 0644)`, cpPort, env.DBFile, env.MetricsPort)    port: %s    enabled: true  metrics:modules:log_level: "info"    database: //%s    driver: sqlite  database:    port: %s    ip: 127.0.0.1  listen:control-plane:	configContent := fmt.Sprintf(`	// Create minimal config	f.Close()	}		return nil, nil, err	if err != nil {	f, err := os.Create(env.DBFile)	// Create empty DB file	env.DBFile = filepath.Join(env.TmpDir, "test.sqlite")	env.ConfigFile = filepath.Join(env.TmpDir, "config.yaml")	}		return nil, nil, err	if err != nil {	env.TmpDir, err = os.MkdirTemp("", "apify-e2e-crud")	// Create temporary directory	l.Close()	env.MetricsPort = fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)	}		return nil, nil, err	if err != nil {	l, err = net.Listen("tcp", "127.0.0.1:0")	// Get a free port for metrics	env.CPBaseURL = "http://127.0.0.1:" + cpPort	l.Close()	cpPort := fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)	}		return nil, nil, err