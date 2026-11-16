package app

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/unalivin/certwatch/internal/inventory"
)

type FormatError struct{ Message string }

func (e *FormatError) Error() string { return e.Message }

type ExistsError struct{ Path string }

func (e *ExistsError) Error() string {
	return e.Path + " already exists: choose another name or allow overwriting it"
}

func TransferFormat(name, explicit string) (string, error) {
	switch strings.ToLower(explicit) {
	case inventory.FormatCSV, inventory.FormatJSON:
		return strings.ToLower(explicit), nil
	case "":
	default:
		return "", &FormatError{fmt.Sprintf("unknown format %q: use csv or json", explicit)}
	}
	switch strings.ToLower(filepath.Ext(name)) {
	case ".csv":
		return inventory.FormatCSV, nil
	case ".json":
		return inventory.FormatJSON, nil
	}
	return "", &FormatError{"the format cannot be told from the file name: choose csv or json"}
}

func WriteFile(path string, content []byte, overwrite bool) error {
	if strings.TrimSpace(path) == "" {
		return &FormatError{"give the file to write"}
	}
	mode := os.O_WRONLY | os.O_CREATE | os.O_EXCL
	if overwrite {
		mode = os.O_WRONLY | os.O_CREATE | os.O_TRUNC
	}
	file, err := os.OpenFile(path, mode, 0o600)
	if errors.Is(err, os.ErrExist) {
		return &ExistsError{Path: path}
	}
	if err != nil {
		return fmt.Errorf("cannot write the file: %w", err)
	}
	if _, err := file.Write(content); err != nil {
		_ = file.Close()
		return fmt.Errorf("cannot write the file: %w", err)
	}
	if err := file.Close(); err != nil {
		return fmt.Errorf("cannot write the file: %w", err)
	}
	return nil
}

func ReadFile(path string, limit int64) ([]byte, error) {
	if strings.TrimSpace(path) == "" {
		return nil, &FormatError{"give the file to read"}
	}
	info, err := os.Stat(path)
	if err != nil {
		return nil, fmt.Errorf("cannot open the file: %w", err)
	}
	if info.IsDir() {
		return nil, &FormatError{path + " is a directory: give a file"}
	}
	if info.Size() > limit {
		return nil, &FormatError{fmt.Sprintf("%s is larger than %d MB: split it into smaller files", path, limit>>20)}
	}
	return os.ReadFile(path)
}
