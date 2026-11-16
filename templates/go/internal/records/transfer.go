package records

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/csv"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strconv"
	"strings"

	"github.com/unalivin/templateapp/internal/theme"
)

const (
	FormatCSV  = "csv"
	FormatJSON = "json"

	MaxImportBytes = 5 << 20
	MaxImportRows  = 10000
)

var csvHeader = []string{"name", "status", "severity", "note", "enabled"}

type exported struct {
	Name     string `json:"name"`
	Status   string `json:"status"`
	Severity string `json:"severity"`
	Note     string `json:"note"`
	Enabled  *bool  `json:"enabled,omitempty"`

	rejected error
}

func guardCell(value string) string {
	if value != "" && strings.ContainsRune("=+-@\t\r", rune(value[0])) {
		return "'" + value
	}
	return value
}

func unguardCell(value string) string {
	if len(value) > 1 && value[0] == '\'' && strings.ContainsRune("=+-@\t\r", rune(value[1])) {
		return value[1:]
	}
	return value
}

func Export(w io.Writer, format string, list []Record) error {
	switch format {
	case FormatCSV:
		writer := csv.NewWriter(w)
		if err := writer.Write(csvHeader); err != nil {
			return err
		}
		for _, record := range list {
			if err := writer.Write([]string{guardCell(record.Name), record.Status, string(record.Severity), guardCell(record.Note), strconv.FormatBool(record.Enabled)}); err != nil {
				return err
			}
		}
		writer.Flush()
		return writer.Error()
	case FormatJSON:
		out := make([]exported, 0, len(list))
		for _, record := range list {
			enabled := record.Enabled
			out = append(out, exported{Name: record.Name, Status: record.Status, Severity: string(record.Severity), Note: record.Note, Enabled: &enabled})
		}
		encoder := json.NewEncoder(w)
		encoder.SetIndent("", "  ")
		return encoder.Encode(out)
	}
	return fmt.Errorf("unknown format %q: use csv or json", format)
}

type RowError struct {
	Row int
	Err error
}

func (e RowError) Error() string {
	return fmt.Sprintf("row %d: %v", e.Row, e.Err)
}

func parseBool(value string) (bool, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", "true", "yes", "1", "on":
		return true, nil
	case "false", "no", "0", "off":
		return false, nil
	}
	return false, &ValidationError{Field: "enabled", Problem: fmt.Sprintf("%q is not a yes or no value", value), Fix: "Use true or false"}
}

