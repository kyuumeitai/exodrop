#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["google-api-python-client>=2.140", "google-auth>=2.30"]
# ///
"""Upload EXODROP to Google Play through the Play Developer API (same flow as Tuninga Radio).

    store/play-upload.py bundle 1.0.0                 # AAB -> internal track (draft), validate, commit
    store/play-upload.py bundle 1.0.0 --tracks internal,alpha --status completed
    store/play-upload.py listing                      # texts + icon, feature graphic, phone screenshots
    store/play-upload.py rollout --tracks internal    # roll the release already on a track out (draft -> completed)
    store/play-upload.py rollout --tracks alpha --from internal   # promote that release to closed testing
    store/play-upload.py status                       # read back what each track holds
    add --dry-run to anything to validate the edit and throw it away instead of committing.

Release metadata comes from store/releases/<version>.json; listings from store/listing/*.json;
images from store/graphics/. Evidence is written to store/releases/<version>-play.json.

Service account key: $PLAY_SERVICE_ACCOUNT_JSON, default ~/.config/google-play/service-account.json.
It must be invited in Play Console > Users and permissions with access to this app.
Brand-new apps only accept draft releases until the first release is rolled out from the Console.
"""
import argparse, datetime, hashlib, json, os, pathlib, socket, sys

socket.setdefaulttimeout(300)  # the default 60 s read timeout is too short for a ~60 MB bundle

from google.oauth2 import service_account
from googleapiclient.discovery import build
from googleapiclient.http import MediaFileUpload

PACKAGE = "cl.lunamedia.exodrop"
ROOT = pathlib.Path(__file__).resolve().parent.parent
STORE = ROOT / "store"
KEY = pathlib.Path(os.environ.get("PLAY_SERVICE_ACCOUNT_JSON", "~/.config/google-play/service-account.json")).expanduser()


def api():
    if not KEY.exists():
        sys.exit(f"Missing service account key: {KEY} (set PLAY_SERVICE_ACCOUNT_JSON)")
    creds = service_account.Credentials.from_service_account_file(
        str(KEY), scopes=["https://www.googleapis.com/auth/androidpublisher"])
    return build("androidpublisher", "v3", credentials=creds, cache_discovery=False).edits()


def finish(edits, edit_id, dry_run):
    edits.validate(packageName=PACKAGE, editId=edit_id).execute()
    if dry_run:
        edits.delete(packageName=PACKAGE, editId=edit_id).execute()
        print("dry run: edit validated and discarded")
        return None
    res = edits.commit(packageName=PACKAGE, editId=edit_id).execute()
    print("committed edit", res["id"])
    return res["id"]


def read_tracks(edits, names):
    edit_id = edits.insert(packageName=PACKAGE, body={}).execute()["id"]
    try:
        return {t: edits.tracks().get(packageName=PACKAGE, editId=edit_id, track=t).execute() for t in names}
    finally:
        edits.delete(packageName=PACKAGE, editId=edit_id).execute()


def cmd_bundle(a):
    meta = json.loads((STORE / "releases" / f"{a.version}.json").read_text())
    aab = pathlib.Path(a.aab or ROOT / "build" / f"exodrop-{a.version}-{meta['versionCode']}.aab")
    if not aab.exists():
        sys.exit(f"Missing {aab}; run store/build-release.sh {a.version} {meta['versionCode']}")
    sha = hashlib.sha256(aab.read_bytes()).hexdigest()
    tracks = a.tracks.split(",")
    edits = api()
    before = read_tracks(edits, tracks) if not a.dry_run else None
    edit_id = edits.insert(packageName=PACKAGE, body={}).execute()["id"]
    print(f"uploading {aab.name} ({aab.stat().st_size // 1024} KB, sha256 {sha[:12]}...)")
    media = MediaFileUpload(str(aab), mimetype="application/octet-stream", resumable=True, chunksize=8 * 1024 * 1024)
    req = edits.bundles().upload(packageName=PACKAGE, editId=edit_id, media_body=media)
    up = None
    while up is None:
        status, up = req.next_chunk(num_retries=5)
        if status:
            print(f"  {int(status.progress() * 100)}%", flush=True)
    if up["versionCode"] != meta["versionCode"]:
        sys.exit(f"Play read versionCode {up['versionCode']}, expected {meta['versionCode']}")
    if up.get("sha256") and up["sha256"] != sha:
        sys.exit("Play's sha256 doesn't match the local bundle")
    release = {
        "name": meta["releaseName"],
        "versionCodes": [str(meta["versionCode"])],
        "status": a.status,
        "releaseNotes": [{"language": k, "text": v} for k, v in meta["releaseNotes"].items()],
    }
    for t in tracks:
        edits.tracks().update(packageName=PACKAGE, editId=edit_id, track=t,
                              body={"track": t, "releases": [release]}).execute()
        print(f"staged {meta['releaseName']} on {t} as {a.status}")
    commit_id = finish(edits, edit_id, a.dry_run)
    if commit_id:
        evidence = {
            "verifiedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
            "package": PACKAGE, "version": a.version, "versionCode": meta["versionCode"],
            "bundle": {"file": str(aab.relative_to(ROOT)), "sha256": sha,
                       "playSha1": up.get("sha1"), "playSha256": up.get("sha256")},
            "editCommit": {"id": commit_id},
            "tracksBefore": before, "tracks": read_tracks(edits, tracks),
        }
        out = STORE / "releases" / f"{a.version}-play.json"
        out.write_text(json.dumps(evidence, indent=2) + "\n")
        print("evidence:", out.relative_to(ROOT))


