APP_NAME = SauronMenu
APP_BUNDLE = $(APP_NAME).app
APP_INSTALL_DIR = /Applications
BINARY_INSTALL_DIR = /usr/local/bin

.PHONY: build build-rust build-menubar install uninstall clean

build: build-rust build-menubar

build-rust:
	cargo build --release

build-menubar:
	cd menubar && swift build -c release
	@# Assemble .app bundle
	mkdir -p menubar/$(APP_BUNDLE)/Contents/MacOS
	cp menubar/.build/release/$(APP_NAME) menubar/$(APP_BUNDLE)/Contents/MacOS/$(APP_NAME)
	cp menubar/Info.plist menubar/$(APP_BUNDLE)/Contents/Info.plist

install: build
	@echo "Installing sauron binary..."
	sudo cp target/release/sauron $(BINARY_INSTALL_DIR)/sauron
	sudo codesign --force --sign - $(BINARY_INSTALL_DIR)/sauron
	@echo "Installing menu bar app..."
	sudo cp -R menubar/$(APP_BUNDLE) $(APP_INSTALL_DIR)/$(APP_BUNDLE)
	sudo codesign --force --sign - $(APP_INSTALL_DIR)/$(APP_BUNDLE)
	@echo "Setting up launchd services..."
	sauron install
	@echo "Done! Sauron daemon and menu bar app are installed."

uninstall:
	sauron uninstall
	sudo rm -f $(BINARY_INSTALL_DIR)/sauron
	sudo rm -rf $(APP_INSTALL_DIR)/$(APP_BUNDLE)
	@echo "Sauron uninstalled."

clean:
	cargo clean
	cd menubar && swift package clean
	rm -rf menubar/$(APP_BUNDLE)
