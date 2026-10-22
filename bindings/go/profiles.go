package unilyrics

import "encoding/json"

type Profile string

const (
	ProfileFull       Profile = "full"
	ProfileLine       Profile = "line"
	ProfileWord       Profile = "word"
	ProfileKaraoke    Profile = "karaoke"
	ProfilePlain      Profile = "plain"
	ProfileTranslated Profile = "translated"
	ProfileCustom     Profile = "custom"
)

type ProjectionFeature string

const (
	FeatureLineTiming    ProjectionFeature = "line_timing"
	FeatureSegmentTiming ProjectionFeature = "segment_timing"
	FeatureAgents        ProjectionFeature = "agents"
	FeatureRelationships ProjectionFeature = "relationships"
	FeatureLanguages     ProjectionFeature = "languages"
	FeatureRoles         ProjectionFeature = "roles"
	FeatureMetadata      ProjectionFeature = "metadata"
	FeatureAttributes    ProjectionFeature = "attributes"
	FeatureAuxiliary     ProjectionFeature = "auxiliary"
)

// ProjectionOptions defaults to full. Custom requires a non-nil Features slice;
// an empty slice explicitly selects text/structure only.
type ProjectionOptions struct {
	Profile  Profile             `json:"profile,omitempty"`
	Features []ProjectionFeature `json:"features"`
}

// Project returns a validated projection and diagnostics without changing document.
func Project(document Document, options ProjectionOptions) (ParseResult, error) {
	raw, err := Process(map[string]any{"op": "project", "document": document, "options": options})
	if err != nil {
		return ParseResult{}, err
	}
	var result ParseResult
	err = json.Unmarshal(raw, &result)
	return result, err
}

// ExportProfile reports projection losses together with target-format losses.
func ExportProfile(document Document, format string, options ProjectionOptions, policy string) (ExportResult, error) {
	if policy == "" {
		policy = "compatible"
	}
	raw, err := Process(map[string]any{"op": "export_profile", "document": document, "format": format, "options": options, "policy": policy})
	if err != nil {
		return ExportResult{}, err
	}
	var result ExportResult
	err = json.Unmarshal(raw, &result)
	return result, err
}
