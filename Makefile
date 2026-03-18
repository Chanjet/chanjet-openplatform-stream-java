# 畅捷通 Stream Gateway Monorepo Makefile

PROJECT_NAME := open-streaming-connector
VERSION := 0.1.0

.PHONY: help init build-java clean test proto

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  init        Initialize development environment"
	@echo "  proto       Generate code from protobuf definitions"
	@echo "  build-java  Build Java service (gateway-java)"
	@echo "  test        Run all tests"
	@echo "  clean       Clean all build artifacts"

init:
	@echo "Initializing Monorepo for $(PROJECT_NAME)..."
	@mkdir -p proto/internal proto/gateway services/gateway-java sdk/java sdk/python infra/docker infra/k8s scripts
	@echo "Done."

proto:
	@echo "Generating code from proto files..."
	@# TODO: Integrate with protoc when .proto files are ready
	@echo "No .proto files found yet."

build-java:
	@echo "Building Java services..."
	@if [ -f services/gateway-java/pom.xml ]; then 
		cd services/gateway-java && mvn clean package; 
	else 
		echo "Skipping: services/gateway-java/pom.xml not found."; 
	fi

test:
	@echo "Running all tests..."
	@# TODO: Add cross-language test execution
	@echo "Done."

clean:
	@echo "Cleaning artifacts..."
	@rm -rf target/
	@if [ -d services/gateway-java ]; then 
		find services/gateway-java -name "target" -type d -exec rm -rf {} +; 
	fi
	@echo "Done."
