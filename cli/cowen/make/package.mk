# --- 打包目标 ---

test-macos-fast: quality-gate local-db-up
	@echo "🧪 Running macOS integration tests (Fast Release Mode)..."
	COWEN_BUILD_CLIENT_ID=dummy RUSTFLAGS="-D warnings" cargo build --release -p cowen-cli -p cowen-daemon
	COWEN_BUILD_CLIENT_ID=dummy COWEN_SKIP_BROWSER=true RUSTFLAGS="-D warnings" cargo test --release
	COWEN_SKIP_BROWSER=true TEST_BASE=target/cowen_tests_macos BASE_PORT_START=18000 crates/app/cowen-cli/tests/runners/run_parallel.sh

package-macos-fast: test-macos-fast macos-aarch64
	@echo "📦 Packaging macOS Apple Silicon (Fast Component Based)..."
	@$(MAKE) impl-package-macos ARCH=aarch64

package-macos-aarch64: clean macos-aarch64
	@echo "📦 Packaging macOS Apple Silicon (Component Based)..."
	@$(MAKE) impl-package-macos ARCH=aarch64

impl-package-macos: build-system-plugins
	@echo "📦 Building Component Packages for $(ARCH)..."
	mkdir -p pkg_core_root/usr/local/bin pkg_core_root/usr/local/share/doc/cowen pkg_core_root/usr/local/share/cowen/system_plugins $(OUTPUT_DIR)/macos-$(ARCH)/release
	mkdir -p pkg_plugin_ai_root/usr/local/share/cowen/staging
	mkdir -p pkg_plugin_mcp_root/usr/local/share/cowen/staging
	cp $(OUTPUT_DIR)/macos-$(ARCH)/$(BINARY) pkg_core_root/usr/local/bin/$(BINARY)
	cp $(OUTPUT_DIR)/macos-$(ARCH)/cowen-daemon pkg_core_root/usr/local/bin/cowen-daemon
	cp dist_assets/macos/cowen-uninstall pkg_core_root/usr/local/bin/cowen-uninstall
	cp CHANGELOG.md pkg_core_root/usr/local/share/doc/cowen/
	cp -r dist_assets/usage pkg_core_root/usr/local/share/doc/cowen/usage
	cp target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm pkg_core_root/usr/local/share/cowen/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.bundle pkg_core_root/usr/local/share/cowen/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.wasm pkg_core_root/usr/local/share/cowen/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.bundle pkg_core_root/usr/local/share/cowen/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_custom.wasm pkg_core_root/usr/local/share/cowen/system_plugins/ || true
	cp target/release/libcowen_search_embedding pkg_plugin_ai_root/usr/local/share/cowen/staging/ || true
	if [ -f "pkg_plugin_ai_root/usr/local/share/cowen/staging/libcowen_search_embedding" ]; then cp target/release/libcowen_search_embedding.bundle pkg_plugin_ai_root/usr/local/share/cowen/staging/ || exit 1; fi
	if [ -f "pkg_plugin_ai_root/usr/local/share/cowen/staging/libcowen_search_embedding" ]; then cp dist_assets/macos/libonnxruntime.dylib pkg_plugin_ai_root/usr/local/share/cowen/staging/ || exit 1; fi
	cp target/release/cowen-mcp-plugin pkg_plugin_mcp_root/usr/local/share/cowen/staging/ || true
	if [ -f "pkg_plugin_mcp_root/usr/local/share/cowen/staging/cowen-mcp-plugin" ]; then cp target/release/cowen-mcp-plugin.bundle pkg_plugin_mcp_root/usr/local/share/cowen/staging/ || exit 1; fi
	@chmod +x dist_assets/macos/scripts_core/postinstall
	@chmod +x dist_assets/macos/scripts_plugin/postinstall
	pkgbuild --identifier com.chanjet.$(BINARY).core.$(ARCH) --version $(VERSION) --root pkg_core_root --install-location / --scripts dist_assets/macos/scripts_core $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-core.pkg
	pkgbuild --identifier com.chanjet.$(BINARY).plugin.ai.$(ARCH) --version $(VERSION) --root pkg_plugin_ai_root --install-location / --scripts dist_assets/macos/scripts_plugin $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-plugin-ai.pkg
	pkgbuild --identifier com.chanjet.$(BINARY).plugin.mcp.$(ARCH) --version $(VERSION) --root pkg_plugin_mcp_root --install-location / --scripts dist_assets/macos/scripts_plugin $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-plugin-mcp.pkg
	@echo "🧩 Synthesizing Distribution UI..."
	cd $(OUTPUT_DIR)/macos-$(ARCH)/release && \
	productbuild --synthesize --package cowen-core.pkg --package cowen-plugin-ai.pkg --package cowen-plugin-mcp.pkg Distribution.xml && \
	sed -i '' 's/customize="never"/customize="always"/' Distribution.xml && \
	perl -0777 -pi -e 's#<choices-outline>.*?</choices-outline>#<choices-outline>\\n        <line choice="com.chanjet.$(BINARY).core.$(ARCH)"/>\\n        <line choice="com.chanjet.$(BINARY).plugins_group">\\n            <line choice="com.chanjet.$(BINARY).plugin.ai.$(ARCH)"/>\\n            <line choice="com.chanjet.$(BINARY).plugin.mcp.$(ARCH)"/>\\n        </line>\\n    </choices-outline>#s' Distribution.xml && \
	perl -pi -e 's#<choice id="default"/>##g' Distribution.xml && \
	sed -i '' 's/<choice id="com.chanjet.$(BINARY).core.$(ARCH)".*>/<choice id="com.chanjet.$(BINARY).plugins_group" title="Cowen Plugins" description="Optional plugins for Cowen." visible="true" selected="true"\/>\n    <choice id="com.chanjet.$(BINARY).core.$(ARCH)" title="Cowen Core CLI" description="The core Cowen CLI and daemon." visible="true" enabled="false" selected="true">/' Distribution.xml && \
	sed -i '' 's/<choice id="com.chanjet.$(BINARY).plugin.ai.$(ARCH)".*>/<choice id="com.chanjet.$(BINARY).plugin.ai.$(ARCH)" title="Cowen AI Plugin" description="Installs the AI Embedding and Search plugin." visible="true" selected="true">/' Distribution.xml && \
	sed -i '' 's/<choice id="com.chanjet.$(BINARY).plugin.mcp.$(ARCH)".*>/<choice id="com.chanjet.$(BINARY).plugin.mcp.$(ARCH)" title="Cowen MCP Plugin" description="Installs the Model Context Protocol (MCP) plugin." visible="true" selected="true">/' Distribution.xml && \
	productbuild --distribution Distribution.xml --package-path . $(BINARY)-v$(VERSION)-macos-$(ARCH).pkg
	rm -rf pkg_core_root pkg_plugin_ai_root pkg_plugin_mcp_root
	rm -f $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-core.pkg $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-plugin-ai.pkg $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-plugin-mcp.pkg $(OUTPUT_DIR)/macos-$(ARCH)/release/Distribution.xml
	cp dist_assets/macos/cowen-uninstall $(OUTPUT_DIR)/macos-$(ARCH)/release/cowen-uninstall
	@$(call MD5,$(OUTPUT_DIR)/macos-$(ARCH)/release/$(BINARY)-v$(VERSION)-macos-$(ARCH).pkg)
	@$(call SHA256,$(OUTPUT_DIR)/macos-$(ARCH)/release/$(BINARY)-v$(VERSION)-macos-$(ARCH).pkg)

