package unilyrics

import (
	"encoding/json"
	"errors"
	"sort"
	"strings"
	"sync"
	"unicode/utf8"
)

// Adapter contains synchronous application callbacks. Nil callbacks are unsupported operations.
// Callbacks can run concurrently and are responsible for their own captured state.
type Adapter struct {
	Matches func(string) bool
	Parse   func(string) (ParseResult, error)
	Export  func(Document, string) (ExportResult, error)
}

// Registry scopes custom formats without modifying built-ins. Its zero value is ready to use.
// Do not copy a Registry after first use. Registration and lookups are safe concurrently.
type Registry struct {
	mu      sync.RWMutex
	formats map[string]Adapter
}

func (r *Registry) Register(name string, adapter Adapter) error {
	if len(name) < 3 || len(name) > 64 || !strings.HasPrefix(name, "x-") {
		return &Error{Code: "INVALID_CUSTOM_FORMAT", Message: "Use a 3–64 character lowercase ASCII name in the x- namespace"}
	}
	for _, c := range []byte(name) {
		if !(c >= 'a' && c <= 'z' || c >= '0' && c <= '9' || c == '-' || c == '_') {
			return &Error{Code: "INVALID_CUSTOM_FORMAT", Message: "Custom names allow lowercase ASCII letters, digits, - and _"}
		}
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	if _, exists := r.formats[name]; exists {
		return &Error{Code: "DUPLICATE_FORMAT", Message: "Custom format is already registered"}
	}
	if r.formats == nil {
		r.formats = make(map[string]Adapter)
	}
	r.formats[name] = adapter
	return nil
}

func builtinFormat(name string) bool {
	switch name {
	case "plain", "lrc", "enhanced_lrc", "json", "ttml", "ttml_lyrics", "srt", "webvtt":
		return true
	}
	return false
}

func checkRegistryInput(input string) error {
	if !utf8.ValidString(input) {
		return &Error{Code: "INVALID_UTF8", Message: "Input must be UTF-8"}
	}
	if len(input) > 1048576 {
		return &Error{Code: "INPUT_TOO_LARGE", Message: "Input exceeds 1 MiB"}
	}
	return nil
}

func (r *Registry) adapter(name string) (Adapter, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	adapter, found := r.formats[name]
	return adapter, found
}

func (r *Registry) Detect(input string) (string, error) {
	if err := checkRegistryInput(input); err != nil {
		return "", err
	}
	builtin, builtinErr := Detect(input)
	var native *Error
	if builtinErr != nil && !errors.As(builtinErr, &native) {
		return "", builtinErr
	}
	type entry struct {
		name    string
		adapter Adapter
	}
	r.mu.RLock()
	snapshot := make([]entry, 0, len(r.formats))
	for name, adapter := range r.formats {
		snapshot = append(snapshot, entry{name, adapter})
	}
	r.mu.RUnlock()
	sort.Slice(snapshot, func(i, j int) bool { return snapshot[i].name < snapshot[j].name })
	matches := []string{}
	for _, item := range snapshot {
		if item.adapter.Matches != nil && item.adapter.Matches(input) {
			matches = append(matches, item.name)
		}
	}
	if len(matches) > 1 || len(matches) == 1 && builtinErr == nil && builtin != "plain" {
		return "", &Error{Code: "AMBIGUOUS_FORMAT", Message: "Multiple formats match; choose explicitly"}
	}
	if len(matches) == 1 {
		return matches[0], nil
	}
	return builtin, builtinErr
}

func checkedCustomParse(result ParseResult) (ParseResult, error) {
	if result.Diagnostics == nil {
		result.Diagnostics = []Diagnostic{}
	}
	raw, err := Process(map[string]any{"op": "validate_custom_parse", "result": result})
	if err != nil {
		return ParseResult{}, err
	}
	var checked ParseResult
	err = json.Unmarshal(raw, &checked)
	return checked, err
}

// Parse uses built-in detection when format is empty, plus this registry's detectors.
func (r *Registry) Parse(input, format string) (ParseResult, error) {
	if err := checkRegistryInput(input); err != nil {
		return ParseResult{}, err
	}
	if format == "" {
		var err error
		format, err = r.Detect(input)
		if err != nil {
			return ParseResult{}, err
		}
	}
	adapter, found := r.adapter(format)
	if !found {
		if builtinFormat(format) {
			return Parse(input, format)
		}
		return ParseResult{}, &Error{Code: "UNKNOWN_FORMAT", Message: "Format is not registered"}
	}
	if adapter.Parse == nil {
		return ParseResult{}, &Error{Code: "UNSUPPORTED_OPERATION", Message: "Custom format has no parser"}
	}
	result, err := adapter.Parse(input)
	if err != nil {
		return ParseResult{}, err
	}
	return checkedCustomParse(result)
}

// Export gives callbacks an independent canonical document and verifies their reparsed output.
func (r *Registry) Export(document Document, format, policy string) (ExportResult, error) {
	adapter, found := r.adapter(format)
	if !found {
		if builtinFormat(format) {
			return Export(document, format, policy)
		}
		return ExportResult{}, &Error{Code: "UNKNOWN_FORMAT", Message: "Format is not registered"}
	}
	if policy == "" {
		policy = "compatible"
	}
	if policy != "compatible" && policy != "strict" {
		return ExportResult{}, &Error{Code: "INVALID_POLICY", Message: "Unknown export policy"}
	}
	canonical, err := Export(document, "json", "strict")
	if err != nil {
		return ExportResult{}, err
	}
	if adapter.Export == nil {
		return ExportResult{}, &Error{Code: "UNSUPPORTED_OPERATION", Message: "Custom format has no exporter"}
	}
	var callbackDocument Document
	if err := json.Unmarshal([]byte(canonical.Output), &callbackDocument); err != nil {
		return ExportResult{}, err
	}
	result, err := adapter.Export(callbackDocument, policy)
	if err != nil {
		return ExportResult{}, err
	}
	if result.Losses == nil {
		result.Losses = []Diagnostic{}
	}
	raw, err := Process(map[string]any{"op": "validate_custom_export", "result": result})
	if err != nil {
		return ExportResult{}, err
	}
	var checked ExportResult
	if err := json.Unmarshal(raw, &checked); err != nil {
		return ExportResult{}, err
	}
	var roundtrip *Document
	if adapter.Parse != nil {
		if parsed, err := adapter.Parse(checked.Output); err == nil {
			if parsed, err := checkedCustomParse(parsed); err == nil {
				roundtrip = &parsed.Document
			}
		}
	}
	raw, err = Process(map[string]any{"op": "verify_custom_export", "document": json.RawMessage(canonical.Output), "result": checked, "roundtrip": roundtrip, "policy": policy})
	if err != nil {
		return ExportResult{}, err
	}
	var verified ExportResult
	err = json.Unmarshal(raw, &verified)
	return verified, err
}
