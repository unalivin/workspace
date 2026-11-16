package settings_test

import (
	"context"
	"errors"
	"strings"
	"testing"

	"github.com/unalivin/templateapp/internal/settings"
	"github.com/unalivin/templateapp/internal/testkit"
)

func newService(t *testing.T, env map[string]string, flags map[string]string) *settings.Service {
	t.Helper()
	opened := testkit.Open(t)
	clock := testkit.NewClock()
	return settings.New(opened.DB(), clock.Now, func(key string) string { return env[key] }, flags)
}

func TestDefinitionsHaveValidDefaults(t *testing.T) {
	seen := map[string]bool{}
	for _, definition := range settings.Definitions() {
		if seen[definition.Key] {
			t.Fatalf("duplicate key %s", definition.Key)
		}
		seen[definition.Key] = true
		if definition.Label == "" || definition.Help == "" || definition.Group == "" {
			t.Fatalf("%s lacks a label, help text or group", definition.Key)
		}
		normalized, err := settings.Normalize(definition, definition.Default)
		if err != nil || normalized != definition.Default {
			t.Fatalf("%s: default %q is not valid and normalised: %q, %v", definition.Key, definition.Default, normalized, err)
		}
	}
}

func TestNormalize(t *testing.T) {
	cases := []struct {
		key  string
		raw  string
		want string
		fix  string
	}{
		{settings.KeyMode, "DARK", "dark", ""},
		{settings.KeyMode, " light ", "light", ""},
		{settings.KeyMode, "sepia", "", "system, light, dark"},
		{settings.KeyMode, "", "", "Use one of"},
		{settings.KeyReducedMotion, "yes", "true", ""},
		{settings.KeyReducedMotion, "0", "false", ""},
		{settings.KeyReducedMotion, "maybe", "", "Use true or false"},
		{settings.KeyTextScale, "80", "80", ""},
		{settings.KeyTextScale, "200", "200", ""},
		{settings.KeyTextScale, "79", "", "from 80 to 200"},
		{settings.KeyTextScale, "201", "", "from 80 to 200"},
		{settings.KeyTextScale, "12.5", "", "from 80 to 200"},
		{settings.KeyTextScale, "99999999999999999999", "", "from 80 to 200"},
		{settings.KeyTextScale, "", "", "from 80 to 200"},
		{settings.KeyAccent, "#1257c4", "#1257C4", ""},
		{settings.KeyAccent, "", "", ""},
		{settings.KeyAccent, "blue", "", "#RRGGBB"},
		{settings.KeyAccent, "#12345", "", "#RRGGBB"},
		{settings.KeyAccent, "#1234567", "", "#RRGGBB"},
		{settings.KeyRecordColumns, "name,status,severity", "name,status,severity", ""},
		{settings.KeyRecordColumns, " Severity , id ,severity", "id,name,severity", ""},
		{settings.KeyRecordColumns, "", "name", ""},
		{settings.KeyRecordColumns, "name; DROP TABLE", "", "Use some of: id, name"},
		{settings.KeyRecordColumns, "name,owner", "", "Use some of: id, name"},
		{settings.KeyRecordColumns, strings.Repeat("a", 401), "", "Use some of: id, name"},
		{settings.KeyRecordSort, "name:desc", "name:desc", ""},
		{settings.KeyRecordSort, "Severity", "severity:asc", ""},
		{settings.KeyRecordSort, "updated : DESC", "updated:desc", ""},
		{settings.KeyRecordSort, "id:desc", "", "followed by :asc or :desc"},
		{settings.KeyRecordSort, "name:sideways", "", "followed by :asc or :desc"},
		{settings.KeyRecordSort, "", "", "followed by :asc or :desc"},
	}
	for _, tc := range cases {
		t.Run(tc.key+"="+tc.raw, func(t *testing.T) {
			definition, ok := settings.Lookup(tc.key)
			if !ok {
				t.Fatalf("unknown key %s", tc.key)
			}
			got, err := settings.Normalize(definition, tc.raw)
			if tc.fix == "" {
				if err != nil || got != tc.want {
					t.Fatalf("Normalize = %q, %v; want %q", got, err, tc.want)
				}
				return
			}
			var validation *settings.ValidationError
			if !errors.As(err, &validation) {
				t.Fatalf("expected a validation error, got %v", err)
			}
			if validation.Problem == "" || !strings.Contains(validation.Fix, tc.fix) {
				t.Fatalf("error lacks the problem or the fix: %+v", validation)
			}
		})
	}
}

func TestLookupRejectsUnknownAndMalformedKeys(t *testing.T) {
	for _, key := range []string{"", "nope", "table..columns", "table.records.width", "table.Records.columns", "table.other.columns", "appearance.mode "} {
		if _, ok := settings.Lookup(key); ok {
			t.Fatalf("Lookup(%q) should fail", key)
		}
	}
}

