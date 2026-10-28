package unilyrics

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"reflect"
	"strings"
	"sync"
	"testing"
)

func wrappedJSONAdapter() Adapter {
	return Adapter{
		Matches: func(input string) bool { return strings.HasPrefix(input, "CUSTOM:") },
		Parse: func(input string) (ParseResult, error) {
			input, ok := strings.CutPrefix(input, "CUSTOM:")
			if !ok {
				return ParseResult{}, &Error{Code: "BAD_HEADER", Message: "Missing CUSTOM prefix"}
			}
			return Parse(input, "json")
		},
		Export: func(document Document, policy string) (ExportResult, error) {
			result, err := Export(document, "json", policy)
			result.Output = "CUSTOM:" + result.Output
			return result, err
		},
	}
}

func requireRegistryError(t *testing.T, err error, code string) {
	t.Helper()
	var native *Error
	if !errors.As(err, &native) || native.Code != code {
		t.Fatalf("expected %s, got %v", code, err)
	}
}

func TestRegistrySharedCorpusAndBuiltins(t *testing.T) {
	data, err := os.ReadFile("../../crates/unilyrics/tests/fixtures/conformance.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Name, Format, Error string
		Input               *string
		Document            json.RawMessage
	}
	if err := json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	var registry Registry
	if err := registry.Register("x-wrapped", wrappedJSONAdapter()); err != nil {
		t.Fatal(err)
	}
	for _, c := range cases {
		if c.Error != "" {
			continue
		}
		t.Run(c.Name, func(t *testing.T) {
			input := string(c.Document)
			if c.Input != nil {
				input = *c.Input
			}
			direct, err := Parse(input, c.Format)
			if err != nil {
				t.Fatal(err)
			}
			through, err := registry.Parse(input, c.Format)
			if err != nil || !reflect.DeepEqual(through, direct) {
				t.Fatalf("builtin dispatch: %v", err)
			}
			before, _ := json.Marshal(direct.Document)
			encoded, err := registry.Export(direct.Document, "x-wrapped", "strict")
			if err != nil || len(encoded.Losses) != 0 {
				t.Fatalf("export: %+v, %v", encoded, err)
			}
			name, err := registry.Detect(encoded.Output)
			if err != nil || name != "x-wrapped" {
				t.Fatalf("detect: %s, %v", name, err)
			}
			parsed, err := registry.Parse(encoded.Output, "")
			if err != nil || !reflect.DeepEqual(parsed.Document, direct.Document) {
				t.Fatalf("round trip: %v", err)
			}
			after, _ := json.Marshal(direct.Document)
			if string(before) != string(after) {
				t.Fatal("original mutated")
			}
		})
	}
}

func TestRegistryValidationAndAmbiguity(t *testing.T) {
	var registry Registry
	for _, name := range []string{"plain", "x-", "X-custom", "x-💻", "x-ok\n", "x-" + strings.Repeat("a", 63)} {
		requireRegistryError(t, registry.Register(name, Adapter{}), "INVALID_CUSTOM_FORMAT")
	}
	if err := registry.Register("x-one", wrappedJSONAdapter()); err != nil {
		t.Fatal(err)
	}
	requireRegistryError(t, registry.Register("x-one", Adapter{}), "DUPLICATE_FORMAT")
	var other Registry
	_, err := other.Parse("CUSTOM:{}", "x-one")
	requireRegistryError(t, err, "UNKNOWN_FORMAT")
	_, err = other.Parse("hi", "not-registered")
	requireRegistryError(t, err, "UNKNOWN_FORMAT")
	if err := registry.Register("x-two", wrappedJSONAdapter()); err != nil {
		t.Fatal(err)
	}
	_, err = registry.Detect("CUSTOM:{}")
	requireRegistryError(t, err, "AMBIGUOUS_FORMAT")
	var broad Registry
	if err := broad.Register("x-broad", Adapter{Matches: func(string) bool { return true }}); err != nil {
		t.Fatal(err)
	}
	_, err = broad.Detect("[00:01]hi")
	requireRegistryError(t, err, "AMBIGUOUS_FORMAT")
	if _, err = broad.Parse("[00:01]hi", "lrc"); err != nil {
		t.Fatal(err)
	}
	_, err = broad.Parse("hi", "x-broad")
	requireRegistryError(t, err, "UNSUPPORTED_OPERATION")
	_, err = broad.Parse(string([]byte{255}), "x-broad")
	requireRegistryError(t, err, "INVALID_UTF8")
	_, err = broad.Parse(strings.Repeat("x", 1048577), "x-broad")
	requireRegistryError(t, err, "INPUT_TOO_LARGE")
	bad, _ := Parse("hi", "plain")
	bad.Document.Version = 999
	if err := broad.Register("x-invalid", Adapter{Parse: func(string) (ParseResult, error) { return bad, nil }}); err != nil {
		t.Fatal(err)
	}
	_, err = broad.Parse("hi", "x-invalid")
	requireRegistryError(t, err, "UNSUPPORTED_VERSION")
	good, err := Parse("hi", "plain")
	if err != nil {
		t.Fatal(err)
	}
	if err := broad.Register("x-nil-diagnostics", Adapter{Parse: func(string) (ParseResult, error) {
		return ParseResult{Document: good.Document}, nil
	}}); err != nil {
		t.Fatal(err)
	}
	parsed, err := broad.Parse("hi", "x-nil-diagnostics")
	if err != nil || parsed.Diagnostics == nil || len(parsed.Diagnostics) != 0 {
		t.Fatalf("nil diagnostics: %+v, %v", parsed, err)
	}

}

