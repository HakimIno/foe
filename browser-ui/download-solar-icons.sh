#!/bin/bash
mkdir -p ui/icons/solar

download_icon() {
    local name=$1
    local id=$2
    echo "Downloading $name ($id)..."
    curl -sS "https://api.iconify.design/$id.svg?width=24&height=24" > ui/icons/solar/$name.svg
}

download_icon "arrow-left" "solar/alt-arrow-left-line-duotone"
download_icon "arrow-right" "solar/alt-arrow-right-line-duotone"
download_icon "refresh" "solar/restart-line-duotone"
download_icon "lock" "solar/lock-password-unlocked-line-duotone"
download_icon "shield" "solar/shield-check-line-duotone"
download_icon "download" "solar/download-square-line-duotone"
download_icon "search" "solar/magnifer-line-duotone"
download_icon "plus" "solar/add-circle-line-duotone"
download_icon "cancel" "solar/close-circle-line-duotone"
download_icon "menu" "solar/hamburger-menu-line-duotone"
download_icon "person" "solar/user-line-duotone"
download_icon "document" "solar/document-text-line-duotone"
download_icon "globe" "solar/global-line-duotone"
download_icon "arc" "solar/widget-line-duotone"
download_icon "google" "logos/google-icon"

echo "Done!"
