#!/bin/bash
# Create and install a self-signed code-signing certificate into the login
# keychain. Signing the app with this stable certificate keeps the macOS
# Accessibility permission (TCC) across app updates.
#
# Why: ad-hoc signatures (`codesign --sign -`) are keyed on the build's
# cdhash, which changes on every rebuild — TCC then treats each new build
# as a different app and the user must remove + re-add the Accessibility
# permission. A certificate-signed requirement
#   `identifier ... and certificate root = H"..."` stays stable.
#
# Run once per machine:  scripts/create-signing-cert.sh
set -e

NAME="Glaspen2 Development"
KEYCHAIN="${HOME}/Library/Keychains/login.keychain-db"
PASS="glaspen2"
TMP=$(mktemp -d)
trap 'rm -rf "${TMP}"' EXIT

if security find-identity -p codesigning 2>/dev/null | grep -q "${NAME}"; then
    echo "'${NAME}' already exists — nothing to do."
    exit 0
fi

openssl req -newkey rsa:2048 -nodes -keyout "${TMP}/key.pem" \
    -x509 -out "${TMP}/cert.pem" -days 3650 \
    -subj "/CN=${NAME}/O=glaspen2" \
    -addext "extendedKeyUsage=codeSigning" \
    -addext "keyUsage=digitalSignature" 2>/dev/null

openssl pkcs12 -export -out "${TMP}/cert.p12" \
    -inkey "${TMP}/key.pem" -in "${TMP}/cert.pem" \
    -passout "pass:${PASS}" -legacy 2>/dev/null

security import "${TMP}/cert.p12" -k "${KEYCHAIN}" -P "${PASS}" -T /usr/bin/codesign

echo "Certificate '${NAME}' installed into the login keychain."
echo "Rebuild the DMG:  scripts/build-dmg.sh"
