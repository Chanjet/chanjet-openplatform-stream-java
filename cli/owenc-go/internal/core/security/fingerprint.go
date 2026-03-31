package security

import (
	"crypto/sha256"
	"fmt"
	"os"
	"runtime"
)

// GetMachineFingerprint generates a unique stable identifier for the current machine.
// In a real production environment, this would use hardware IDs (CPU, Disk, etc.).
// For this implementation, we combine OS, Architecture, and Hostname as a stable base.
func GetMachineFingerprint() (string, error) {
	hostname, err := os.Hostname()
	if err != nil {
		return "", err
	}

	// Stable attributes
	base := fmt.Sprintf("%s-%s-%s", runtime.GOOS, runtime.GOARCH, hostname)
	
	hash := sha256.Sum256([]byte(base))
	return fmt.Sprintf("%x", hash), nil
}
