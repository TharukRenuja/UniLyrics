// Package unilyrics calls the shared Rust core through its native C interface.
package unilyrics

/*
#cgo LDFLAGS: -lunilyrics_ffi
#include <stdint.h>
#include <stdlib.h>
char *unilyrics_process(const uint8_t *data, size_t length);
void unilyrics_free(char *response);
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"reflect"
	"unicode/utf8"
)

type Error struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

func (e *Error) Error() string { return e.Code + ": " + e.Message }

type Segment struct {
	ID         *string                    `json:"id,omitempty"`
	Language   *string                    `json:"language,omitempty"`
	AgentIDs   []string                   `json:"agent_ids,omitempty"`
	Roles      []string                   `json:"roles,omitempty"`
	Text       string                     `json:"text"`
	Start      *uint64                    `json:"start_us"`
	End        *uint64                    `json:"end_us"`
	Attributes map[string]json.RawMessage `json:"attributes"`
}
type Cue struct {
	Language   *string                    `json:"language,omitempty"`
	AgentIDs   []string                   `json:"agent_ids,omitempty"`
	Roles      []string                   `json:"roles,omitempty"`
	ParentID   *string                    `json:"parent_id,omitempty"`
	ID         string                     `json:"id"`
	Start      *uint64                    `json:"start_us"`
	End        *uint64                    `json:"end_us"`
	Segments   []Segment                  `json:"segments"`
	Attributes map[string]json.RawMessage `json:"attributes"`
}
type Metadata struct {
	Key   string `json:"key"`
	Value string `json:"value"`
}
type Document struct {
	Language   *string                    `json:"language,omitempty"`
	Agents     []Agent                    `json:"agents,omitempty"`
	Auxiliary  []Auxiliary                `json:"auxiliary,omitempty"`
	Version    uint32                     `json:"version"`
	Cues       []Cue                      `json:"cues"`
	Metadata   []Metadata                 `json:"metadata"`
	Attributes map[string]json.RawMessage `json:"attributes"`
}
type Agent struct {
	ID         string                     `json:"id"`
	Name       *string                    `json:"name,omitempty"`
	Kind       *string                    `json:"kind,omitempty"`
	Attributes map[string]json.RawMessage `json:"attributes"`
}
type Auxiliary struct {
	ID         string                     `json:"id"`
	Kind       string                     `json:"kind"`
	Language   *string                    `json:"language,omitempty"`
	TargetIDs  []string                   `json:"target_ids"`
	Segments   []Segment                  `json:"segments"`
	Attributes map[string]json.RawMessage `json:"attributes"`
}
type Features struct {
	LineTiming     bool     `json:"line_timing"`
	SegmentTiming  bool     `json:"segment_timing"`
	ExplicitEnds   bool     `json:"explicit_ends"`
	Agents         bool     `json:"agents"`
	Relationships  bool     `json:"relationships"`
	Languages      []string `json:"languages"`
	Roles          []string `json:"roles"`
	AuxiliaryKinds []string `json:"auxiliary_kinds"`
	Extensions     bool     `json:"extensions"`
}
type Diagnostic struct {
	Code    string  `json:"code"`
	Message string  `json:"message"`
	CueID   *string `json:"cue_id"`
}
type ParseResult struct {
	Document    Document     `json:"document"`
	Diagnostics []Diagnostic `json:"diagnostics"`
}
type ExportResult struct {
	Output string       `json:"output"`
	Losses []Diagnostic `json:"losses"`
}

func Process(request any) (json.RawMessage, error) {
	input, err := json.Marshal(request)
	if err != nil {
		return nil, err
	}
	if len(input) > 1048576 {
		return nil, &Error{Code: "INPUT_TOO_LARGE", Message: "Request exceeds 1 MiB"}
	}
	if !utf8.Valid(input) || !validRequestUTF8(request) {
		return nil, &Error{Code: "INVALID_UTF8", Message: "Request strings and keys must be valid UTF-8"}
	}
	buffer := C.CBytes(input)
	defer C.free(buffer)
	output := C.unilyrics_process((*C.uint8_t)(buffer), C.size_t(len(input)))
	if output == nil {
		return nil, fmt.Errorf("UniLyrics returned no response")
	}
	defer C.unilyrics_free(output)
	var response struct {
		OK     bool            `json:"ok"`
		Result json.RawMessage `json:"result"`
		Error  *Error          `json:"error"`
	}
	if err := json.Unmarshal([]byte(C.GoString(output)), &response); err != nil {
		return nil, err
	}
	if !response.OK {
		if response.Error == nil {
			return nil, fmt.Errorf("Invalid UniLyrics error response")
		}
		return nil, response.Error
	}
	return response.Result, nil
}

func validRequestUTF8(request any) bool {
	marshalType := reflect.TypeOf((*json.Marshaler)(nil)).Elem()
	type reference struct {
		kind    reflect.Type
		pointer uintptr
		length  int
	}
	seen := map[reference]bool{}
	pending := []reflect.Value{reflect.ValueOf(request)}
	for len(pending) != 0 {
		value := pending[len(pending)-1]
		pending = pending[:len(pending)-1]
		if !value.IsValid() {
			continue
		}
		// Caller-provided JSON serializers own their representation; check their emitted bytes instead.
		if value.Type().Implements(marshalType) || value.CanAddr() && value.Addr().Type().Implements(marshalType) {
			continue
		}
		if value.Kind() == reflect.Pointer || value.Kind() == reflect.Map || value.Kind() == reflect.Slice {
			if value.IsNil() {
				continue
			}
			key := reference{kind: value.Type(), pointer: uintptr(value.UnsafePointer())}
			if value.Kind() == reflect.Slice {
				key.length = value.Len()
			}
			if seen[key] {
				continue
			}
			seen[key] = true
		}
		switch value.Kind() {
		case reflect.String:
			if !utf8.ValidString(value.String()) {
				return false
			}
		case reflect.Interface, reflect.Pointer:
			if !value.IsNil() {
				pending = append(pending, value.Elem())
			}
		case reflect.Struct:
			for i := 0; i < value.NumField(); i++ {
				field := value.Type().Field(i)
				if field.PkgPath == "" && field.Tag.Get("json") != "-" {
					pending = append(pending, value.Field(i))
				}
			}
		case reflect.Map:
			iterator := value.MapRange()
			for iterator.Next() {
				pending = append(pending, iterator.Key(), iterator.Value())
			}
		case reflect.Slice, reflect.Array:
			for i := 0; i < value.Len(); i++ {
				pending = append(pending, value.Index(i))
			}
		}
	}
	return true
}

func Parse(input, format string) (ParseResult, error) {
	request := map[string]any{"op": "parse", "input": input}
	if format != "" {
		request["format"] = format
	}
	raw, err := Process(request)
	if err != nil {
		return ParseResult{}, err
	}
	var result ParseResult
	err = json.Unmarshal(raw, &result)
	return result, err
}

func Export(document Document, format, policy string) (ExportResult, error) {
	if policy == "" {
		policy = "compatible"
	}
	raw, err := Process(map[string]any{"op": "export", "document": document, "format": format, "policy": policy})
	if err != nil {
		return ExportResult{}, err
	}
	var result ExportResult
	err = json.Unmarshal(raw, &result)
	return result, err
}

func Detect(input string) (string, error) {
	raw, err := Process(map[string]any{"op": "detect", "input": input})
	if err != nil {
		return "", err
	}
	var format string
	err = json.Unmarshal(raw, &format)
	return format, err
}

func Validate(document Document) error {
	_, err := Process(map[string]any{"op": "validate", "document": document})
	return err
}

func Inspect(document Document) (Features, error) {
	raw, err := Process(map[string]any{"op": "inspect", "document": document})
	if err != nil {
		return Features{}, err
	}
	var result Features
	err = json.Unmarshal(raw, &result)
	return result, err
}

// Normalize shifts every explicit time, rejecting range errors without changing document.
func Normalize(document Document, offsetUS int64) (ParseResult, error) {
	raw, err := Process(map[string]any{"op": "normalize", "document": document, "offset_us": offsetUS})
	if err != nil {
		return ParseResult{}, err
	}
	var result ParseResult
	err = json.Unmarshal(raw, &result)
	return result, err
}
