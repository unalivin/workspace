package inventory

import (
	"context"
	"database/sql"
	"encoding/csv"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strconv"
	"strings"
)

const (
	MaxImportBytes = 4 << 20
	MaxImportRows  = 5000
)

var csvHeader = []string{"host", "port", "server_name", "note", "enabled"}

type exported struct {
	Host       string `json:"host"`
	Port       int    `json:"port"`
	ServerName string `json:"server_name"`
	Note       string `json:"note"`
	Enabled    *bool  `json:"enabled,omitempty"`
}

func unguardCell(value string) string {
	if len(value) > 1 && value[0] == '\'' && strings.ContainsRune("=+-@\t\r", rune(value[1])) {
		return value[1:]
	}
	return value
}

func ExportEndpoints(w io.Writer, format string, list []Endpoint) error {
	switch format {
	case FormatCSV:
		writer := csv.NewWriter(w)
		if err := writer.Write(csvHeader); err != nil {
			return err
		}
		for _, e := range list {
			if err := writer.Write([]string{e.Host, strconv.Itoa(e.Port), e.ServerName, GuardCell(e.Note), strconv.FormatBool(e.Enabled)}); err != nil {
				return err
			}
		}
		writer.Flush()
		return writer.Error()
	case FormatJSON:
		items := make([]exported, 0, len(list))
		for _, e := range list {
			enabled := e.Enabled
			items = append(items, exported{Host: e.Host, Port: e.Port, ServerName: e.ServerName, Note: e.Note, Enabled: &enabled})
		}
		encoder := json.NewEncoder(w)
		encoder.SetIndent("", "  ")
		return encoder.Encode(items)
	}
	return &ValidationError{Field: "format", Problem: fmt.Sprintf("unknown format %q", format), Fix: "Use csv or json"}
}

type RowError struct {
	Row     int
	Problem string
}

func (e RowError) Error() string {
	return fmt.Sprintf("row %d: %s", e.Row, e.Problem)
}

func parseBool(value string) (bool, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", "true", "yes", "1":
		return true, nil
	case "false", "no", "0":
		return false, nil
	}
	return false, fmt.Errorf("enabled: %q is not true or false", value)
}

func ParseEndpoints(r io.Reader, format string) ([]EndpointInput, []RowError, error) {
	data, err := io.ReadAll(io.LimitReader(r, MaxImportBytes+1))
	if err != nil {
		return nil, nil, err
	}
	if len(data) > MaxImportBytes {
		return nil, nil, fmt.Errorf("the file is larger than %d MB; split it into smaller files", MaxImportBytes>>20)
	}
	var inputs []EndpointInput
	var problems []RowError
	add := func(row int, in EndpointInput, rowErr error) {
		if rowErr == nil {
			in, rowErr = Normalize(in)
		}
		if rowErr != nil {
			problems = append(problems, RowError{Row: row, Problem: rowErr.Error()})
			return
		}
		inputs = append(inputs, in)
	}
	switch format {
	case FormatCSV:
		reader := csv.NewReader(strings.NewReader(string(data)))
		reader.FieldsPerRecord = -1
		header, err := reader.Read()
		if errors.Is(err, io.EOF) {
			return nil, nil, errors.New("the file is empty; the first line must be the header host,port,server_name,note,enabled")
		}
		if err != nil {
			return nil, nil, fmt.Errorf("the file is not valid CSV: %w", err)
		}
		index := map[string]int{}
		for position, name := range header {
			index[strings.ToLower(strings.TrimSpace(strings.TrimPrefix(name, "\ufeff")))] = position
		}
		if _, found := index["host"]; !found {
			return nil, nil, errors.New("the header has no host column; the first line must name the columns, for example host,port,server_name,note,enabled")
		}
		row := 1
		for {
			fields, err := reader.Read()
			if errors.Is(err, io.EOF) {
				break
			}
			row++
			if err != nil {
				problems = append(problems, RowError{Row: row, Problem: err.Error()})
				continue
			}
			if row-1 > MaxImportRows {
				return nil, nil, fmt.Errorf("the file has more than %d rows; split it into smaller files", MaxImportRows)
			}
			cell := func(name string) string {
				position, found := index[name]
				if !found || position >= len(fields) {
					return ""
				}
				return strings.TrimSpace(fields[position])
			}
			in := EndpointInput{Host: cell("host"), ServerName: cell("server_name"), Note: unguardCell(cell("note"))}
			var rowErr error
			if text := cell("port"); text != "" {
				if in.Port, rowErr = strconv.Atoi(text); rowErr != nil {
					rowErr = fmt.Errorf("port: %q is not a number", text)
				}
			}
			if rowErr == nil {
				in.Enabled, rowErr = parseBool(cell("enabled"))
			}
			add(row, in, rowErr)
		}
	case FormatJSON:
		var items []exported
		decoder := json.NewDecoder(strings.NewReader(string(data)))
		decoder.DisallowUnknownFields()
		if err := decoder.Decode(&items); err != nil {
			return nil, nil, fmt.Errorf("the file is not a JSON list of endpoints: %w", err)
		}
		if len(items) > MaxImportRows {
			return nil, nil, fmt.Errorf("the file has more than %d rows; split it into smaller files", MaxImportRows)
		}
		for position, item := range items {
			add(position+1, EndpointInput{Host: item.Host, Port: item.Port, ServerName: item.ServerName, Note: item.Note, Enabled: item.Enabled == nil || *item.Enabled}, nil)
		}
	default:
		return nil, nil, &ValidationError{Field: "format", Problem: fmt.Sprintf("unknown format %q", format), Fix: "Use csv or json"}
	}
	return inputs, problems, nil
}

type ImportResult struct {
	Created int
	Known   int
	Errors  []RowError
	DryRun  bool
	Hosts   []string
}

func (r ImportResult) Summary() string {
	verb := "imported"
	if r.DryRun {
		verb = "would import"
	}
	return fmt.Sprintf("%s %d endpoints, %d already monitored, %d rejected", verb, r.Created, r.Known, len(r.Errors))
}

func (s *Service) ImportEndpoints(ctx context.Context, r io.Reader, format string, dryRun, authorized bool) (ImportResult, error) {
	inputs, problems, err := ParseEndpoints(r, format)
	if err != nil {
		return ImportResult{}, err
	}
	if !dryRun && !authorized && len(inputs) > 0 {
		return ImportResult{}, ErrNotAuthorized
	}
	var result ImportResult
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		result = ImportResult{Errors: problems, DryRun: dryRun}
		seen := map[string]bool{}
		for _, in := range inputs {
			address := Endpoint{Host: in.Host, Port: in.Port}.Address()
			_, err := scanEndpoint(tx.QueryRowContext(ctx, sqlEndpointByAddress, in.Host, in.Port))
			if err == nil || seen[address] {
				result.Known++
				continue
			}
			if !errors.Is(err, ErrNotFound) {
				return err
			}
			seen[address] = true
			result.Created++
			result.Hosts = append(result.Hosts, address)
			if dryRun {
				continue
			}
			if _, err := createEndpoint(ctx, tx, s.now(), in, false); err != nil {
				return err
			}
		}
		return nil
	})
	return result, err
}
