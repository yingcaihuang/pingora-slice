#!/bin/bash
# Installation script for Pingora Slice

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Detect OS
detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$ID
        VER=$VERSION_ID
    else
        echo -e "${RED}Cannot detect OS${NC}"
        exit 1
    fi
}

# Check if running as root
check_root() {
    if [ "$EUID" -ne 0 ]; then
        echo -e "${RED}Please run as root or with sudo${NC}"
        exit 1
    fi
}

# Install dependencies
install_dependencies() {
    echo -e "${GREEN}Installing dependencies...${NC}"
    
    case $OS in
        centos|rocky|almalinux)
            dnf install -y openssl-libs
            ;;
        *)
            echo -e "${YELLOW}Unknown OS: $OS${NC}"
            echo -e "${YELLOW}Please install openssl-libs manually${NC}"
            ;;
    esac
}

# Download and install RPM
install_rpm() {
    local VERSION=${1:-latest}
    local GITHUB_REPO="your-username/pingora-slice"  # Update this
    
    echo -e "${GREEN}Detecting system version...${NC}"
    detect_os
    
    # Determine RPM dist
    case $OS in
        centos|rocky|almalinux)
            case ${VER%%.*} in
                8)
                    DIST="el8"
                    ;;
                9)
                    DIST="el9"
                    ;;
                *)
                    echo -e "${RED}Unsupported version: $VER${NC}"
                    exit 1
                    ;;
            esac
            ;;
        *)
            echo -e "${RED}Unsupported OS: $OS${NC}"
            exit 1
            ;;
    esac
    
    echo -e "${GREEN}Detected: $OS $VER (${DIST})${NC}"
    
    # Get latest version if not specified
    if [ "$VERSION" = "latest" ]; then
        echo -e "${GREEN}Fetching latest version...${NC}"
        VERSION=$(curl -s "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"v([^"]+)".*/\1/')
        if [ -z "$VERSION" ]; then
            echo -e "${RED}Failed to fetch latest version${NC}"
            exit 1
        fi
    fi
    
    echo -e "${GREEN}Installing version: $VERSION${NC}"
    
    # Download RPM
    RPM_FILE="pingora-slice-${VERSION}-1.${DIST}.x86_64.rpm"
    DOWNLOAD_URL="https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/${RPM_FILE}"
    
    echo -e "${GREEN}Downloading from: $DOWNLOAD_URL${NC}"
    
    TMP_DIR=$(mktemp -d)
    cd "$TMP_DIR"
    
    if ! curl -L -o "$RPM_FILE" "$DOWNLOAD_URL"; then
        echo -e "${RED}Failed to download RPM${NC}"
        rm -rf "$TMP_DIR"
        exit 1
    fi
    
    # Install RPM
    echo -e "${GREEN}Installing RPM...${NC}"
    dnf install -y "./$RPM_FILE"
    
    # Cleanup
    cd -
    rm -rf "$TMP_DIR"
    
    echo -e "${GREEN}Installation completed successfully!${NC}"
    echo ""
    echo -e "${YELLOW}Next steps:${NC}"
    echo "1. Edit configuration: sudo vi /etc/pingora-slice/pingora_slice.yaml"
    echo "2. Start service: sudo systemctl start pingora-slice"
    echo "3. Enable on boot: sudo systemctl enable pingora-slice"
    echo "4. Check status: sudo systemctl status pingora-slice"
    echo "5. View logs: sudo journalctl -u pingora-slice -f"
}

# Main
main() {
    check_root
    install_dependencies
    install_rpm "$@"
}

main "$@"
