package inventory

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"net"
	"strconv"
	"strings"
	"time"

	"github.com/NexzaDev/certwatch/internal/store"
	"github.com/NexzaDev/certwatch/internal/tlscheck"
)

const (
	KindEndpoint    = "endpoint"
	KindCertificate = "certificate"
	KindReport      = "report"

	DefaultPort   = 443
	MaxNoteLength = 4000
	MaxHostLength = 253

	timeLayout = "2006-01-02T15:04:05Z"

	endpointColumns = "endpoint_id, host, port, server_name, note, enabled, is_demo, revision, authorized_at, created_at, updated_at, observation_id, observed_at, outcome, error, leaf_certificate_id, chain_status, chain_error, hostname_status, hostname_error, tls_version, cipher_suite, accepted_versions, protocols_tested, changed, certificate_name, certificate_issuer, not_after"

	sqlEndpointInsert     = "INSERT INTO endpoints (host, port, server_name, note, enabled, is_demo, authorized_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
	sqlEndpointGet        = "SELECT " + endpointColumns + " FROM endpoint_status WHERE endpoint_id = ?"
	sqlEndpointByAddress  = "SELECT " + endpointColumns + " FROM endpoint_status WHERE host = ? AND port = ?"
	sqlEndpointUpdate     = "UPDATE endpoints SET host = ?, port = ?, server_name = ?, note = ?, enabled = ?, revision = revision + 1, updated_at = ? WHERE id = ? AND revision = ?"
	sqlEndpointSetEnabled = "UPDATE endpoints SET enabled = ?, revision = revision + 1, updated_at = ? WHERE id = ? AND enabled <> ?"
	sqlEndpointDelete     = "DELETE FROM endpoints WHERE id = ?"
	sqlEndpointCount      = "SELECT count(*) FROM endpoints"
	sqlEndpointList       = "SELECT " + endpointColumns + ` FROM endpoint_status
WHERE (?1 = '' OR instr(lower(host || ':' || port || ' ' || server_name || ' ' || note || ' ' || certificate_name), ?1) > 0)
  AND (?2 = 'all'
    OR (?2 = 'enabled' AND enabled = 1)
    OR (?2 = 'disabled' AND enabled = 0)
    OR (?2 = 'never' AND enabled = 1 AND outcome = 'never')
    OR (?2 = 'unreachable' AND enabled = 1 AND outcome IN ('unreachable', 'handshake_failed'))
    OR (?2 = 'chain' AND enabled = 1 AND chain_status = 'invalid')
    OR (?2 = 'hostname' AND enabled = 1 AND hostname_status = 'mismatch')
    OR (?2 = 'changed' AND enabled = 1 AND changed = 1))
ORDER BY host COLLATE NOCASE, port
LIMIT ?3`
	sqlEndpointDue = "SELECT " + endpointColumns + ` FROM endpoint_status
WHERE enabled = 1 AND is_demo = 0 AND (observed_at = '' OR observed_at <= ?1)
ORDER BY observed_at, endpoint_id`
	sqlOrphans = `DELETE FROM certificates
WHERE source = 'observed'
  AND id NOT IN (SELECT certificate_id FROM observation_certificates)
  AND id NOT IN (SELECT leaf_certificate_id FROM observations WHERE leaf_certificate_id IS NOT NULL)
  AND id NOT IN (SELECT previous_certificate_id FROM observations WHERE previous_certificate_id IS NOT NULL)`
)

var EndpointFilters = []string{"all", "enabled", "disabled", "never", "unreachable", "chain", "hostname", "changed"}

var ErrNotFound = errors.New("no such entry; list the entries to see valid identifiers")

var ErrNotAuthorized = errors.New("confirm that you are authorized to test this host; CertWatch connects only to hosts you confirm")

type ValidationError struct {
	Field   string
	Problem string
	Fix     string
}

func (e *ValidationError) Error() string {
	return fmt.Sprintf("%s: %s. %s", e.Field, e.Problem, e.Fix)
}

type ConflictError struct {
	ID       int64
	Expected int64
	Actual   int64
}

func (e *ConflictError) Error() string {
	return fmt.Sprintf("endpoint %d was changed elsewhere (revision %d, expected %d); reload it and apply the change again", e.ID, e.Actual, e.Expected)
}

