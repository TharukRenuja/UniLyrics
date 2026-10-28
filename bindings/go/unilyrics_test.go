package unilyrics

import (
	"encoding/json"
	"os"
	"reflect"
	"testing"
)

func TestConformance(t *testing.T) {
	data, err := os.ReadFile("../../crates/unilyrics/tests/fixtures/conformance.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Name, Format, Error string
		Input               *string
		Text                []string
		Starts              []*uint64
		Document            json.RawMessage
		Features            *Features
		LossCodes           []string `json:"loss_codes"`
		Exports             []struct{ Format, Policy, Output string }
	}
	if err := json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	for _, c := range cases {
		t.Run(c.Name, func(t *testing.T) {
			input := string(c.Document)
			if c.Input != nil {
				input = *c.Input
			}
			result, err := Parse(input, c.Format)
			if c.Error != "" {
				if e, ok := err.(*Error); !ok || e.Code != c.Error {
					t.Fatalf("expected %s, got %v", c.Error, err)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			for _, expected := range c.Exports {
				encoded, err := Export(result.Document, expected.Format, expected.Policy)
				if err != nil || encoded.Output != expected.Output || len(encoded.Losses) != 0 {
					t.Fatalf("export fixture: %+v %v", encoded, err)
				}
			}
			if len(result.Document.Cues) != len(c.Text) {
				t.Fatal("cue count")
			}
			for i, cue := range result.Document.Cues {
				text := ""
				for _, s := range cue.Segments {
					text += s.Text
				}
				if text != c.Text[i] {
					t.Fatal("text", text)
				}
				a, b := cue.Start, c.Starts[i]
				if (a == nil) != (b == nil) || (a != nil && *a != *b) {
					t.Fatal("time")
				}
			}
			output, err := Export(result.Document, "json", "strict")
			if err != nil {
				t.Fatal(err)
			}
			if c.Features != nil {
				var expected, actual any
				if err := json.Unmarshal(c.Document, &expected); err != nil {
					t.Fatal(err)
				}
				if err := json.Unmarshal([]byte(output.Output), &actual); err != nil {
					t.Fatal(err)
				}
				if !reflect.DeepEqual(expected, actual) {
					t.Fatal("rich document changed", output.Output)
				}
				features, err := Inspect(result.Document)
				if err != nil || !reflect.DeepEqual(features, *c.Features) {
					t.Fatalf("features: %+v %v", features, err)
				}
				if err := Validate(result.Document); err != nil {
					t.Fatal(err)
				}
				plain, err := Export(result.Document, "plain", "compatible")
				if err != nil {
					t.Fatal(err)
				}
				codes := map[string]bool{}
				for _, loss := range plain.Losses {
					codes[loss.Code] = true
				}
				if len(codes) != len(c.LossCodes) {
					t.Fatal("unexpected losses", codes)
				}
				for _, code := range c.LossCodes {
					if !codes[code] {
						t.Fatal("missing loss", code)
					}
				}
				if _, err := Export(result.Document, "plain", "strict"); err == nil {
					t.Fatal("missing strict loss error")
				}
			}
			if _, err := Parse(output.Output, "json"); err != nil {
				t.Fatal(err)
			}
		})
	}
}

func TestInvalidUTF8(t *testing.T) {
	_, err := Parse(string([]byte{255}), "plain")
	if e, ok := err.(*Error); !ok || e.Code != "INVALID_UTF8" {
		t.Fatalf("invalid UTF-8 must not be replaced: %v", err)
	}
}

func TestInvalidUTF8DocumentBoundary(t *testing.T) {
	parsed, err := Parse("hello", "plain")
	if err != nil {
		t.Fatal(err)
	}
	bad := string([]byte{255})
	parsed.Document.Cues[0].Segments[0].Text = bad
	for name, operation := range map[string]func() error{
		"export":   func() error { _, err := Export(parsed.Document, "json", "strict"); return err },
		"validate": func() error { return Validate(parsed.Document) },
		"inspect":  func() error { _, err := Inspect(parsed.Document); return err },
		"process": func() error {
			_, err := Process(map[string]any{"op": "parse", "input": bad, "format": "plain"})
			return err
		},
	} {
		t.Run(name, func(t *testing.T) {
			if e, ok := operation().(*Error); !ok || e.Code != "INVALID_UTF8" {
				t.Fatalf("invalid UTF-8 must not be replaced: %v", e)
			}
		})
	}
	parsed.Document.Cues[0].Segments[0].Text = "\ufffd"
	parsed.Document.Attributes[bad] = json.RawMessage("null")
	if _, err := Export(parsed.Document, "json", "strict"); err == nil {
		t.Fatal("invalid attribute key was replaced")
	}
	delete(parsed.Document.Attributes, bad)
	if _, err := Export(parsed.Document, "json", "strict"); err != nil {
		t.Fatalf("a valid replacement character was rejected: %v", err)
	}
}

func TestConvenienceOperations(t *testing.T) {
	format, err := Detect("[00:01]hello")
	if err != nil || format != "lrc" {
		t.Fatalf("detect: %s %v", format, err)
	}
	parsed, err := Parse("hello", "plain")
	if err != nil {
		t.Fatal(err)
	}
	if err := Validate(parsed.Document); err != nil {
		t.Fatal(err)
	}
	parsed.Document.Version = 99
	if err := Validate(parsed.Document); err == nil {
		t.Fatal("invalid version accepted")
	}
}

func TestIgnoredJSONCyclesDoNotLoop(t *testing.T) {
	type A struct{ Clash any }
	type B struct{ Clash any }
	request := struct {
		A
		B
		Op     string `json:"op"`
		Input  string `json:"input"`
		Format string `json:"format"`
	}{Op: "parse", Input: "safe", Format: "plain"}
	request.A.Clash = &request
	request.B.Clash = &request
	if _, err := Process(request); err != nil {
		t.Fatal(err)
	}
}

func TestNormalizationFixtures(t *testing.T) {
	data, err := os.ReadFile("../../crates/unilyrics/tests/fixtures/normalization.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Name     string
		Document Document
		Offset   int64           `json:"offset_us"`
		Expected json.RawMessage `json:"expected_document"`
		Codes    []string        `json:"diagnostic_codes"`
		Error    string
	}
	if err := json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	for _, c := range cases {
		t.Run(c.Name, func(t *testing.T) {
			before, _ := json.Marshal(c.Document)
			result, err := Normalize(c.Document, c.Offset)
			after, _ := json.Marshal(c.Document)
			if string(before) != string(after) {
				t.Fatal("input mutated")
			}
			if c.Error != "" {
				native, ok := err.(*Error)
				if !ok || native.Code != c.Error {
					t.Fatalf("expected %s, got %v", c.Error, err)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			actual, _ := json.Marshal(result.Document)
			var actualValue, expectedValue any
			if err := json.Unmarshal(actual, &actualValue); err != nil {
				t.Fatal(err)
			}
			if err := json.Unmarshal(c.Expected, &expectedValue); err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(actualValue, expectedValue) {
				t.Fatalf("expected %s, got %s", c.Expected, actual)
			}
			codes := make([]string, 0, len(result.Diagnostics))
			for _, d := range result.Diagnostics {
				codes = append(codes, d.Code)
			}
			if !reflect.DeepEqual(codes, c.Codes) {
				t.Fatalf("expected %v, got %v", c.Codes, codes)
			}
		})
	}
}
