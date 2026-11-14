package app

import (
	"context"
	"crypto/x509"
	"fmt"
	"os"
	"sync"
	"time"

	"github.com/NexzaDev/certwatch/internal/inventory"
	"github.com/NexzaDev/certwatch/internal/settings"
	"github.com/NexzaDev/certwatch/internal/tlscheck"
)

const checkWorkers = 4

type CheckLine struct {
	EndpointID int64
	Address    string
	Outcome    string
	Detail     string
	Failed     bool
}

type CheckReport struct {
	Checked int
	Failed  int
	Changed int
	Alerts  int
	Skipped int
	Lines   []CheckLine
}

func (r CheckReport) Summary() string {
	text := fmt.Sprintf("checked %d %s: %d failed, %d changed certificates, %d new alerts", r.Checked, plural(r.Checked, "endpoint", "endpoints"), r.Failed, r.Changed, r.Alerts)
	if r.Skipped > 0 {
		text += fmt.Sprintf(", %d demo %s skipped", r.Skipped, plural(r.Skipped, "endpoint", "endpoints"))
	}
	return text
}

func plural(count int, one, many string) string {
	if count == 1 {
		return one
	}
	return many
}

func (a *App) Policy(ctx context.Context) (inventory.Policy, error) {
	values, err := a.Settings.Effective(ctx)
	if err != nil {
		return inventory.Policy{}, err
	}
	return inventory.Policy{Thresholds: values[settings.KeyExpiryDays].Days(), Keep: values[settings.KeyHistoryKeep].Int()}, nil
}

func (a *App) roots(path string) (*x509.CertPool, error) {
	if a.options.Roots != nil {
		return a.options.Roots, nil
	}
	if path == "" {
		return nil, nil
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("the file with additional trusted roots cannot be read: %w; correct the setting %s or clear it", err, settings.KeyTrustFile)
	}
	certificates, err := tlscheck.ParsePEM(data)
	if err != nil {
		return nil, fmt.Errorf("the file with additional trusted roots is not usable: %w", err)
	}
	pool, err := x509.SystemCertPool()
	if err != nil || pool == nil {
		pool = x509.NewCertPool()
	}
	for _, certificate := range certificates {
		pool.AddCert(certificate)
	}
	return pool, nil
}

func Disclosure(endpoints []inventory.Endpoint, probe bool) []string {
	var lines []string
	for _, endpoint := range endpoints {
		if endpoint.Demo {
			continue
		}
		line := fmt.Sprintf("%s: name lookup for %s, then one TCP connection with a TLS handshake", endpoint.Address(), endpoint.Host)
		if probe {
			line = fmt.Sprintf("%s: name lookup for %s, then up to four TCP connections with a TLS handshake each", endpoint.Address(), endpoint.Host)
		}
		if endpoint.ServerName != "" {
			line += ", asking for " + endpoint.ServerName
		}
		lines = append(lines, line)
	}
	return lines
}

func describeResult(result tlscheck.Result, observation inventory.Observation) (string, string, bool) {
	switch result.Outcome {
	case tlscheck.OutcomeUnreachable:
		return "unreachable", result.Error, true
	case tlscheck.OutcomeHandshakeFailed:
		return "handshake failed", result.Error, true
	}
	detail := fmt.Sprintf("%s, %d %s", result.Version, len(observation.Findings), plural(len(observation.Findings), "finding", "findings"))
	if leaf := result.Leaf(); leaf != nil {
		detail = fmt.Sprintf("expires %s, %s", leaf.NotAfter.UTC().Format("2006-01-02"), detail)
	}
	if observation.Changed {
		detail += ", certificate changed"
	}
	switch {
	case result.ChainStatus == tlscheck.StatusInvalid && result.HostnameStatus == tlscheck.HostnameMismatch:
		return "chain and name failed", detail, true
	case result.ChainStatus == tlscheck.StatusInvalid:
		return "chain not trusted", detail, true
	case result.HostnameStatus == tlscheck.HostnameMismatch:
		return "name mismatch", detail, true
	}
	return "ok", detail, false
}