func TestSetGetResetRoundTrip(t *testing.T) {
	ctx := context.Background()
	service := newService(t, nil, nil)
	value, err := service.Get(ctx, settings.KeyTheme)
	if err != nil || value.Raw != "slate" || value.Source != settings.SourceDefault || value.HasStored {
		t.Fatalf("default lookup wrong: %+v, %v", value, err)
	}
	value, err = service.Set(ctx, settings.KeyTheme, "Purple")
	if err != nil || value.Raw != "purple" || value.Source != settings.SourceStored {
		t.Fatalf("set wrong: %+v, %v", value, err)
	}
	value, _ = service.Get(ctx, settings.KeyTheme)
	if value.Raw != "purple" || !value.HasStored {
		t.Fatalf("stored value not returned: %+v", value)
	}
	if _, err := service.Set(ctx, settings.KeyTheme, "neon"); err == nil {
		t.Fatal("invalid value was accepted")
	}
	value, _ = service.Get(ctx, settings.KeyTheme)
	if value.Raw != "purple" {
		t.Fatalf("a rejected value changed the setting: %+v", value)
	}
	value, err = service.Reset(ctx, settings.KeyTheme)
	if err != nil || value.Raw != "slate" || value.HasStored {
		t.Fatalf("reset wrong: %+v, %v", value, err)
	}
	if _, err := service.Set(ctx, "unknown.key", "x"); err == nil {
		t.Fatal("unknown key was accepted")
	}
	if _, err := service.Get(ctx, "unknown.key"); err == nil {
		t.Fatal("unknown key was returned")
	}
	if _, err := service.Reset(ctx, "unknown.key"); err == nil {
		t.Fatal("unknown key was reset")
	}
}

func TestOverridesAreReportedWithTheirSource(t *testing.T) {
	ctx := context.Background()
	env := map[string]string{
		settings.EnvName(settings.KeyTheme):     "green",
		settings.EnvName(settings.KeyDensity):   "compact",
		settings.EnvName(settings.KeyTextScale): "huge",
	}
	flags := map[string]string{settings.KeyDensity: "comfortable", settings.KeyMode: "neon"}
	service := newService(t, env, flags)
	if _, err := service.Set(ctx, settings.KeyTheme, "blue"); err != nil {
		t.Fatal(err)
	}
	theme, _ := service.Get(ctx, settings.KeyTheme)
	if theme.Raw != "green" || theme.Source != settings.SourceEnvironment || theme.Stored != "blue" || !strings.Contains(theme.OverriddenBy, settings.EnvName(settings.KeyTheme)) {
		t.Fatalf("environment override not reported: %+v", theme)
	}
	density, _ := service.Get(ctx, settings.KeyDensity)
	if density.Raw != "comfortable" || density.Source != settings.SourceFlag || !strings.Contains(density.OverriddenBy, "--set "+settings.KeyDensity) {
		t.Fatalf("flag must win over the environment: %+v", density)
	}
	scale, _ := service.Get(ctx, settings.KeyTextScale)
	if scale.Raw != "100" || scale.Source != settings.SourceDefault || !strings.Contains(scale.OverrideError, "ignored") {
		t.Fatalf("invalid environment override must be ignored and reported: %+v", scale)
	}
	mode, _ := service.Get(ctx, settings.KeyMode)
	if mode.Raw != "system" || !strings.Contains(mode.OverrideError, "ignored") {
		t.Fatalf("invalid flag override must be ignored and reported: %+v", mode)
	}
}

func TestListKeepsUnknownAndInvalidStoredValues(t *testing.T) {
	ctx := context.Background()
	opened := testkit.Open(t)
	clock := testkit.NewClock()
	service := settings.New(opened.DB(), clock.Now, nil, nil)
	insert := "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, 'now')"
	for key, value := range map[string]string{"future.option": "42", settings.KeyMode: "sepia", settings.KeyRecordColumns: "name,status"} {
		if _, err := opened.DB().ExecContext(ctx, insert, key, value); err != nil {
			t.Fatal(err)
		}
	}
	values, err := service.List(ctx)
	if err != nil {
		t.Fatal(err)
	}
	byKey := map[string]settings.Value{}
	for _, value := range values {
		byKey[value.Key] = value
	}
	if len(values) != len(settings.Definitions())+1 {
		t.Fatalf("expected every definition plus one stored extra, got %d", len(values))
	}
	if future := byKey["future.option"]; !future.Unknown || future.Raw != "42" {
		t.Fatalf("unknown stored key was dropped or altered: %+v", future)
	}
	if mode := byKey[settings.KeyMode]; mode.Raw != "system" || mode.Stored != "sepia" || mode.OverrideError == "" {
		t.Fatalf("invalid stored value must fall back to the default and stay visible: %+v", mode)
	}
	if table := byKey[settings.KeyRecordColumns]; table.Raw != "name,status" || table.Unknown || !table.Shows("status") || table.Shows("id") {
		t.Fatalf("table preference not listed: %+v", table)
	}
	if key, descending := byKey[settings.KeyRecordSort].Sort(); key != "updated" || !descending {
		t.Fatalf("default sort is %s descending=%v", key, descending)
	}
	if settings.JoinSort("name", false) != "name:asc" || settings.JoinSort("name", true) != "name:desc" {
		t.Fatal("sort values are not joined as column:direction")
	}
	effective, err := service.Effective(ctx)
	if err != nil || effective[settings.KeyTextScale].Int() != 100 || effective[settings.KeyConfirm].Bool() != true {
		t.Fatalf("effective values wrong: %v", err)
	}
}

func FuzzNormalize(f *testing.F) {
	for _, seed := range []string{"", "dark", "true", "100", "#FFFFFF", "a,b", "id,name", "name:desc", "\x00", strings.Repeat("9", 400)} {
		f.Add(seed)
	}
	all := settings.Definitions()
	f.Fuzz(func(t *testing.T, raw string) {
		for _, definition := range all {
			normalized, err := settings.Normalize(definition, raw)
			if err != nil {
				continue
			}
			again, err := settings.Normalize(definition, normalized)
			if err != nil || again != normalized {
				t.Fatalf("%s: normalisation of %q is not stable: %q then %q (%v)", definition.Key, raw, normalized, again, err)
			}
			if len(normalized) > 4096 {
				t.Fatalf("%s: accepted an oversized value", definition.Key)
			}
		}
	})
}
