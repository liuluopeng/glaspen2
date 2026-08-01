#!/bin/bash
# Create and install a self-signed code-signing certificate into a DEDICATED
# keychain (glaspen2-signing). Signing with this stable certificate keeps the
# macOS Accessibility permission (TCC) across app updates.
#
# Why dedicated: the login keychain would prompt for the login password on
# every codesign call. The dedicated keychain's password is known to the
# build scripts, so codesign runs non-interactively via the partition list.
#
# Why not ad-hoc: ad-hoc signatures are keyed on the build's cdhash, which
# changes on every rebuild — TCC treats each new build as a different app
# and the user must remove + re-add the Accessibility permission.
#
# Run once per machine:  scripts/create-signing-cert.sh
set -e

NAME="Glaspen2 Development"
PASS="glaspen2"
KC="${HOME}/Library/Keychains/glaspen2-signing.keychain-db"
LOGIN_KC="${HOME}/Library/Keychains/login.keychain-db"
TMP=$(mktemp -d)
trap 'rm -rf "${TMP}"' EXIT

# 1. Dedicated keychain (created once, password known to the scripts)
if [ ! -f "${KC}" ]; then
    echo "Creating dedicated keychain ${KC}"
    security create-keychain -p "${PASS}" "${KC}"
fi
security set-keychain-settings -lut 21600 "${KC}" 2>/dev/null || true
security unlock-keychain -p "${PASS}" "${KC}"

# 2. Certificate (recreated only if missing)
if security find-identity -p codesigning "${KC}" 2>/dev/null | grep -q "${NAME}"; then
    echo "'${NAME}' already exists in the dedicated keychain."
else
    openssl req -newkey rsa:2048 -nodes -keyout "${TMP}/key.pem" \
        -x509 -out "${TMP}/cert.pem" -days 3650 \
        -subj "/CN=${NAME}/O=glaspen2" \
        -addext "extendedKeyUsage=codeSigning" \
        -addext "keyUsage=digitalSignature" 2>/dev/null

    openssl pkcs12 -export -out "${TMP}/cert.p12" \
        -inkey "${TMP}/key.pem" -in "${TMP}/cert.pem" \
        -passout "pass:${PASS}" -legacy 2>/dev/null

    security import "${TMP}/cert.p12" -k "${KC}" -P "${PASS}" -T /usr/bin/codesign
    echo "Certificate '${NAME}' imported."
fi

# 3. Allow codesign to use the key WITHOUT prompting (partition list)
security set-key-partition-list -S apple-tool:,apple:,codesign: -k "${PASS}" "${KC}"
security unlock-keychain -p "${PASS}" "${KC}"

# 4. Make codesign find it: dedicated keychain first in the search list
security list-keychains -d user -s "${KC}" "${LOGIN_KC}"

# 5. Remove any earlier copy from the login keychain (would prompt for the
#    login password during signing)
security delete-certificate -c "${NAME}" "${LOGIN_KC}" 2>/dev/null \
    && echo "Removed old '${NAME}' from the login keychain." || true

echo
echo "Done. Rebuild the DMG:  scripts/build-dmg.sh"