func Parse(r io.Reader, format string) ([]Input, []RowError, error) {
	raw, err := io.ReadAll(io.LimitReader(r, MaxImportBytes+1))
	if err != nil {
		return nil, nil, fmt.Errorf("cannot read the import: %w", err)
	}
	if len(raw) > MaxImportBytes {
		return nil, nil, fmt.Errorf("the import is larger than %d MB: split it into smaller files", MaxImportBytes>>20)
	}
	raw = bytes.TrimPrefix(raw, []byte("\xef\xbb\xbf"))
	if len(bytes.TrimSpace(raw)) == 0 {
		return nil, nil, errors.New("the import is empty: export a list first to see the expected layout")
	}
	var rows []exported
	var lines []int
	switch format {
	case FormatCSV:
		reader := csv.NewReader(bytes.NewReader(raw))
		reader.FieldsPerRecord = -1
		header, err := reader.Read()
		if err != nil {
			return nil, nil, fmt.Errorf("the CSV header cannot be read: %w", err)
		}
		position := map[string]int{}
		for index, name := range header {
			position[strings.ToLower(strings.TrimSpace(name))] = index
		}
		if _, ok := position["name"]; !ok {
			return nil, nil, errors.New(`the CSV header has no "name" column: the first line must be name,status,severity,note,enabled`)
		}
		for row := 2; ; row++ {
			fields, err := reader.Read()
			if errors.Is(err, io.EOF) {
				break
			}
			if err != nil {
				return nil, nil, fmt.Errorf("the CSV cannot be read near row %d: %w", row, err)
			}
			if len(rows) == MaxImportRows {
				return nil, nil, fmt.Errorf("the import has more than %d rows: split it into smaller files", MaxImportRows)
			}
			cell := func(name string) string {
				index, ok := position[name]
				if !ok || index >= len(fields) {
					return ""
				}
				return fields[index]
			}
			item := exported{Name: unguardCell(cell("name")), Status: cell("status"), Severity: cell("severity"), Note: unguardCell(cell("note"))}
			if _, ok := position["enabled"]; ok {
				enabled, err := parseBool(cell("enabled"))
				item.Enabled, item.rejected = &enabled, err
			}
			rows, lines = append(rows, item), append(lines, row)
		}
	case FormatJSON:
		decoder := json.NewDecoder(bytes.NewReader(raw))
		decoder.DisallowUnknownFields()
		if err := decoder.Decode(&rows); err != nil {
			return nil, nil, fmt.Errorf("the JSON cannot be read: %w: the file must hold a list of objects with name, status, severity, note and enabled", err)
		}
		if decoder.More() {
			return nil, nil, errors.New("the JSON has extra content after the list: keep a single list in the file")
		}
		if len(rows) > MaxImportRows {
			return nil, nil, fmt.Errorf("the import has more than %d rows: split it into smaller files", MaxImportRows)
		}
		for index := range rows {
			lines = append(lines, index+1)
		}
	default:
		return nil, nil, fmt.Errorf("unknown format %q: use csv or json", format)
	}
	var inputs []Input
	var problems []RowError
	seen := map[string]int{}
	for index, item := range rows {
		if item.rejected != nil {
			problems = append(problems, RowError{Row: lines[index], Err: item.rejected})
			continue
		}
		in := Input{Name: item.Name, Status: item.Status, Severity: theme.Severity(item.Severity), Note: item.Note, Enabled: item.Enabled == nil || *item.Enabled}
		normalized, err := Normalize(in)
		if err != nil {
			problems = append(problems, RowError{Row: lines[index], Err: err})
			continue
		}
		key := strings.ToLower(normalized.Name)
		if first, duplicate := seen[key]; duplicate {
			problems = append(problems, RowError{Row: lines[index], Err: &ValidationError{Field: "name", Problem: fmt.Sprintf("the name repeats row %d", first), Fix: "Keep one row per name"}})
			continue
		}
		seen[key] = lines[index]
		inputs = append(inputs, normalized)
	}
	return inputs, problems, nil
}

type ImportResult struct {
	Created  int
	Updated  int
	Skipped  int
	Problems []RowError
	DryRun   bool
}

func (r ImportResult) Summary() string {
	prefix := ""
	if r.DryRun {
		prefix = "Dry run: would have "
	}
	return fmt.Sprintf("%screated %d, updated %d, unchanged %d, rejected %d", prefix, r.Created, r.Updated, r.Skipped, len(r.Problems))
}

func (s *Service) Import(ctx context.Context, r io.Reader, format string, dryRun bool) (ImportResult, error) {
	inputs, problems, err := Parse(r, format)
	if err != nil {
		return ImportResult{}, err
	}
	result := ImportResult{Problems: problems, DryRun: dryRun}
	errDryRun := errors.New("dry run")
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		result.Created, result.Updated, result.Skipped = 0, 0, 0
		now := s.now()
		for _, in := range inputs {
			existing, err := scan(tx.QueryRowContext(ctx, sqlGetByName, in.Name))
			if errors.Is(err, sql.ErrNoRows) {
				if _, err := create(ctx, tx, now, in, false); err != nil {
					return err
				}
				result.Created++
				continue
			}
			if err != nil {
				return err
			}
			if existing.Input() == in {
				result.Skipped++
				continue
			}
			if _, err := tx.ExecContext(ctx, sqlUpdate, in.Name, in.Status, string(in.Severity), in.Note, flag(in.Enabled), now.UTC().Format(timeLayout), existing.ID, existing.Revision); err != nil {
				return duplicateName(err, in.Name)
			}
			after, err := scan(tx.QueryRowContext(ctx, sqlGet, existing.ID))
			if err != nil {
				return err
			}
			if err := announce(ctx, tx, now, &existing, after); err != nil {
				return err
			}
			result.Updated++
		}
		if dryRun {
			return errDryRun
		}
		return nil
	})
	if errors.Is(err, errDryRun) {
		err = nil
	}
	return result, err
}