def cmd_listing(a):
    edits = api()
    edit_id = edits.insert(packageName=PACKAGE, body={}).execute()["id"]
    if a.contact_email:
        body = {"defaultLanguage": "en-US", "contactEmail": a.contact_email}
        if a.website:
            body["contactWebsite"] = a.website
        edits.details().update(packageName=PACKAGE, editId=edit_id, body=body).execute()
    g = STORE / "graphics"
    for f in sorted((STORE / "listing").glob("*.json")):
        d = json.loads(f.read_text())
        lang = d["language"]
        edits.listings().update(packageName=PACKAGE, editId=edit_id, language=lang, body={
            "language": lang, "title": d["title"],
            "shortDescription": d["shortDescription"], "fullDescription": d["fullDescription"]}).execute()
        images = {"icon": [g / "icon-512.png"], "featureGraphic": [g / "feature-graphic.png"],
                  "phoneScreenshots": sorted(g.glob("phone-*.png"))}
        for kind, files in images.items():
            edits.images().deleteall(packageName=PACKAGE, editId=edit_id, language=lang, imageType=kind).execute()
            for p in files:
                edits.images().upload(packageName=PACKAGE, editId=edit_id, language=lang, imageType=kind,
                                      media_body=MediaFileUpload(str(p), mimetype="image/png")).execute()
        print(f"listing {lang}: text + {sum(len(v) for v in images.values())} images")
    finish(edits, edit_id, a.dry_run)


def cmd_rollout(a):
    # set the newest build on a track (e.g. the uploaded draft) to another status, no re-upload
    edits = api()
    edit_id = edits.insert(packageName=PACKAGE, body={}).execute()["id"]
    for t in a.tracks.split(","):
        src = a.from_track or t
        track = edits.tracks().get(packageName=PACKAGE, editId=edit_id, track=src).execute()
        built = [r for r in track.get("releases", []) if r.get("versionCodes")]
        if not built:
            sys.exit(f"{t}: no release with a build to roll out")
        # the newest build replaces whatever the track held before
        newest = max(built, key=lambda r: max(int(c) for c in r["versionCodes"]))
        releases = [{k: newest[k] for k in ("name", "versionCodes", "releaseNotes") if k in newest} | {"status": a.status}]
        edits.tracks().update(packageName=PACKAGE, editId=edit_id, track=t,
                              body={"track": t, "releases": releases}).execute()
        print(f"{t}: {', '.join(r['name'] for r in releases)} -> {a.status}")
    finish(edits, edit_id, a.dry_run)


def cmd_status(a):
    for name, t in read_tracks(api(), a.tracks.split(",")).items():
        for r in t.get("releases", []):
            print(f"{name:10} {r.get('status', ''):12} {r.get('name', '')}  codes={r.get('versionCodes')}")
        if not t.get("releases"):
            print(f"{name:10} (empty)")


p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
sub = p.add_subparsers(dest="cmd", required=True)
b = sub.add_parser("bundle"); b.add_argument("version"); b.add_argument("--aab")
b.add_argument("--tracks", default="internal"); b.add_argument("--status", default="draft",
               choices=["draft", "completed", "inProgress", "halted"])
l = sub.add_parser("listing"); l.add_argument("--contact-email"); l.add_argument("--website")
r = sub.add_parser("rollout"); r.add_argument("--tracks", default="internal")
r.add_argument("--from", dest="from_track", help="copy the release from this track (e.g. promote internal -> alpha)")
r.add_argument("--status", default="completed", choices=["completed", "draft", "halted"])
s = sub.add_parser("status"); s.add_argument("--tracks", default="internal,alpha,beta,production")
for x in (b, l, r, s):
    x.add_argument("--dry-run", action="store_true")
a = p.parse_args()
{"bundle": cmd_bundle, "listing": cmd_listing, "rollout": cmd_rollout, "status": cmd_status}[a.cmd](a)
