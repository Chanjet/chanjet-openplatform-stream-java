package suggestion

import "strings"

func GetSuggestion(err error) string {
	msg := err.Error()
	switch {
	case strings.Contains(msg, "app_key"):
		return "Please check if 'app_key' is correctly set in your profile or CJT_APP_KEY env var."
	case strings.Contains(msg, "app_secret"):
		return "Please check if 'app_secret' is correctly stored in vault or CJT_APP_SECRET env var."
	case strings.Contains(msg, "connection refused"):
		return "Please check your network connection or the target service availability."
	case strings.Contains(msg, "unauthorized"):
		return "Authentication failed. Please check your credentials and try 'cjtCli auth reset'."
	default:
		return "Please check the logs in ~/.cjtCli/log/sys.log for more details."
	}
}