package-linux-x86_64: clean linux-x86_64
	@echo "📦 Packaging Linux x86_64 (Native Build)..."
	@$(MAKE) impl-package-linux-x86_64

package-linux-x86_64-with-docker: clean linux-x86_64-with-docker
	@echo "📦 Packaging Linux x86_64 (Docker Build)..."
	@$(MAKE) impl-package-linux-x86_64

package-linux-x86_64-cross: clean linux-x86_64-cross
	@echo "📦 Packaging Linux x86_64 (Cross Build via zigbuild)..."
	@$(MAKE) impl-package-linux-x86_64

package-linux-aarch64: clean linux-aarch64 build-system-plugins
	@echo "📦 Packaging Linux aarch64..."
	mkdir -p $(OUTPUT_DIR)/linux-aarch64/release pkg_root_aarch64/lib
	cp target/release/libcowen_search_embedding pkg_root_aarch64/lib/ || true
	if [ -f "pkg_root_aarch64/lib/libcowen_search_embedding" ]; then cp target/release/libcowen_search_embedding.bundle pkg_root_aarch64/lib/ || exit 1; fi
	if [ -f "pkg_root_aarch64/lib/libcowen_search_embedding" ]; then cp dist_assets/linux/libonnxruntime.so pkg_root_aarch64/lib/ || exit 1; fi
	cp target/release/cowen-mcp-plugin pkg_root_aarch64/lib/ || true
	if [ -f "pkg_root_aarch64/lib/cowen-mcp-plugin" ]; then cp target/release/cowen-mcp-plugin.bundle pkg_root_aarch64/lib/ || exit 1; fi
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-aarch64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-aarch64/README.txt
	cp CHANGELOG.md $(OUTPUT_DIR)/linux-aarch64/
	cp -r dist_assets/usage $(OUTPUT_DIR)/linux-aarch64/
	mkdir -p pkg_root_aarch64/system_plugins
	cp target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm pkg_root_aarch64/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.bundle pkg_root_aarch64/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.wasm pkg_root_aarch64/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.bundle pkg_root_aarch64/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_custom.wasm pkg_root_aarch64/system_plugins/ || true
	cd $(OUTPUT_DIR)/linux-aarch64 && tar -czf release/$(BINARY)-v$(VERSION)-linux-aarch64.tar.gz $(BINARY) cowen-daemon install.sh README.txt CHANGELOG.md usage -C ../../cli/cowen/pkg_root_aarch64 lib system_plugins
	rm -rf pkg_root_aarch64
	@$(call MD5,$(OUTPUT_DIR)/linux-aarch64/release/$(BINARY)-v$(VERSION)-linux-aarch64.tar.gz)
	@$(call SHA256,$(OUTPUT_DIR)/linux-aarch64/release/$(BINARY)-v$(VERSION)-linux-aarch64.tar.gz)

