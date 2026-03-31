package telemetry

import (
	"encoding/json"
	"fmt"
	"os"
	"runtime/debug"

	"cjtc/pkg/suggestion"
	"gopkg.in/yaml.v3"
)

type OutputFormat string

const (
	FormatJSON OutputFormat = "json"
	FormatYAML OutputFormat = "yaml"
	FormatText OutputFormat = "text"
)

type Response struct {
	Success    bool        `json:"success" yaml:"success"`
	Data       interface{} `json:"data,omitempty" yaml:"data,omitempty"`
	Error      *ErrorInfo  `json:"error,omitempty" yaml:"error,omitempty"`
	Suggestion string      `json:"suggestion,omitempty" yaml:"suggestion,omitempty"`
}

type ErrorInfo struct {
	Code    string `json:"code" yaml:"code"`
	Message string `json:"message" yaml:"message"`
	Stack   string `json:"stack,omitempty" yaml:"stack,omitempty"`
}

func FormatOutput(data interface{}, err error, format OutputFormat) {
	resp := Response{
		Success: err == nil,
		Data:    data,
	}

	if err != nil {
		resp.Error = &ErrorInfo{
			Code:    "ERR_EXECUTION",
			Message: err.Error(),
		}
		resp.Suggestion = suggestion.GetSuggestion(err)
	}

	switch format {
	case FormatJSON:
		b, _ := json.MarshalIndent(resp, "", "  ")
		fmt.Println(string(b))
	case FormatYAML:
		b, _ := yaml.Marshal(resp)
		fmt.Println(string(b))
	default:
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\nSuggestion: %s\n", err, resp.Suggestion)
		} else if data != nil {
			fmt.Printf("%+v\n", data)
		}
	}
}

func Recover(format OutputFormat) {
	if r := recover(); r != nil {
		err := fmt.Errorf("panic: %v", r)
		resp := Response{
			Success: false,
			Error: &ErrorInfo{
				Code:    "ERR_PANIC",
				Message: err.Error(),
				Stack:   string(debug.Stack()),
			},
			Suggestion: "A critical internal error occurred. Please report this with the stack trace.",
		}

		switch format {
		case FormatJSON:
			b, _ := json.MarshalIndent(resp, "", "  ")
			fmt.Println(string(b))
		case FormatYAML:
			b, _ := yaml.Marshal(resp)
			fmt.Println(string(b))
		default:
			fmt.Fprintf(os.Stderr, "PANIC: %v\nStack: %s\nSuggestion: %s\n", r, resp.Error.Stack, resp.Suggestion)
		}
		os.Exit(1)
	}
}