func TestRegistryMutationLossesAndCallbackErrors(t *testing.T) {
	original, err := Parse("සිංහල 👩🏽‍💻", "plain")
	if err != nil {
		t.Fatal(err)
	}
	original.Document.Attributes["nested"] = json.RawMessage(`{"value":1}`)
	before, _ := json.Marshal(original.Document)
	wrapped := wrappedJSONAdapter()
	var registry Registry
	mutation := wrapped
	mutation.Export = func(document Document, policy string) (ExportResult, error) {
		document.Cues[0].Segments[0].Text = "changed"
		document.Cues[0].Attributes["mutated"] = json.RawMessage("true")
		nested := document.Attributes["nested"]
		nested[len(nested)-2] = '2'

		return wrapped.Export(document, policy)
	}
	if err := registry.Register("x-mutate", mutation); err != nil {
		t.Fatal(err)
	}
	changed, err := registry.Export(original.Document, "x-mutate", "compatible")
	if err != nil || len(changed.Losses) != 1 || changed.Losses[0].Code != "CUSTOM_ROUNDTRIP_CHANGED" {
		t.Fatalf("loss: %+v, %v", changed, err)
	}
	_, err = registry.Export(original.Document, "x-mutate", "strict")
	requireRegistryError(t, err, "LOSSY_CONVERSION")
	after, _ := json.Marshal(original.Document)
	if string(before) != string(after) {
		t.Fatal("callback changed original slices/maps")
	}
	output := Adapter{Export: func(Document, string) (ExportResult, error) { return ExportResult{Output: "opaque"}, nil }}
	if err := registry.Register("x-output", output); err != nil {
		t.Fatal(err)
	}
	unverified, err := registry.Export(original.Document, "x-output", "")
	if err != nil || unverified.Losses[0].Code != "CUSTOM_ROUNDTRIP_UNVERIFIED" {
		t.Fatalf("unverified: %+v, %v", unverified, err)
	}
	_, err = registry.Export(original.Document, "x-output", "strict")
	requireRegistryError(t, err, "LOSSY_CONVERSION")
	sentinel := errors.New("application parse failed")
	if err := registry.Register("x-error", Adapter{Parse: func(string) (ParseResult, error) { return ParseResult{}, sentinel }, Export: output.Export}); err != nil {
		t.Fatal(err)
	}
	_, err = registry.Parse("hi", "x-error")
	if err != sentinel {
		t.Fatal("application error changed")
	}
	unverified, err = registry.Export(original.Document, "x-error", "")
	if err != nil || unverified.Losses[0].Code != "CUSTOM_ROUNDTRIP_UNVERIFIED" {
		t.Fatalf("reparse failure: %+v, %v", unverified, err)
	}
	huge := wrapped
	huge.Export = func(Document, string) (ExportResult, error) {
		return ExportResult{Output: strings.Repeat("x", 1048577)}, nil
	}
	huge.Parse = func(string) (ParseResult, error) {
		t.Fatal("must not parse oversized output")
		return ParseResult{}, nil
	}
	if err := registry.Register("x-large", huge); err != nil {
		t.Fatal(err)
	}
	_, err = registry.Export(original.Document, "x-large", "")
	requireRegistryError(t, err, "INPUT_TOO_LARGE")
	_, err = registry.Export(original.Document, "x-output", "invented")
	requireRegistryError(t, err, "INVALID_POLICY")
}

func TestRegistryExportResultSnapshot(t *testing.T) {
	original, err := Parse("hi", "plain")
	if err != nil {
		t.Fatal(err)
	}
	wrapped := wrappedJSONAdapter()
	reported := []Diagnostic{{Code: "ORIGINAL_LOSS", Message: "reported"}}
	var registry Registry
	if err := registry.Register("x-snapshot", Adapter{
		Parse: func(input string) (ParseResult, error) {
			reported[0].Code = "MUTATED_LOSS"
			return wrapped.Parse(input)
		},
		Export: func(document Document, policy string) (ExportResult, error) {
			result, err := wrapped.Export(document, policy)
			result.Losses = reported
			return result, err
		},
	}); err != nil {
		t.Fatal(err)
	}
	result, err := registry.Export(original.Document, "x-snapshot", "compatible")
	if err != nil || result.Losses[0].Code != "ORIGINAL_LOSS" {
		t.Fatalf("callback-owned slice leaked: %+v, %v", result, err)
	}
}

func TestRegistryConcurrentRegistrationAndCallbacks(t *testing.T) {
	var registry Registry
	adapter := wrappedJSONAdapter()
	if err := registry.Register("x-wrapped", adapter); err != nil {
		t.Fatal(err)
	}
	canonical, _ := Parse("hi", "plain")
	jsonText, _ := Export(canonical.Document, "json", "strict")
	input := "CUSTOM:" + jsonText.Output
	var group sync.WaitGroup
	for i := 0; i < 8; i++ {
		group.Add(1)
		go func(index int) {
			defer group.Done()
			if err := registry.Register(fmt.Sprintf("x-extra-%d", index), Adapter{}); err != nil {
				t.Error(err)
			}
			for repeat := 0; repeat < 10; repeat++ {
				if _, err := registry.Parse(input, ""); err != nil {
					t.Error(err)
				}
				if _, err := registry.Export(canonical.Document, "x-wrapped", "strict"); err != nil {
					t.Error(err)
				}
			}
		}(i)
	}
	group.Wait()
	if err := registry.Register("x-reentrant", Adapter{Parse: func(string) (ParseResult, error) {
		return canonical, registry.Register("x-inside-callback", Adapter{})
	}}); err != nil {
		t.Fatal(err)
	}
	if _, err := registry.Parse("hi", "x-reentrant"); err != nil {
		t.Fatal(err)
	}
}
