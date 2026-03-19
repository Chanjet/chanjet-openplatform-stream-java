# 畅捷通 Stream Gateway Monorepo Makefile

PROJECT_NAME := open-streaming-connector
VERSION := 0.1.0
MAVEN_SETTINGS := scripts/chanjet_settings.xml
MVN := mvn -s $(MAVEN_SETTINGS)

.PHONY: help init build-java build-sdk clean test proto

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  init        Initialize development environment"
	@echo "  proto       Generate code from protobuf definitions"
	@echo "  build-java  Build Java service (gateway-java) using chanjet settings"
	@echo "  build-sdk   Build Java SDK using chanjet settings"
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
	@echo "Building Java services using $(MAVEN_SETTINGS)..."
	@if [ -f services/gateway-java/pom.xml ]; then \
		cd services/gateway-java && $(MVN) clean install -DskipTests; \
	else \
		echo "Skipping: services/gateway-java/pom.xml not found."; \
	fi

build-sdk:
	@echo "Building Java SDK using $(MAVEN_SETTINGS)..."
	@if [ -f sdk/java/pom.xml ]; then \
		cd sdk/java && $(MVN) clean install -DskipTests; \
	else \
		echo "Skipping: sdk/java/pom.xml not found."; \
	fi

test:
	@echo "Running all tests..."
	@cd services/gateway-java && $(MVN) test
	@cd sdk/java && $(MVN) test
	@echo "Done."

clean:
	@echo "Cleaning artifacts..."
	@rm -rf target/
	@if [ -d services/gateway-java ]; then \
		find services/gateway-java -name "target" -type d -exec rm -rf {} +; \
	fi
	@if [ -d sdk/java ]; then \
		find sdk/java -name "target" -type d -exec rm -rf {} +; \
	fi
	@echo "Done."
