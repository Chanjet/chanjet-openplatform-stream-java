package ui

import (
	"fmt"
	"strings"
)

// Confirm asks the user for confirmation (Y/N).
func Confirm(question string) bool {
	fmt.Printf("%s [y/N]: ", question)
	var response string
	_, err := fmt.Scanln(&response)
	if err != nil {
		return false
	}
	response = strings.ToLower(strings.TrimSpace(response))
	return response == "y" || response == "yes"
}

// Info prints an informational message with a link.
func Info(message, link string) {
	fmt.Printf("\n💡 %s\n🔗 %s\n\n", message, link)
}
