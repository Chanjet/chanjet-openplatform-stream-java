# 畅捷通 Stream Gateway Monorepo Makefile

PROJECT_NAME := open-streaming-connector
VERSION := 0.2.0
# 使用绝对路径确保子目录调用有效
ROOT_DIR := $(shell pwd)
MAVEN_SETTINGS := $(ROOT_DIR)/.mvn/settings.xml
MVN := mvn -s $(MAVEN_SETTINGS)

.PHONY: help init build-all clean test test-java test-nodejs test-go build-cli-inte build-cli-prod proto

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  build-all    Build all services and SDKs"
	@echo "  test         Run all tests (Java, Node.js, Go)"
	@echo "  test-java    Run Java unit tests"
	@echo "  test-nodejs  Run Node.js unit tests"
	@echo "  test-go      Run Go unit tests"
	@echo "  clean        Clean all build artifacts"
	@echo "  build-cli-inte  Build CLI with integration feature (delegated)"
	@echo "  build-cli-prod  Build CLI production version (delegated)"

build-all:
	@echo "Building all modules..."
	@$(MVN) clean install -DskipTests
	@cd sdk/nodejs && npm install && npm run build
	@cd sdk/go && go build ./...
	@echo "Building CLI..."
	@$(MAKE) -C cli/cowen build-prod
	@echo "Done."

# CLI Build Targets (Delegated to cli/cowen/Makefile)
build-cli-inte:
	@$(MAKE) -C cli/cowen build-inte

build-cli-prod:
	@$(MAKE) -C cli/cowen build-prod

test: test-java test-nodejs test-go test-cli

test-java:
	@echo "Running Java tests..."
	@$(MVN) test

test-nodejs:
	@echo "Running Node.js tests..."
	@cd sdk/nodejs && npm test

test-go:
	@echo "Running Go tests..."
	@cd sdk/go && go test ./...

test-cli:
	@echo "Running CLI tests..."
	@$(MAKE) -C cli/cowen test

clean:
	@echo "Cleaning artifacts..."
	@$(MVN) clean
	@rm -rf sdk/nodejs/dist sdk/nodejs/node_modules
	@rm -f sdk/go/go.sum
	@echo "Done."
