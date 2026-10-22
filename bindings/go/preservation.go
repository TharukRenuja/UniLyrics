package unilyrics

import "encoding/json"

// PreservationSidecar binds an original canonical snapshot to exact output and format.
// This is untrusted metadata, not authenticated provenance.
type PreservationSidecar struct {
	Version  uint32   `json:"version"`
	Format   string   `json:"format"`
	Output   string   `json:"output"`
	Document Document `json:"document"`
}

// PreservedExportResult requires saving both Output and Sidecar. Losses describe output alone.
type PreservedExportResult struct {
	Output  string              `json:"output"`
	Losses  []Diagnostic        `json:"losses"`
	Sidecar PreservationSidecar `json:"sidecar"`
}

// ExportPreserved retains original data, including profile removals, in a sidecar.
func ExportPreserved(document Document, format string, options ProjectionOptions) (PreservedExportResult, error) {
	raw, err := Process(map[string]any{"op": "export_preserved", "document": document, "format": format, "options": options})
	if err != nil {
		return PreservedExportResult{}, err
	}
	var result PreservedExportResult
	if err := json.Unmarshal(raw, &result); err != nil {
		return PreservedExportResult{}, err
	}
	// Match the serializer used by RestorePreserved, including HTML escaping.
	request, err := json.Marshal(map[string]any{"op": "restore_preserved", "input": result.Output, "format": format, "sidecar": result.Sidecar})
	if err != nil {
		return PreservedExportResult{}, err
	}
	if len(request) > 1048576 {
		return PreservedExportResult{}, &Error{Code: "INPUT_TOO_LARGE", Message: "Encoded preservation restore request exceeds 1 MiB"}
	}
	return result, nil
}

// RestorePreserved validates the snapshot and rejects changed output or format.
func RestorePreserved(input, format string, sidecar PreservationSidecar) (ParseResult, error) {
	raw, err := Process(map[string]any{"op": "restore_preserved", "input": input, "format": format, "sidecar": sidecar})
	if err != nil {
		return ParseResult{}, err
	}
	var result ParseResult
	err = json.Unmarshal(raw, &result)
	return result, err
}
