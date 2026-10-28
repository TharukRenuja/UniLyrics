package unilyrics

import (
	"encoding/json"
	"os"
	"reflect"
	"testing"
)

func TestProfileProtocolFixtures(t *testing.T) {
	data, err := os.ReadFile("../../crates/unilyrics/tests/fixtures/profiles.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Name     string
		Request  json.RawMessage
		Expected json.RawMessage `json:"expected_document"`
		Output   json.RawMessage `json:"output_document"`
		Codes    []string
		Error    string
	}
	if err := json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	for _, c := range cases {
		t.Run(c.Name, func(t *testing.T) {
			before := string(c.Request)
			raw, err := Process(c.Request)
			if string(c.Request) != before {
				t.Fatal("request mutated")
			}
			if c.Error != "" {
				requireRegistryError(t, err, c.Error)
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			if c.Error == "" {
				var request struct {
					Op       string
					Document Document
					Options  ProjectionOptions
					Format   string
					Policy   string
				}
				if err := json.Unmarshal(c.Request, &request); err != nil {
					t.Fatal(err)
				}
				var helper any
				if request.Op == "project" {
					helper, err = Project(request.Document, request.Options)
				} else {
					helper, err = ExportProfile(request.Document, request.Format, request.Options, request.Policy)
				}
				if err != nil {
					t.Fatal(err)
				}
				encoded, err := json.Marshal(helper)
				if err != nil {
					t.Fatal(err)
				}
				var actual, expected any
				if err := json.Unmarshal(encoded, &actual); err != nil {
					t.Fatal(err)
				}
				if err := json.Unmarshal(raw, &expected); err != nil {
					t.Fatal(err)
				}
				if !reflect.DeepEqual(actual, expected) {
					t.Fatalf("helper and protocol differ: %s vs %s", encoded, raw)
				}
			}
			var result struct {
				Document    json.RawMessage
				Output      string
				Diagnostics []Diagnostic
				Losses      []Diagnostic
			}
			if err := json.Unmarshal(raw, &result); err != nil {
				t.Fatal(err)
			}
			actual, expected := result.Document, c.Expected
			codes := result.Diagnostics
			if len(expected) == 0 {
				actual, expected, codes = json.RawMessage(result.Output), c.Output, result.Losses
			}
			var actualValue, expectedValue any
			if err := json.Unmarshal(actual, &actualValue); err != nil {
				t.Fatal(err)
			}
			if err := json.Unmarshal(expected, &expectedValue); err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(actualValue, expectedValue) {
				t.Fatalf("expected %s, got %s", expected, actual)
			}
			actualCodes := make([]string, 0, len(codes))
			for _, d := range codes {
				actualCodes = append(actualCodes, d.Code)
			}
			if !reflect.DeepEqual(actualCodes, c.Codes) {
				t.Fatalf("expected %v, got %v", c.Codes, actualCodes)
			}
		})
	}
}

func TestProfileHelpers(t *testing.T) {
	parsed, err := Parse("[00:01]hello", "lrc")
	if err != nil {
		t.Fatal(err)
	}
	before, _ := json.Marshal(parsed.Document)
	full, err := Project(parsed.Document, ProjectionOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(full.Document, parsed.Document) {
		t.Fatal("default projection changed document")
	}
	if _, err := Project(parsed.Document, ProjectionOptions{Profile: ProfileCustom}); err == nil {
		t.Fatal("missing custom selection accepted")
	}
	options := ProjectionOptions{Profile: ProfileCustom, Features: []ProjectionFeature{}}
	plain, err := Project(parsed.Document, options)
	if err != nil {
		t.Fatal(err)
	}
	if plain.Document.Cues[0].Start != nil || len(plain.Diagnostics) == 0 {
		t.Fatal("empty selection did not remove timing")
	}
	exported, err := ExportProfile(parsed.Document, "json", options, "")
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := Parse(exported.Output, "json")
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decoded.Document, plain.Document) || !reflect.DeepEqual(exported.Losses, plain.Diagnostics) {
		t.Fatal("export projection mismatch")
	}
	_, err = ExportProfile(parsed.Document, "json", options, "strict")
	requireRegistryError(t, err, "LOSSY_CONVERSION")
	for _, invalid := range []ProjectionOptions{
		{Profile: Profile("unknown")},
		{Profile: ProfileFull, Features: []ProjectionFeature{}},
		{Profile: ProfileCustom, Features: []ProjectionFeature{FeatureAgents, FeatureAgents}},
		{Profile: ProfileCustom, Features: []ProjectionFeature{ProjectionFeature("unknown")}},
	} {
		if _, err := Project(parsed.Document, invalid); err == nil {
			t.Fatalf("invalid options accepted: %+v", invalid)
		}
	}
	after, _ := json.Marshal(parsed.Document)
	if string(before) != string(after) {
		t.Fatal("input mutated")
	}
}
