# Varmlen — Android

Open-source [xray-core](https://github.com/XTLS/Xray-core) VPN client for Android, with **independent per-app and per-domain split tunneling**. Built on Tauri 2 + SvelteKit; the UI, subscription parser and xray config generator are shared with the [desktop client](https://github.com/demented484/varmlen-client-linux).

> **Status:** working, tested on a physical device. Not on Google Play yet — install the APK directly.

## Features

- VLESS / VMess / Trojan / Shadowsocks over REALITY / TLS; transports tcp · ws · grpc · xhttp · httpupgrade.
- Import a subscription URL, a single share-link, several links, or a raw **xray/v2ray JSON** config — paste from the clipboard or by hand.
- Split tunneling with **independent** modes for apps and for sites (whitelist / blacklist each).
- **Quick Settings tile** to toggle the VPN straight from the notification shade.
- Runs in its own process, so the VPN **survives the app being swiped away**.

## Architecture

A `VpnService` builds the tun interface, `hev-socks5-tunnel` (tun2socks) bridges it to the bundled Android **xray** running as a local SOCKS proxy. The per-app split is enforced by the VpnService; the per-site split by xray routing. See **[ANDROID.md](./ANDROID.md)** for the full data plane + toolchain notes.

## Build

```bash
source ~/varmlen-android-env.sh          # JDK 17, Android SDK/NDK, rust android targets
bash scripts/android-native.sh           # fetch xray-android + build tun2socks into jniLibs
npm install
npm run tauri android build -- --debug --target aarch64 --apk
# → src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

## License

[MIT](./LICENSE). Bundles [xray-core](https://github.com/XTLS/Xray-core) (MPL-2.0) and [hev-socks5-tunnel](https://github.com/heiher/hev-socks5-tunnel); see [NOTICE](./NOTICE).
