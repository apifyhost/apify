package e2e_test

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

var _ = Describe("Observability Features", Ordered, func() {
	var (
		baseURL     string
		metricsURL  string
		apiKey      string
		client      *http.Client
		metricsPort string
	)

	BeforeAll(func() {
		baseURL = os.Getenv("BASE_URL")
		if baseURL == "" {
			baseURL = "http://localhost:3000"
		}

		// Metrics port - SQLite uses 9090, Postgres uses 9091
		metricsPort = os.Getenv("METRICS_PORT")
		if metricsPort == "" {
			metricsPort = "9090"
		}
		metricsURL = fmt.Sprintf("http://localhost:%s/metrics", metricsPort)

		apiKey = os.Getenv("API_KEY")
		if apiKey == "" {
			apiKey = "e2e-test-key-001"
		}

		client = &http.Client{
			Timeout: 10 * time.Second,
		}

		// Verify service is ready
		Eventually(func() error {
			resp, err := client.Get(baseURL + "/healthz")
			if err != nil {
				return err
			}
			defer resp.Body.Close()
			if resp.StatusCode != http.StatusOK {
				return fmt.Errorf("health check failed with status %d", resp.StatusCode)
			}
			return nil
		}, "10s", "500ms").Should(Succeed())
	})

	Describe("Prometheus Metrics Endpoint", func() {
		It("should expose metrics endpoint", func() {
			resp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))
			Expect(resp.Header.Get("Content-Type")).To(ContainSubstring("text/plain"))
		})

		It("should include HTTP request metrics", func() {
			// First make some requests to generate metrics
			for i := 0; i < 5; i++ {
				req, _ := http.NewRequest("GET", baseURL+"/healthz", nil)
				req.Header.Set("X-API-Key", apiKey)
				resp, err := client.Do(req)
				Expect(err).NotTo(HaveOccurred())
				resp.Body.Close()
			}

			// Wait a bit for metrics to be recorded
			time.Sleep(100 * time.Millisecond)

			// Check metrics
			resp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			body, err := io.ReadAll(resp.Body)
			Expect(err).NotTo(HaveOccurred())
			metricsText := string(body)

			// Verify HTTP metrics exist
			Expect(metricsText).To(ContainSubstring("apify_http_requests_total"))
			Expect(metricsText).To(ContainSubstring("apify_http_request_duration_seconds"))
			Expect(metricsText).To(ContainSubstring("apify_active_connections"))
		})

		It("should include worker threads gauge", func() {
			resp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			body, err := io.ReadAll(resp.Body)
			Expect(err).NotTo(HaveOccurred())
			metricsText := string(body)

			Expect(metricsText).To(ContainSubstring("apify_worker_threads"))
			// Should show 1 thread (APIFY_THREADS=1 in docker-compose)
			Expect(metricsText).To(ContainSubstring("apify_worker_threads 1"))
		})

		It("should track request counts by status code", func() {
			// Make successful request
			req, _ := http.NewRequest("GET", baseURL+"/healthz", nil)
			req.Header.Set("X-API-Key", apiKey)
			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			resp.Body.Close()

			// Make a 404 request
			req404, _ := http.NewRequest("GET", baseURL+"/nonexistent", nil)
			req404.Header.Set("X-API-Key", apiKey)
			resp404, err := client.Do(req404)
			Expect(err).NotTo(HaveOccurred())
			resp404.Body.Close()

			time.Sleep(100 * time.Millisecond)

			// Check metrics include status labels
			metricsResp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer metricsResp.Body.Close()

			body, err := io.ReadAll(metricsResp.Body)
			Expect(err).NotTo(HaveOccurred())
			metricsText := string(body)

			// Should have metrics with status="200" and status="404"
			Expect(metricsText).To(ContainSubstring(`status="200"`))
			Expect(metricsText).To(ContainSubstring(`status="404"`))
		})

		It("should include database query metrics after CRUD operations", func() {
			// Create an item to trigger database operations
			itemData := map[string]interface{}{
				"name":        "metrics-test-item",
				"description": "Testing metrics",
				"price":       99.99,
				"in_stock":    true,
			}
			jsonData, _ := json.Marshal(itemData)

			req, _ := http.NewRequest("POST", baseURL+"/items", strings.NewReader(string(jsonData)))
			req.Header.Set("Content-Type", "application/json")
			req.Header.Set("X-API-Key", apiKey)

			resp, err := client.Do(req)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			// Wait for metrics
			time.Sleep(100 * time.Millisecond)

			// Check for database metrics
			metricsResp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer metricsResp.Body.Close()

			body, err := io.ReadAll(metricsResp.Body)
			Expect(err).NotTo(HaveOccurred())
			metricsText := string(body)

			// Verify DB metrics exist
			Expect(metricsText).To(ContainSubstring("apify_db_queries_total"))
			Expect(metricsText).To(ContainSubstring("apify_db_query_duration_seconds"))

			// Should have operation labels
			Expect(metricsText).To(ContainSubstring(`operation="insert"`))
			Expect(metricsText).To(ContainSubstring(`table="items"`))
		})

		It("should include histogram buckets for request duration", func() {
			resp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			body, err := io.ReadAll(resp.Body)
			Expect(err).NotTo(HaveOccurred())
			metricsText := string(body)

			// Check for histogram buckets
			Expect(metricsText).To(ContainSubstring("apify_http_request_duration_seconds_bucket"))
			Expect(metricsText).To(ContainSubstring(`le="0.001"`))
			Expect(metricsText).To(ContainSubstring(`le="0.01"`))
			Expect(metricsText).To(ContainSubstring(`le="0.1"`))
			Expect(metricsText).To(ContainSubstring(`le="1"`))
			Expect(metricsText).To(ContainSubstring(`le="+Inf"`))
		})
	})

	Describe("Structured Logging", func() {
		It("should produce JSON formatted logs", func() {
			// This test verifies log format by checking docker logs
			// In a real environment, you'd check log aggregation system
			Skip("Log format verification requires log collection setup")
		})
	})

	Describe("Health Check Endpoint", func() {
		It("should return healthy status", func() {
			resp, err := client.Get(baseURL + "/healthz")
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			Expect(resp.StatusCode).To(Equal(http.StatusOK))

			var health map[string]interface{}
			err = json.NewDecoder(resp.Body).Decode(&health)
			Expect(err).NotTo(HaveOccurred())
			Expect(health["status"]).To(Equal("ok"))
		})

		It("should be included in metrics", func() {
			// Make health check requests
			for i := 0; i < 3; i++ {
				resp, err := client.Get(baseURL + "/healthz")
				Expect(err).NotTo(HaveOccurred())
				resp.Body.Close()
			}

			time.Sleep(100 * time.Millisecond)

			// Verify in metrics
			metricsResp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer metricsResp.Body.Close()

			body, err := io.ReadAll(metricsResp.Body)
			Expect(err).NotTo(HaveOccurred())
			metricsText := string(body)

			// Should track /healthz requests
			Expect(metricsText).To(ContainSubstring(`path="/healthz"`))
		})
	})

	Describe("Metrics Performance", func() {
		It("should handle high request volume", func() {
			startTime := time.Now()

			// Generate load
			for i := 0; i < 50; i++ {
				req, _ := http.NewRequest("GET", baseURL+"/healthz", nil)
				req.Header.Set("X-API-Key", apiKey)
				resp, err := client.Do(req)
				if err == nil {
					resp.Body.Close()
				}
			}

			elapsed := time.Since(startTime)
			fmt.Printf("50 requests completed in %v\n", elapsed)

			// Metrics should still be available
			metricsResp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer metricsResp.Body.Close()

			Expect(metricsResp.StatusCode).To(Equal(http.StatusOK))

			body, err := io.ReadAll(metricsResp.Body)
			Expect(err).NotTo(HaveOccurred())

			// Should have all expected metrics
			metricsText := string(body)
			Expect(metricsText).To(ContainSubstring("apify_http_requests_total"))
			Expect(metricsText).To(ContainSubstring("apify_http_request_duration_seconds"))
		})

		It("should report metrics quickly", func() {
			startTime := time.Now()

			resp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer resp.Body.Close()

			elapsed := time.Since(startTime)

			// Metrics endpoint should respond quickly (< 100ms)
			Expect(elapsed).To(BeNumerically("<", 100*time.Millisecond))
		})
	})

	Describe("Active Connections Gauge", func() {
		It("should track active connections", func() {
			// Get initial metrics
			resp1, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			body1, _ := io.ReadAll(resp1.Body)
			resp1.Body.Close()

			// Active connections should be 0 or low when idle
			metricsText := string(body1)
			Expect(metricsText).To(ContainSubstring("apify_active_connections"))
		})
	})

	Describe("Metric Labels", func() {
		It("should include method labels", func() {
			// Make requests with different methods
			getReq, _ := http.NewRequest("GET", baseURL+"/healthz", nil)
			getReq.Header.Set("X-API-Key", apiKey)
			getResp, _ := client.Do(getReq)
			if getResp != nil {
				getResp.Body.Close()
			}

			time.Sleep(100 * time.Millisecond)

			metricsResp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer metricsResp.Body.Close()

			body, _ := io.ReadAll(metricsResp.Body)
			metricsText := string(body)

			// Should have method labels
			Expect(metricsText).To(ContainSubstring(`method="GET"`))
		})

		It("should include path labels", func() {
			metricsResp, err := client.Get(metricsURL)
			Expect(err).NotTo(HaveOccurred())
			defer metricsResp.Body.Close()

			body, _ := io.ReadAll(metricsResp.Body)
			metricsText := string(body)

			// Should have path labels
			Expect(metricsText).To(ContainSubstring(`path=`))
		})
	})
})
