package service

import (
	"fmt"
	"strings"
)

type DiffSummary struct {
	Added     int        `json:"added"`
	Removed   int        `json:"removed"`
	Unchanged int        `json:"unchanged"`
	Lines     []DiffLine `json:"lines"`
}
type DiffLine struct {
	Kind string `json:"kind"`
	Text string `json:"text"`
}

// CompareConfigurations computes a deterministic line diff suitable for human approval.
// It intentionally does not execute or apply commands.
func CompareConfigurations(before, after string) DiffSummary {
	left := strings.Split(strings.TrimSuffix(before, "\n"), "\n")
	right := strings.Split(strings.TrimSuffix(after, "\n"), "\n")
	if before == "" {
		left = nil
	}
	if after == "" {
		right = nil
	}
	// Longest-common-subsequence gives stable, reviewable output for small device configs.
	n, m := len(left), len(right)
	dp := make([][]int, n+1)
	for i := range dp {
		dp[i] = make([]int, m+1)
	}
	for i := n - 1; i >= 0; i-- {
		for j := m - 1; j >= 0; j-- {
			if left[i] == right[j] {
				dp[i][j] = dp[i+1][j+1] + 1
			} else if dp[i+1][j] >= dp[i][j+1] {
				dp[i][j] = dp[i+1][j]
			} else {
				dp[i][j] = dp[i][j+1]
			}
		}
	}
	out := DiffSummary{Lines: []DiffLine{}}
	i, j := 0, 0
	for i < n || j < m {
		if i < n && j < m && left[i] == right[j] {
			out.Unchanged++
			out.Lines = append(out.Lines, DiffLine{Kind: "unchanged", Text: left[i]})
			i++
			j++
			continue
		}
		if j < m && (i == n || dp[i][j+1] > dp[i+1][j]) {
			out.Added++
			out.Lines = append(out.Lines, DiffLine{Kind: "added", Text: right[j]})
			j++
			continue
		}
		if i < n {
			out.Removed++
			out.Lines = append(out.Lines, DiffLine{Kind: "removed", Text: left[i]})
			i++
		}
	}
	return out
}
func (d DiffSummary) Validate() error {
	for _, line := range d.Lines {
		if line.Kind != "added" && line.Kind != "removed" && line.Kind != "unchanged" {
			return fmt.Errorf("invalid diff kind %q", line.Kind)
		}
	}
	return nil
}
func (d DiffSummary) Unified() string {
	var b strings.Builder
	for _, line := range d.Lines {
		prefix := " "
		switch line.Kind {
		case "added":
			prefix = "+"
		case "removed":
			prefix = "-"
		}
		b.WriteString(prefix)
		b.WriteString(line.Text)
		b.WriteByte('\n')
	}
	return b.String()
}