package-windows-x86_64: clean windows-x86_64
	@echo "📦 Packaging Windows x86_64..."
	powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(OUTPUT_DIR)/windows-x86_64/release"
	powershell -NoProfile -Command "Copy-Item -Path CHANGELOG.md -Destination ../cowen_setup/ -Force"
	powershell -NoProfile -Command "cd ../cowen_setup; $$env:CARGO_BIN_NAME_OVERRIDE='$(BINARY)'; $$env:APP_DIR_NAME='.$(BINARY)'; $$env:DEF_OPENAPI_URL='$(PROD_OPENAPI)'; $$env:DEF_STREAM_URL='$(PROD_STREAM)'; cargo build --release --target x86_64-pc-windows-msvc"
	powershell -NoProfile -Command "Copy-Item -Path ../cowen_setup/target/x86_64-pc-windows-msvc/release/cowen_setup.exe -Destination $(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.exe -Force"
	@echo "🗜️ Zipping Windows package..."
	powershell -NoProfile -Command "Compress-Archive -Path $(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.exe -DestinationPath $(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.zip -Force"
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.zip)
	@$(call SHA256,$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.zip)
	@echo "🔍 Automating Windows package verification..."
	powershell -NoProfile -ExecutionPolicy Bypass -File "crates/app/cowen-cli/tests/runners/verify_windows_pkg.ps1" -SetupExePath "$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.exe"



