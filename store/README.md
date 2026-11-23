# EXODROP: Google Play

Package `cl.lunamedia.exodrop` · Play Console account `alex@lunamedia.cl` (LunaMedia, same as the a30-games titles) ·
first version 0.1.0 (code 1). Same flow as `kyuumeitai/a30-games` `swarm-surge/store/README.md`: upload key outside
the repo, signed AAB targeting SDK 36, Play App Signing, uploads through the Play Developer API with the Tuninga
service account.

| Path | What |
|---|---|
| `build-release.sh <name> <code>` | Signed AAB + APK → `build/exodrop-<name>-<code>.aab` (arm64-v8a + armeabi-v7a, min SDK 26, target 36). |
| `play-upload.py` | `bundle <version>`, `rollout --tracks …`, `listing`, `status`, all with `--dry-run` (run it directly: `uv`). |
| `releases/<version>.json` | Version code, release name and notes (en-US / es-419). |
| `listing/en-US.json`, `listing/es-419.json` | Title, short and full description (30/80/4000 limits checked). |
| `graphics/` | `icon-512.png`, `feature-graphic.png` (1024×500), `phone-1..4.png` (1920×1080, real renders with the touch HUD). Rebuild with `make_store_art.py`; sources and the render commands are in `graphics/src/` and the script's docstring. The same script writes the launcher icons (`mobile/android/app/src/main/res/mipmap-*`). |
| `privacy-policy.html` | Live at https://lunamedia.cl/exodrop/privacy/ (kyuumeitai/lunamedia-cl). |

**Upload key:** `~/.android/upload-keys/exodrop-upload.jks` + `.properties` (alias `upload`, PKCS12, RSA 4096, valid to 2054),
on the MacBook and on deathscythe, and in the encrypted backup `kyuumeitai/lunamedia-signing-keys` (`3ae5f20`).
Upload certificate SHA-256: `26:4D:F8:E2:6B:14:37:7F:EB:D3:B6:EA:7E:6A:0F:06:62:EF:FE:38:07:34:D3:81:A9:F0:96:04:1D:CD:37:6F`.

## One-time setup in Play Console (alex@lunamedia.cl)
1. **Create app:** name "EXODROP: Orbital Exo-Suit", default language English (US), *Game*, *Free*.
2. **Service account:** Users and permissions → give `tuninga-play-submit@tuninga-seo-mcp.iam.gserviceaccount.com`
   access to EXODROP (releases, testers, store listing, app content), like the other games.
3. **First build:** `store/play-upload.py bundle 0.1.0` then `store/play-upload.py rollout --tracks internal`
   (if Play keeps the first release as a draft, roll it out once from the Console).
4. **Listing:** `store/play-upload.py listing --contact-email alex@lunamedia.cl --website https://lunamedia.cl`, then
   category **Games → Action**.
5. **App content:** privacy policy https://lunamedia.cl/exodrop/privacy/ · Ads: **No** · App access: all functionality
   available · Data safety: **no data collected or shared** (no network permission at all) · Advertising ID: **No** ·
   Target audience: 13+ · IARC: sci-fi shooting at robots/alien creatures, no blood, no humans harmed → expect
   Teen / PEGI 7–12 · Government / financial / health / news: No.
6. **Testers:** internal list "Lunamedia app testers".

## Every release
```bash
# add store/releases/<new>.json with a higher versionCode, then
store/build-release.sh 0.1.1 2
store/play-upload.py bundle 0.1.1 --dry-run
store/play-upload.py bundle 0.1.1
store/play-upload.py rollout --tracks internal
```

## Device status (2026-11-22)
- Moto e7i Power (Android 10 Go, PowerVR GE8322, GLES 3.2, armeabi-v7a): release APK launches, plays, pauses
  (Back), survives background/resume; ~25 FPS in combat after the adaptive render scale settles at 0.40, ~68 MB PSS.
- **Not yet run:** any arm64 / Vulkan device (the Samsung A30 was offline). Vulkan is tried first on Android and
  falls back to GL if the adapter or device fails; check logcat tag `exodrop` for the chosen backend.
