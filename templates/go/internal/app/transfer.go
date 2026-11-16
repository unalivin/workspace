package app

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/unalivin/templateapp/internal/records"
)

const exportLimit = 10000

type FormatError struct{ Message string }

func (e *FormatError) Error() string { return e.Message }

type ExistsError struct{ Path string }

func (e *ExistsError) Error() string {
	return e.Path + " already exists: choose another name or allow overwriting it"
}

func TransferFormat(name, explicit string) (string, error) {
	switch strings.ToLower(explicit) {
	case records.FormatCSV, records.FormatJSON:
		return strings.ToLower(explicit), nil
	case "":
	default:
		return "", &FormatError{fmt.Sprintf("unknown format %q: use csv or json", explicit)}
	}
	switch strings.ToLower(filepath.Ext(name)) {
	case ".csv":
		return records.FormatCSV, nil
	case ".json":
		return records.FormatJSON, nil
	}
	return "", &FormatError{"the format cannot be told from the file name: choose csv or json"}
}

func (a *App) ImportFile(ctx context.Context, path, format string, dryRun bool) (records.ImportResult, error) {
	format, err := TransferFormat(path, format)
	if err != nil {
		return records.ImportResult{}, err
	}
	file, err := os.Open(path)
	if err != nil {
		return records.ImportResult{}, fmt.Errorf("cannot open the import file: %w", err)
	}
	defer file.Close()
	return a.Records.Import(ctx, file, format, dryRun)
}

func (a *App) ExportBytes(ctx context.Context, format string) ([]byte, int, error) {
	list, err := a.Records.List(ctx, records.Query{IncludeDisabled: true, Limit: exportLimit})
	if err != nil {
		return nil, 0, err
	}
	var buffer bytes.Buffer
	if err := records.Export(&buffer, format, list); err != nil {
		return nil, 0, err
	}
	return buffer.Bytes(), len(list), nil
}

func (a *App) ExportFile(ctx context.Context, path, format string, overwrite bool) (int, error) {
	if strings.TrimSpace(path) == "" {
		return 0, &FormatError{"give the file to write, for example records.csv"}
	}
	format, err := TransferFormat(path, format)
	if err != nil {
		return 0, err
	}
	content, count, err := a.ExportBytes(ctx, format)
	if err != nil {
		return 0, err
	}
	mode := os.O_WRONLY | os.O_CREATE | os.O_EXCL
	if overwrite {
		mode = os.O_WRONLY | os.O_CREATE | os.O_TRUNC
	}
	file, err := os.OpenFile(path, mode, 0o600)
	if errors.Is(err, os.ErrExist) {
		return 0, &ExistsError{Path: path}
	}
	if err != nil {
		return 0, fmt.Errorf("cannot write the export: %w", err)
	}
	if _, err := file.Write(content); err != nil {
		_ = file.Close()
		return 0, fmt.Errorf("cannot write the export: %w", err)
	}
	if err := file.Close(); err != nil {
		return 0, fmt.Errorf("cannot write the export: %w", err)
	}
	return count, nil
}