package-windows-x86_64-cross: windows-x86_64-cross build-system-plugins
	@echo "📦 Packaging Windows Setup natively via cargo-zigbuild..."
	mkdir -p $(OUTPUT_DIR)/windows-x86_64/release
	cp CHANGELOG.md ../cowen_setup/ || true
	WINAPI_X64_LIB_DIR=$$(cargo metadata --format-version 1 | tr ',' '\n' | grep winapi-x86_64-pc-windows-gnu | grep src_path | head -n 1 | cut -d '"' -f 4 | sed 's/\/src\/lib.rs/\/lib/g'); \
	cd ../cowen_setup && CARGO_BIN_NAME_OVERRIDE=$(BINARY) APP_DIR_NAME=.$(BINARY) DEF_OPENAPI_URL=$(PROD_OPENAPI) DEF_STREAM_URL=$(PROD_STREAM) RUSTFLAGS="-L $$WINAPI_X64_LIB_DIR" cargo zigbuild --release --target x86_64-pc-windows-gnu && cp target/x86_64-pc-windows-gnu/release/cowen_setup.exe ../cowen/$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.exe
	cd $(OUTPUT_DIR)/windows-x86_64/release && zip $(BINARY)-v$(VERSION)-windows-x86_64-setup.zip $(BINARY)-v$(VERSION)-windows-x86_64-setup.exe
	@$(call MD5,$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.zip)
	@$(call SHA256,$(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.zip)
	@echo "✅ Native cross packaging completed: $(OUTPUT_DIR)/windows-x86_64/release/$(BINARY)-v$(VERSION)-windows-x86_64-setup.zip"

# 内部复用的 Linux 打包逻辑
impl-package-linux-x86_64: build-system-plugins
	mkdir -p $(OUTPUT_DIR)/linux-x86_64/release pkg_root/lib
	cp $(OUTPUT_DIR)/linux-x86_64/libcowen_search_embedding pkg_root/lib/ || true
	if [ -f "pkg_root/lib/libcowen_search_embedding" ]; then cp $(OUTPUT_DIR)/linux-x86_64/libcowen_search_embedding.bundle pkg_root/lib/ || exit 1; fi
	if [ -f "pkg_root/lib/libcowen_search_embedding" ]; then cp dist_assets/linux/libonnxruntime.so pkg_root/lib/ || exit 1; fi
	cp $(OUTPUT_DIR)/linux-x86_64/cowen-mcp-plugin pkg_root/lib/ || true
	if [ -f "pkg_root/lib/cowen-mcp-plugin" ]; then cp $(OUTPUT_DIR)/linux-x86_64/cowen-mcp-plugin.bundle pkg_root/lib/ || exit 1; fi
	cp ./dist_assets/linux/install.sh $(OUTPUT_DIR)/linux-x86_64/
	cp ./dist_assets/QUICK_START.txt $(OUTPUT_DIR)/linux-x86_64/README.txt
	cp CHANGELOG.md $(OUTPUT_DIR)/linux-x86_64/
	cp -r dist_assets/usage $(OUTPUT_DIR)/linux-x86_64/
	mkdir -p pkg_root/system_plugins
	cp target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm pkg_root/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.bundle pkg_root/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.wasm pkg_root/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.bundle pkg_root/system_plugins/ || true
	cp target/wasm32-wasip1/release/cowen_wasm_auth_custom.wasm pkg_root/system_plugins/ || true
	cd $(OUTPUT_DIR)/linux-x86_64 && tar -czf release/$(BINARY)-v$(VERSION)-linux-x86_64.tar.gz $(BINARY) cowen-daemon install.sh README.txt CHANGELOG.md usage -C ../../cli/cowen/pkg_root lib system_plugins
	rm -rf pkg_root
	@$(call MD5,$(OUTPUT_DIR)/linux-x86_64/release/$(BINARY)-v$(VERSION)-linux-x86_64.tar.gz)
	@$(call SHA256,$(OUTPUT_DIR)/linux-x86_64/release/$(BINARY)-v$(VERSION)-linux-x86_64.tar.gz)

package-all-on-macos: clean package-macos-aarch64 package-linux-x86_64-cross package-windows-x86_64-cross
	@echo "🎉 All packages successfully built on macOS!"
