package notify

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"time"

	"github.com/unalivin/certwatch/internal/meta"
)

const (
	MinInterval   = 15 * time.Second
	MaxInterval   = time.Hour
	batchLimit    = 50
	stateFileName = "notifier-state.json"
)

type Deliverer interface {
	Deliver(ctx context.Context, title, body string) error
}

type Source interface {
	After(ctx context.Context, afterID int64, limit int) ([]Notification, error)
	HighestID(ctx context.Context) (int64, error)
}

type Opener func(ctx context.Context) (Source, io.Closer, error)

type Runner struct {
	Interval   time.Duration
	ShowTitles bool
	StateDir   string
	Open       Opener
	Deliverer  Deliverer
	Wait       func(ctx context.Context, d time.Duration) error
	Log        func(string)
	MaxTicks   int
}

type state struct {
	LastID      int64 `json:"last_id"`
	Initialized bool  `json:"initialized"`
}

func ClampInterval(d time.Duration) time.Duration {
	if d < MinInterval {
		return MinInterval
	}
	if d > MaxInterval {
		return MaxInterval
	}
	return d
}

func (r *Runner) statePath() string {
	return filepath.Join(r.StateDir, stateFileName)
}

func (r *Runner) loadState() (state, error) {
	var current state
	raw, err := os.ReadFile(r.statePath())
	if errors.Is(err, fs.ErrNotExist) {
		return current, nil
	}
	if err != nil {
		return current, err
	}
	if len(raw) > 4096 || json.Unmarshal(raw, &current) != nil || current.LastID < 0 {
		return state{}, nil
	}
	return current, nil
}

func (r *Runner) saveState(current state) error {
	if err := os.MkdirAll(r.StateDir, 0o700); err != nil {
		return err
	}
	raw, err := json.Marshal(current)
	if err != nil {
		return err
	}
	temporary := r.statePath() + ".tmp"
	if err := os.WriteFile(temporary, raw, 0o600); err != nil {
		return err
	}
	return os.Rename(temporary, r.statePath())
}

func Summary(items []Notification, showTitles bool) (string, string) {
	title := meta.AppName
	if len(items) == 0 {
		return title, ""
	}
	if len(items) == 1 {
		if showTitles {
			return title, SafeTitle(items[0].Title)
		}
		return title, "1 new alert"
	}
	body := fmt.Sprintf("%d new alerts", len(items))
	if showTitles {
		top := items[0]
		for _, item := range items[1:] {
			if item.Severity.Rank() > top.Severity.Rank() {
				top = item
			}
		}
		body += ", most severe: " + SafeTitle(top.Title)
	}
	return title, body
}

func (r *Runner) Tick(ctx context.Context) (int, error) {
	source, closer, err := r.Open(ctx)
	if err != nil {
		return 0, fmt.Errorf("cannot open the database read-only: %w", err)
	}
	defer closer.Close()
	current, err := r.loadState()
	if err != nil {
		return 0, fmt.Errorf("cannot read the notifier state: %w", err)
	}
	if !current.Initialized {
		highest, err := source.HighestID(ctx)
		if err != nil {
			return 0, err
		}
		return 0, r.saveState(state{LastID: highest, Initialized: true})
	}
	items, err := source.After(ctx, current.LastID, batchLimit)
	if err != nil {
		return 0, err
	}
	var fresh []Notification
	for _, item := range items {
		if item.Unread() {
			fresh = append(fresh, item)
		}
	}
	if len(items) == 0 {
		return 0, nil
	}
	if len(fresh) > 0 {
		title, body := Summary(fresh, r.ShowTitles)
		if err := r.Deliverer.Deliver(ctx, title, body); err != nil {
			return 0, fmt.Errorf("cannot show the system notification: %w", err)
		}
	}
	current.LastID = items[len(items)-1].ID
	return len(fresh), r.saveState(current)
}

func (r *Runner) Run(ctx context.Context) error {
	interval := ClampInterval(r.Interval)
	wait := r.Wait
	if wait == nil {
		wait = func(ctx context.Context, d time.Duration) error {
			timer := time.NewTimer(d)
			defer timer.Stop()
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-timer.C:
				return nil
			}
		}
	}
	for tick := 0; r.MaxTicks == 0 || tick < r.MaxTicks; tick++ {
		if _, err := r.Tick(ctx); err != nil && r.Log != nil {
			r.Log(err.Error())
		}
		if err := wait(ctx, interval); err != nil {
			if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
				return nil
			}
			return err
		}
	}
	return nil
}