func (a *App) CheckEndpoints(ctx context.Context, endpoints []inventory.Endpoint, probe bool) (CheckReport, error) {
	var report CheckReport
	values, err := a.Settings.Effective(ctx)
	if err != nil {
		return report, err
	}
	roots, err := a.roots(values[settings.KeyTrustFile].Raw)
	if err != nil {
		return report, err
	}
	policy := inventory.Policy{Thresholds: values[settings.KeyExpiryDays].Days(), Keep: values[settings.KeyHistoryKeep].Int()}
	options := tlscheck.Options{Timeout: time.Duration(values[settings.KeyCheckTimeout].Int()) * time.Second, Roots: roots, Now: a.options.Now, Dial: a.options.Dial, Protocols: probe}
	var targets []inventory.Endpoint
	for _, endpoint := range endpoints {
		if endpoint.Demo {
			report.Skipped++
			continue
		}
		targets = append(targets, endpoint)
	}
	lines := make([]CheckLine, len(targets))
	errs := make([]error, len(targets))
	var guard sync.Mutex
	var group sync.WaitGroup
	queue := make(chan int)
	for worker := 0; worker < checkWorkers; worker++ {
		group.Add(1)
		go func() {
			defer group.Done()
			for index := range queue {
				endpoint := targets[index]
				result := tlscheck.Check(ctx, tlscheck.Target{Host: endpoint.Host, Port: endpoint.Port, ServerName: endpoint.ServerName}, options)
				if ctx.Err() != nil {
					errs[index] = ctx.Err()
					continue
				}
				guard.Lock()
				observation, err := a.Inventory.Record(ctx, endpoint.ID, result, policy)
				if err == nil {
					outcome, detail, failed := describeResult(result, observation)
					lines[index] = CheckLine{EndpointID: endpoint.ID, Address: endpoint.Address(), Outcome: outcome, Detail: detail, Failed: failed}
					report.Checked++
					report.Alerts += observation.Alerts
					if failed {
						report.Failed++
					}
					if observation.Changed {
						report.Changed++
					}
				}
				errs[index] = err
				guard.Unlock()
			}
		}()
	}
	for index := range targets {
		queue <- index
	}
	close(queue)
	group.Wait()
	for index, line := range lines {
		if errs[index] != nil {
			return report, errs[index]
		}
		report.Lines = append(report.Lines, line)
	}
	a.touch()
	return report, nil
}

func (a *App) CheckInterval(ctx context.Context) (time.Duration, error) {
	value, err := a.Settings.Get(ctx, settings.KeyCheckInterval)
	if err != nil {
		return 0, err
	}
	return time.Duration(value.Int()) * time.Minute, nil
}

func (a *App) probe(ctx context.Context) bool {
	value, err := a.Settings.Get(ctx, settings.KeyProbeProtocols)
	return err == nil && value.Bool()
}

func (a *App) CheckDue(ctx context.Context, interval time.Duration) (CheckReport, error) {
	due, err := a.Inventory.DueEndpoints(ctx, interval)
	if err != nil || len(due) == 0 {
		return CheckReport{}, err
	}
	return a.CheckEndpoints(ctx, due, a.probe(ctx))
}

type Housekeeping struct {
	Report CheckReport
	Alerts int
}

func (a *App) Housekeep(ctx context.Context) (Housekeeping, error) {
	var done Housekeeping
	policy, err := a.Policy(ctx)
	if err != nil {
		return done, err
	}
	if done.Alerts, err = a.Inventory.EvaluateExpiry(ctx, policy); err != nil {
		return done, err
	}
	if done.Alerts > 0 {
		a.touch()
	}
	interval, err := a.CheckInterval(ctx)
	if err != nil || interval <= 0 {
		return done, err
	}
	done.Report, err = a.CheckDue(ctx, interval)
	return done, err
}

func (a *App) StartScheduler(ctx context.Context, every time.Duration, report func(Housekeeping, error)) (stop func()) {
	ctx, cancel := context.WithCancel(ctx)
	finished := make(chan struct{})
	go func() {
		defer close(finished)
		ticker := time.NewTicker(every)
		defer ticker.Stop()
		for {
			done, err := a.Housekeep(ctx)
			if ctx.Err() != nil {
				return
			}
			if report != nil && (err != nil || done.Alerts > 0 || done.Report.Checked > 0) {
				report(done, err)
			}
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
			}
		}
	}()
	return func() {
		cancel()
		<-finished
	}
}