type EndpointInput struct {
	Host       string
	Port       int
	ServerName string
	Note       string
	Enabled    bool
}

type Endpoint struct {
	ID               int64
	Host             string
	Port             int
	ServerName       string
	Note             string
	Enabled          bool
	Demo             bool
	Revision         int64
	AuthorizedAt     time.Time
	CreatedAt        time.Time
	UpdatedAt        time.Time
	ObservationID    int64
	ObservedAt       time.Time
	Outcome          string
	Error            string
	CertificateID    int64
	ChainStatus      string
	ChainError       string
	HostnameStatus   string
	HostnameError    string
	Version          string
	CipherSuite      string
	Accepted         string
	ProtocolsTested  bool
	Changed          bool
	CertificateName  string
	CertificateIssue string
	NotAfter         time.Time
}

func (e Endpoint) Address() string {
	return net.JoinHostPort(e.Host, strconv.Itoa(e.Port))
}

func (e Endpoint) Input() EndpointInput {
	return EndpointInput{Host: e.Host, Port: e.Port, ServerName: e.ServerName, Note: e.Note, Enabled: e.Enabled}
}

func (e Endpoint) VerifiedName() string {
	if e.ServerName != "" {
		return e.ServerName
	}
	return e.Host
}

type Service struct {
	db  *sql.DB
	now func() time.Time
}

func New(db *sql.DB, now func() time.Time) *Service {
	return &Service{db: db, now: now}
}

type scanner interface {
	Scan(dest ...any) error
}

func parseTime(text string) time.Time {
	if text == "" {
		return time.Time{}
	}
	parsed, err := time.Parse(time.RFC3339, text)
	if err != nil {
		return time.Time{}
	}
	return parsed
}

func stamp(moment time.Time) string {
	return moment.UTC().Format(timeLayout)
}

func flag(value bool) int {
	if value {
		return 1
	}
	return 0
}

func scanEndpoint(row scanner) (Endpoint, error) {
	var e Endpoint
	var authorized, created, updated, observed, notAfter string
	err := row.Scan(&e.ID, &e.Host, &e.Port, &e.ServerName, &e.Note, &e.Enabled, &e.Demo, &e.Revision, &authorized, &created, &updated,
		&e.ObservationID, &observed, &e.Outcome, &e.Error, &e.CertificateID, &e.ChainStatus, &e.ChainError, &e.HostnameStatus, &e.HostnameError,
		&e.Version, &e.CipherSuite, &e.Accepted, &e.ProtocolsTested, &e.Changed, &e.CertificateName, &e.CertificateIssue, &notAfter)
	if errors.Is(err, sql.ErrNoRows) {
		return e, ErrNotFound
	}
	e.AuthorizedAt, e.CreatedAt, e.UpdatedAt = parseTime(authorized), parseTime(created), parseTime(updated)
	e.ObservedAt, e.NotAfter = parseTime(observed), parseTime(notAfter)
	return e, err
}

func validLabel(label string) bool {
	if label == "" || len(label) > 63 || label[0] == '-' || label[len(label)-1] == '-' {
		return false
	}
	for _, r := range label {
		if !(r >= 'a' && r <= 'z' || r >= '0' && r <= '9' || r == '-' || r == '_') {
			return false
		}
	}
	return true
}

func NormalizeHost(field, text string) (string, error) {
	host := strings.ToLower(strings.TrimSpace(text))
	host = strings.TrimSuffix(host, ".")
	if host == "" {
		return "", &ValidationError{Field: field, Problem: "the host name is empty", Fix: "Enter a host name such as www.example.org or an IP address"}
	}
	if ip := net.ParseIP(strings.Trim(host, "[]")); ip != nil {
		return ip.String(), nil
	}
	for _, r := range host {
		if r > 127 {
			return "", &ValidationError{Field: field, Problem: "the host name has characters outside ASCII", Fix: "Write internationalized names in their punycode form, which starts with xn--"}
		}
	}
	if len(host) > MaxHostLength {
		return "", &ValidationError{Field: field, Problem: fmt.Sprintf("the host name is longer than %d characters", MaxHostLength), Fix: "Check the name for pasted extra text"}
	}
	for _, label := range strings.Split(host, ".") {
		if !validLabel(label) {
			return "", &ValidationError{Field: field, Problem: fmt.Sprintf("%q is not a host name", text), Fix: "Use letters, digits, hyphens and dots, for example www.example.org; wildcards and paths are not host names"}
		}
	}
	return host, nil
}

