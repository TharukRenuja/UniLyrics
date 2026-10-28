package unilyrics

import (
	"encoding/json"
	"os"
	"reflect"
	"strings"
	"testing"
)

func TestPreservationProfileFixtures(t *testing.T) {
	data, err := os.ReadFile("../../crates/unilyrics/tests/fixtures/profiles.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Name    string
		Error   string
		Request struct {
			Op       string
			Document Document
			Options  ProjectionOptions
		}
		Expected Document `json:"expected_document"`
		Codes    []string
	}
	if err := json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	for _, c := range cases {
		if c.Error != "" || c.Request.Op != "project" {
			continue
		}
		t.Run(c.Name, func(t *testing.T) {
			before, _ := json.Marshal(c.Request.Document)
			bundle, err := ExportPreserved(c.Request.Document, "json", c.Request.Options)
			if err != nil {
				t.Fatal(err)
			}
			parsed, err := Parse(bundle.Output, "json")
			if err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(parsed.Document, c.Expected) {
				t.Fatal("projected output differs")
			}
			codes := make([]string, 0, len(bundle.Losses))
			for _, d := range bundle.Losses {
				codes = append(codes, d.Code)
			}
			if !reflect.DeepEqual(codes, c.Codes) {
				t.Fatalf("loss codes: %v vs %v", codes, c.Codes)
			}
			// Save/reload the sidecar before restoring; do not rely on shared Go pointers.
			encoded, err := json.Marshal(bundle.Sidecar)
			if err != nil {
				t.Fatal(err)
			}
			var sidecar PreservationSidecar
			if err := json.Unmarshal(encoded, &sidecar); err != nil {
				t.Fatal(err)
			}
			restored, err := RestorePreserved(bundle.Output, "json", sidecar)
			if err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(restored.Document, c.Request.Document) {
				t.Fatal("snapshot restoration differs")
			}
			_, err = RestorePreserved(bundle.Output+"\n", "json", sidecar)
			requireRegistryError(t, err, "SIDECAR_MISMATCH")
			_, err = RestorePreserved(bundle.Output, "plain", sidecar)
			requireRegistryError(t, err, "SIDECAR_MISMATCH")
			sidecar.Version = 2
			_, err = RestorePreserved(bundle.Output, "json", sidecar)
			requireRegistryError(t, err, "UNSUPPORTED_SIDECAR_VERSION")
			after, _ := json.Marshal(c.Request.Document)
			if string(before) != string(after) {
				t.Fatal("input mutated")
			}
		})
	}
}

func TestPreservationLossyPlainAndInvalidSnapshot(t *testing.T) {
	parsed, err := Parse("[00:01]සිංහල 👩🏽‍💻", "lrc")
	if err != nil {
		t.Fatal(err)
	}
	bundle, err := ExportPreserved(parsed.Document, "plain", ProjectionOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if len(bundle.Losses) == 0 {
		t.Fatal("missing output loss report")
	}
	restored, err := RestorePreserved(bundle.Output, "plain", bundle.Sidecar)
	if err != nil || !reflect.DeepEqual(restored.Document, parsed.Document) {
		t.Fatalf("restore: %v", err)
	}
	if len(restored.Diagnostics) != 1 || restored.Diagnostics[0].Code != "SIDECAR_RESTORED" {
		t.Fatal("missing restoration diagnostic")
	}
	bundle.Sidecar.Document.Version = 99
	if _, err := RestorePreserved(bundle.Output, "plain", bundle.Sidecar); err == nil {
		t.Fatal("invalid model accepted")
	}
	if _, err := ExportPreserved(parsed.Document, "unknown", ProjectionOptions{}); err == nil {
		t.Fatal("invalid format accepted")
	}
}

func TestPreservationExportRejectsUnrestorableEncodedBundle(t *testing.T) {
	parsed, err := Parse(strings.Repeat("<", 100_000), "plain")
	if err != nil {
		t.Fatal(err)
	}
	_, err = ExportPreserved(parsed.Document, "plain", ProjectionOptions{})
	requireRegistryError(t, err, "INPUT_TOO_LARGE")
	parsed, err = Parse(strings.Repeat("<", 40_000), "plain")
	if err != nil {
		t.Fatal(err)
	}
	bundle, err := ExportPreserved(parsed.Document, "plain", ProjectionOptions{})
	if err != nil {
		t.Fatal(err)
	}
	restored, err := RestorePreserved(bundle.Output, "plain", bundle.Sidecar)
	if err != nil || !reflect.DeepEqual(restored.Document, parsed.Document) {
		t.Fatalf("valid smaller bundle: %v", err)
	}
}
