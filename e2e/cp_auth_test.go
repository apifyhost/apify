package e2e_test

import (
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Control Plane Authentication", func() {
	var (
		cpCmd       *exec.Cmd
		tmpDir      string
		cpBaseURL   string
		metricsPort string
		client      *http.Client
		authToken   = "secret-token-123"
	)

	BeforeEach(func() {
		var err error
		wd, _ := os.Getwd()
		projectRoot := filepath.Dir(wd)

		// Get a free port for CP
		l, err := net.Listen("tcp", "127.0.0.1:0")
		Expect(err).NotTo(HaveOccurred())
		cpPort := fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
		l.Close()
		cpBaseURL = "http://127.0.0.1:" + cpPort

		// Get a free port for metrics
		l, err = net.Listen("tcp", "127.0.0.1:0")
		Expect(err).NotTo(HaveOccurred())
		metricsPort = fmt.Sprintf("%d", l.Addr().(*net.TCPAddr).Port)
		l.Close()

		// Create temporary directory
		tmpDir, err = os.MkdirTemp("", "apify-e2e-auth-test")
		Expect(err).NotTo(HaveOccurred())

		configFile := filepath.Join(tmpDir, "config.yaml")
		dbFile := filepath.Join(tmpDir, "test.sqlite")

		// Create empty DB file
		f, err := os.Create(dbFile)
		Expect(err).NotTo(HaveOccurred())
		f.Close()

		// Create config with admin_key
		configContent := fmt.Sprintf(`
control-plane:
  listen:
    ip: 127.0.0.1
    port: %s
  database:
    driver: sqlite
    database: //%s
  admin_key: "%s"

log_level: "info"

modules:
  metrics:
    enabled: true
    port: %s
`, cpPort, dbFile, authToken, metricsPort)

		err = os.WriteFile(configFile, []byte(configContent), 0644)
		Expect(err).NotTo(HaveOccurred())

		// Start Control Plane
		binPath := filepath.Join(projectRoot, "target", "debug", "apify")
		if _, err := os.Stat(binPath); err == nil {
			cpCmd = exec.Command(binPath, "--control-plane", "--config", configFile)
		} else {
			cpCmd = exec.Command("cargo", "run", "--bin", "apify", "--", "--control-plane", "--config", configFile)
		}
		cpCmd.Dir = projectRoot
		cpCmd.Env = append(os.Environ(), "APIFY_DB_URL=sqlite://"+dbFile)

		// Redirect stdout/stderr for debugging
		cpCmd.Stdout = GinkgoWriter
		cpCmd.Stderr = GinkgoWriter

		err = cpCmd.Start()
		Expect(err).NotTo(HaveOccurred())

		client = &http.Client{Timeout: 5 * time.Second}

		// Wait for CP to be ready (we use a loop with auth token to check readiness)
		Eventually(func() error {
			req, _ := http.NewRequest("GET", cpBaseURL+"/apify/admin/apis", nil)
			req.Header.Set("X-API-KEY", authToken)
			resp, err := client.Do(req)
			if err != nil {
				return err
			}
			defer resp.Body.Close()
			if resp.StatusCode != 200 {
				return fmt.Errorf("status code %d", resp.StatusCode)
			}
			return nil
		}, 10*time.Second, 500*time.Millisecond).Should(Succeed(), "Control Plane failed to start with auth")
	})

	AfterEach(func() {
		if cpCmd != nil && cpCmd.Process != nil {
			cpCmd.Process.Kill()
		}
		if tmpDir != "" {
			os.RemoveAll(tmpDir)
		}
	})

	It("should reject requests without api key header", func() {
		resp, err := client.Get(cpBaseURL + "/apify/admin/apis")
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized))
	})

	It("should reject requests with invalid key", func() {
		req, _ := http.NewRequest("GET", cpBaseURL+"/apify/admin/apis", nil)
		req.Header.Set("X-API-KEY", "invalid-key")
		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusUnauthorized))
	})

	It("should allow requests with valid key", func() {
		req, _ := http.NewRequest("GET", cpBaseURL+"/apify/admin/apis", nil)
		req.Header.Set("X-API-KEY", authToken)
		resp, err := client.Do(req)
		Expect(err).NotTo(HaveOccurred())
		defer resp.Body.Close()
		Expect(resp.StatusCode).To(Equal(http.StatusOK))
	})
})