func ParseTarget(text string) (string, int, error) {
	value := strings.TrimSpace(text)
	if index := strings.Index(value, "://"); index >= 0 {
		value = value[index+3:]
	}
	if index := strings.IndexAny(value, "/?#"); index >= 0 {
		value = value[:index]
	}
	port := 0
	hostPart := value
	if host, portText, err := net.SplitHostPort(value); err == nil {
		number, convErr := strconv.Atoi(portText)
		if convErr != nil || number < 1 || number > 65535 {
			return "", 0, &ValidationError{Field: "port", Problem: fmt.Sprintf("%q is not a port", portText), Fix: "Use a number from 1 to 65535, for example 443"}
		}
		hostPart, port = host, number
	} else if strings.Count(value, ":") == 1 {
		return "", 0, &ValidationError{Field: "port", Problem: fmt.Sprintf("%q has no valid port after the colon", text), Fix: "Use host:port with a port from 1 to 65535, for example www.example.org:443"}
	}
	host, err := NormalizeHost("host", hostPart)
	if err != nil {
		return "", 0, err
	}
	return host, port, nil
}

func Normalize(in EndpointInput) (EndpointInput, error) {
	host, port, err := ParseTarget(in.Host)
	if err != nil {
		return in, err
	}
	in.Host = host
	if port != 0 {
		in.Port = port
	}
	if in.Port == 0 {
		in.Port = DefaultPort
	}
	if in.Port < 1 || in.Port > 65535 {
		return in, &ValidationError{Field: "port", Problem: fmt.Sprintf("%d is not a port", in.Port), Fix: "Use a number from 1 to 65535, for example 443"}
	}
	if strings.TrimSpace(in.ServerName) != "" {
		name, err := NormalizeHost("server name", in.ServerName)
		if err != nil {
			return in, err
		}
		in.ServerName = name
		if name == in.Host {
			in.ServerName = ""
		}
	} else {
		in.ServerName = ""
	}
	in.Note = tlscheck.Clean(strings.Join(strings.Fields(in.Note), " "))
	if len(in.Note) > MaxNoteLength {
		return in, &ValidationError{Field: "note", Problem: fmt.Sprintf("the note is longer than %d characters", MaxNoteLength), Fix: "Shorten the note"}
	}
	return in, nil
}

func duplicate(err error, in EndpointInput) error {
	if store.IsConstraint(err) && strings.Contains(err.Error(), "endpoints.host") {
		return &ValidationError{Field: "host", Problem: fmt.Sprintf("%s is already monitored", net.JoinHostPort(in.Host, strconv.Itoa(in.Port))), Fix: "Edit the existing endpoint or use another port"}
	}
	return err
}

func (s *Service) inTx(ctx context.Context, work func(tx *sql.Tx) error) error {
	return store.Retry(ctx, 5, 40*time.Millisecond, func() error {
		tx, err := s.db.BeginTx(ctx, nil)
		if err != nil {
			return err
		}
		if err := work(tx); err != nil {
			_ = tx.Rollback()
			return err
		}
		return tx.Commit()
	})
}

func createEndpoint(ctx context.Context, tx *sql.Tx, now time.Time, in EndpointInput, demo bool) (Endpoint, error) {
	moment := stamp(now)
	result, err := tx.ExecContext(ctx, sqlEndpointInsert, in.Host, in.Port, in.ServerName, in.Note, flag(in.Enabled), flag(demo), moment, moment, moment)
	if err != nil {
		return Endpoint{}, duplicate(err, in)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return Endpoint{}, err
	}
	return scanEndpoint(tx.QueryRowContext(ctx, sqlEndpointGet, id))
}

func (s *Service) CreateEndpoint(ctx context.Context, in EndpointInput, authorized bool) (Endpoint, error) {
	in, err := Normalize(in)
	if err != nil {
		return Endpoint{}, err
	}
	if !authorized {
		return Endpoint{}, ErrNotAuthorized
	}
	var created Endpoint
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		var txErr error
		created, txErr = createEndpoint(ctx, tx, s.now(), in, false)
		return txErr
	})
	return created, err
}

func (s *Service) Endpoint(ctx context.Context, id int64) (Endpoint, error) {
	return scanEndpoint(s.db.QueryRowContext(ctx, sqlEndpointGet, id))
}

func (s *Service) EndpointByAddress(ctx context.Context, host string, port int) (Endpoint, error) {
	return scanEndpoint(s.db.QueryRowContext(ctx, sqlEndpointByAddress, host, port))
}

func (s *Service) UpdateEndpoint(ctx context.Context, id, revision int64, in EndpointInput, authorized bool) (Endpoint, error) {
	in, err := Normalize(in)
	if err != nil {
		return Endpoint{}, err
	}
	var updated Endpoint
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		current, err := scanEndpoint(tx.QueryRowContext(ctx, sqlEndpointGet, id))
		if err != nil {
			return err
		}
		if current.Revision != revision {
			return &ConflictError{ID: id, Expected: revision, Actual: current.Revision}
		}
		if (current.Host != in.Host || current.Port != in.Port) && !authorized {
			return ErrNotAuthorized
		}
		if _, err := tx.ExecContext(ctx, sqlEndpointUpdate, in.Host, in.Port, in.ServerName, in.Note, flag(in.Enabled), stamp(s.now()), id, revision); err != nil {
			return duplicate(err, in)
		}
		updated, err = scanEndpoint(tx.QueryRowContext(ctx, sqlEndpointGet, id))
		return err
	})
	return updated, err
}

func (s *Service) SetEndpointEnabled(ctx context.Context, id int64, enabled bool) (Endpoint, error) {
	var updated Endpoint
	err := s.inTx(ctx, func(tx *sql.Tx) error {
		if _, err := tx.ExecContext(ctx, sqlEndpointSetEnabled, flag(enabled), stamp(s.now()), id, flag(enabled)); err != nil {
			return err
		}
		var err error
		updated, err = scanEndpoint(tx.QueryRowContext(ctx, sqlEndpointGet, id))
		return err
	})
	return updated, err
}

func (s *Service) DeleteEndpoint(ctx context.Context, id int64) error {
	return s.inTx(ctx, func(tx *sql.Tx) error {
		result, err := tx.ExecContext(ctx, sqlEndpointDelete, id)
		if err != nil {
			return err
		}
		if count, _ := result.RowsAffected(); count == 0 {
			return ErrNotFound
		}
		_, err = tx.ExecContext(ctx, sqlOrphans)
		return err
	})
}

type Query struct {
	Search string
	Filter string
	Limit  int
}

func (q Query) limit() int {
	if q.Limit <= 0 || q.Limit > 5000 {
		return 5000
	}
	return q.Limit
}

func (q Query) search() string {
	return strings.ToLower(strings.TrimSpace(q.Search))
}

func pick(filter string, allowed []string) (string, error) {
	if filter == "" {
		return allowed[0], nil
	}
	for _, option := range allowed {
		if option == filter {
			return filter, nil
		}
	}
	return "", &ValidationError{Field: "filter", Problem: fmt.Sprintf("unknown filter %q", filter), Fix: "Use one of: " + strings.Join(allowed, ", ")}
}

func (s *Service) Endpoints(ctx context.Context, query Query) ([]Endpoint, error) {
	filter, err := pick(query.Filter, EndpointFilters)
	if err != nil {
		return nil, err
	}
	rows, err := s.db.QueryContext(ctx, sqlEndpointList, query.search(), filter, query.limit())
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	var list []Endpoint
	for rows.Next() {
		endpoint, err := scanEndpoint(rows)
		if err != nil {
			return nil, err
		}
		list = append(list, endpoint)
	}
	return list, rows.Err()
}

func (s *Service) DueEndpoints(ctx context.Context, interval time.Duration) ([]Endpoint, error) {
	rows, err := s.db.QueryContext(ctx, sqlEndpointDue, stamp(s.now().Add(-interval)))
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	var list []Endpoint
	for rows.Next() {
		endpoint, err := scanEndpoint(rows)
		if err != nil {
			return nil, err
		}
		list = append(list, endpoint)
	}
	return list, rows.Err()
}

func (s *Service) EndpointCount(ctx context.Context) (int, error) {
	var count int
	err := s.db.QueryRowContext(ctx, sqlEndpointCount).Scan(&count)
	return count, err
}
